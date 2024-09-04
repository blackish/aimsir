use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::{
    self,
    time as async_time,
    sync::RwLock,
};
use axum::{
    response::Json,
    http::StatusCode,
    extract::{Path, State},
};

use crate::model;

#[derive(Serialize, Clone)]
pub struct BackendTag {
    values: HashMap<i64, model::StoreMetric>,
}

#[derive(Clone)]
pub struct BackendState {
    pub metrics: Arc<RwLock<HashMap<i64, HashMap<i64, BackendTag>>>>,
    pub db: Arc<RwLock<dyn model::db::Db + Send + Sync>>,
}

pub async fn stats(State(metrics): State<BackendState>) -> Result<Json<Value>, (StatusCode, String)> {
    let result = metrics.metrics.read().await;
    Ok(Json(json!(*result)))
}

pub async fn stats_id(Path(id): Path<i64>, State(metrics): State<BackendState>) -> Result<Json<Value>, (StatusCode, String)> {
    if let Some(stats) = metrics.metrics.read().await.get(&id) {
        Ok(Json(json!(stats)))
    } else {
        Err((StatusCode::NOT_FOUND, "Id not found".to_string()))
    }
}

pub async fn levels(metrics: &State<BackendState>) -> Result<Json<Value>, (StatusCode, String)> {
    let tag_levels = metrics
        .db
        .write()
        .await
        .get_tag_levels()
        .await
        .map_err(
            |err: sqlx::Error|
                {
                    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
                }
        )?;
    Ok(Json(json!(tag_levels)))
}

async fn _create_result_hashmap(
    peers: Vec<model::Peer>,
    tags: Vec<model::Tag>,
    tag_levels: Vec<model::TagLevel>,
    peer_tags: Vec<model::PeerTag>
) -> Result<(HashMap<i64, HashMap<i64, BackendTag>>, HashMap<String, HashMap<i64, i64>>), sqlx::Error> {
        let mut peers_with_tags: HashMap<String, HashMap<i64, i64>> = HashMap::with_capacity(peers.len());
        for peer in peers {
            peers_with_tags
                .entry(peer.peer_id.clone())
                .or_insert_with(|| {
                    let mut hm = HashMap::with_capacity(tag_levels.len());
                    for tag in peer_tags.iter().filter(|x| x.peer_id == &*peer.peer_id) {
                        if let Some(level) = tags.iter().find(|x| x.id == tag.tag_id) {
                            hm.insert(level.id, tag.tag_id);
                        }
                    }
                    hm
                });
        };
        let mut levels: HashMap<i64, HashMap<i64, BackendTag>> = HashMap::with_capacity(tag_levels.len());
        for level in &tag_levels {
            let tag_list: Vec<model::Tag> = tags.clone().into_iter().filter(|x| x.level == level.id).collect();
            let mut tags_in_level: HashMap<i64, BackendTag> = HashMap::with_capacity(tag_list.len());
            for tag in &tag_list {
                tags_in_level.insert(
                    tag.id,
                    BackendTag {
                        values: tag_list
                                .clone()
                                .into_iter()
                                .map(
                                    |x|
                                    (x.id, model::StoreMetric::new_empty())
                                )
                                .collect()
                    }
                );
            }
            levels.insert(level.id, tags_in_level);
        }
        return Ok((levels, peers_with_tags));
}

fn _parse_output_metrics(
    local_metrics: &HashMap<String, HashMap<String, model::StoreMetric>>,
    peers_with_tags: HashMap<String, HashMap<i64, i64>>,
    levels: &mut HashMap<i64, HashMap<i64, BackendTag>>,
) {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
    for (key, metric) in local_metrics {
        let dst = key;
        for (src, vals) in metric {
            let src_level = peers_with_tags[src].clone();
            let dst_level = peers_with_tags[dst].clone();
            for (level, tag) in src_level {
                if let Some(dst_tag) = dst_level.get(&level) {
                    levels
                        .entry(level)
                        .and_modify(|x|
                            {
                                x
                                    .entry(tag)
                                    .and_modify(|y|
                                        {
                                            y
                                                .values
                                                .entry(*dst_tag)
                                                .and_modify(|z|
                                                    {
                                                        z.ts = ts.clone();
                                                        z.pl += vals.pl;
                                                        if z.jitter_min == 0.0 || z.jitter_min > vals.jitter_min {
                                                            z.jitter_min = vals.jitter_min;
                                                        }
                                                        if z.jitter_max == 0.0 || z.jitter_max < vals.jitter_max {
                                                            z.jitter_max = vals.jitter_max;
                                                        }
                                                        z.jitter_stddev += vals.jitter_stddev;
                                                        if z.jitter_stddev != vals.jitter_stddev {
                                                            z.jitter_stddev /= 2.0;
                                                        };
                                                    });
                                        }
                                    );
                            }
                        );
                }
            }
        }
    }
}

pub async fn render_results(
    metrics: Arc<RwLock<HashMap<String, HashMap<String, model::StoreMetric>>>>,
    mut db: Box<dyn model::db::Db>,
    reconcile_time: u16,
    output_metrics: Arc<RwLock<HashMap<i64, HashMap<i64, BackendTag>>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let sleep_duration = Duration::from_secs(reconcile_time.into());
    loop {
        async_time::sleep(sleep_duration).await;
        let peers = db.get_peers().await?;
        let tags = db.get_tags().await?;
        let tag_levels = db.get_tag_levels().await?;
        let peer_tags = db.get_peer_tags().await?;
        let (mut levels, peers_with_tags) = _create_result_hashmap(peers, tags, tag_levels, peer_tags).await?;

        {
            let local_metrics = metrics.read().await;
            _parse_output_metrics(&*local_metrics, peers_with_tags, &mut levels);
        }
        {
            let mut new_output_metrics = output_metrics.write().await;
            *new_output_metrics = levels;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        routing::get,
        http::{Request, StatusCode},
    };
    use crate::model::db::Db;
    use std::net::SocketAddr;
    /* use aimsir::{
        // self,
        aimsir_service_server::AimsirServiceServer,
        aimsir_service_client::AimsirServiceClient,
    }; */
    use tonic;
    use tokio::{
        self,
        sync::mpsc,
    };
    use tokio_stream;
    use tower::ServiceExt;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn test_get_stats() {
        let storemetric = model::StoreMetric{
            jitter_min: 0.0,
            jitter_max: 1.0,
            jitter_stddev: 0.5,
            pl: 5,
            ts: 1,
        };
        let backendtag = BackendTag{
            values: HashMap::from([(2, storemetric)]),
        };
        let metrics = Arc::new(RwLock::new(HashMap::new()));
        metrics.write().await.insert(0, HashMap::from([(1, backendtag)]));
        let db = Arc::new(RwLock::new(model::db::SqliteDb::new("sqlite://diesel.sqlite".to_string()).await.unwrap()));
        let backend = BackendState{
            metrics: metrics.clone(),
            db,
        };
        let app = Router::new()
            .route("/stats", get(stats))
            .with_state(backend);
        let request = Request::builder().uri("/stats").body(Body::empty()).unwrap();
        let response = app
                        .oneshot(request)
                        .await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!(*metrics.read().await));
    }
    #[tokio::test]
    async fn test_get_stats_id_non_existent() {
        let storemetric = model::StoreMetric{
            jitter_min: 0.0,
            jitter_max: 1.0,
            jitter_stddev: 0.5,
            pl: 5,
            ts: 1,
        };
        let backendtag = BackendTag{
            values: HashMap::from([(2, storemetric)]),
        };
        let metrics = Arc::new(RwLock::new(HashMap::new()));
        metrics.write().await.insert(0, HashMap::from([(1, backendtag)]));
        let db = Arc::new(RwLock::new(model::db::SqliteDb::new("sqlite://diesel.sqlite".to_string()).await.unwrap()));
        let backend = BackendState{
            metrics: metrics.clone(),
            db,
        };
        let app = Router::new()
            .route("/stats", get(stats))
            .route("/stats/:statid", get(stats_id))
            .with_state(backend);
        let request = Request::builder().uri("/stats/10").body(Body::empty()).unwrap();
        let response = app
                        .oneshot(request)
                        .await.expect("ERR");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body, b"Id not found"[..]);
    }
    #[tokio::test]
    async fn test_get_stats_id_exists() {
        let storemetric = model::StoreMetric{
            jitter_min: 0.0,
            jitter_max: 1.0,
            jitter_stddev: 0.5,
            pl: 5,
            ts: 1,
        };
        let backendtag = BackendTag{
            values: HashMap::from([(2, storemetric)]),
        };
        let metrics = Arc::new(RwLock::new(HashMap::new()));
        metrics.write().await.insert(0, HashMap::from([(1, backendtag)]));
        let db = Arc::new(RwLock::new(model::db::SqliteDb::new("sqlite://diesel.sqlite".to_string()).await.unwrap()));
        let backend = BackendState{
            metrics: metrics.clone(),
            db,
        };
        let app = Router::new()
            .route("/stats", get(stats))
            .route("/stats/:statid", get(stats_id))
            .with_state(backend);
        let request = Request::builder().uri("/stats/0").body(Body::empty()).unwrap();
        let response = app
                        .oneshot(request)
                        .await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!(*metrics.read().await.get(&0).unwrap()));
    }
    #[tokio::test]
    async fn test_get_levels() {
        let storemetric = model::StoreMetric{
            jitter_min: 0.0,
            jitter_max: 1.0,
            jitter_stddev: 0.5,
            pl: 5,
            ts: 1,
        };
        let backendtag = BackendTag{
            values: HashMap::from([(2, storemetric)]),
        };
        let metrics = Arc::new(RwLock::new(HashMap::new()));
        metrics.write().await.insert(0, HashMap::from([(1, backendtag)]));
        let db = Arc::new(RwLock::new(model::db::SqliteDb::new("sqlite://diesel.sqlite".to_string()).await.unwrap()));
        let backend = BackendState{
            metrics: metrics.clone(),
            db,
        };
        let app = Router::new()
            .route("/stats", get(stats))
            .route("/stats/:statid", get(stats_id))
            .with_state(backend);
        let request = Request::builder().uri("/stats/0").body(Body::empty()).unwrap();
        let response = app
                        .oneshot(request)
                        .await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!(*metrics.read().await.get(&0).unwrap()));
    }
}
