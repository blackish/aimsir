use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
};
use log;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{self, sync::RwLock, time as async_time};

use crate::model;
use crate::model::aimsir::aimsir_service_client::AimsirServiceClient;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct BackendTag {
    values: HashMap<i32, model::StoreMetric>,
}

#[derive(Clone)]
pub struct BackendState {
    pub metrics: Arc<RwLock<HashMap<i32, BackendTag>>>,
    pub db: Arc<RwLock<dyn model::db::Db + Send + Sync>>,
    pub grpc_server: Arc<RwLock<String>>,
    pub atomic_metrics: Arc<RwLock<HashMap<String, HashMap<String, model::StoreMetric>>>>,
}

pub async fn get_full_metrics(
    State(metrics): State<BackendState>,
) -> Result<String, (StatusCode, String)> {
    let mut result = String::new();
    let local_metrics = metrics.atomic_metrics.read().await;
    for (dst, metric) in &*local_metrics {
        for (src, store_metric) in metric {
            if (store_metric.jitter_max > -1.0
                && store_metric.jitter_min > -1.0
                && store_metric.jitter_stddev > -1.0)
                || (store_metric.ts > 0)
            {
                let s = format!(
                    "pl_gauge{{src=\"{}\", dst=\"{}\", service=\"aimsir\"}} {} {}\n",
                    src, dst, store_metric.pl, store_metric.ts
                );
                result = result + &s;
            }
            if store_metric.jitter_min > -1.0 {
                let s = format!(
                    "jitter_min_gauge{{src=\"{}\", dst=\"{}\", service=\"aimsir\"}} {} {}\n",
                    src, dst, store_metric.jitter_min, store_metric.ts
                );
                result = result + &s;
            }
            if store_metric.jitter_max > -1.0 {
                let s = format!(
                    "jitter_max_gauge{{src=\"{}\", dst=\"{}\", service=\"aimsir\"}} {} {}\n",
                    src, dst, store_metric.jitter_max, store_metric.ts
                );
                result = result + &s;
            }
            if store_metric.jitter_stddev > -1.0 {
                let s = format!(
                    "jitter_stddev_gauge{{src=\"{}\", dst=\"{}\", service=\"aimsir\"}} {} {}\n",
                    src, dst, store_metric.jitter_stddev, store_metric.ts
                );
                result = result + &s;
            }
        }
    }
    Ok(result)
}

pub async fn get_metrics(
    State(metrics): State<BackendState>,
) -> Result<String, (StatusCode, String)> {
    let tags = metrics
        .db
        .write()
        .await
        .get_tags()
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let mut tag_maps: HashMap<i32, String> = HashMap::new();
    for tag in tags {
        tag_maps.insert(tag.id.unwrap(), tag.name);
    }
    let mut result = String::new();
    let local_metrics = metrics.metrics.read().await;
    for (src, metric) in &*local_metrics {
        for (dst, store_metric) in &metric.values {
            if (store_metric.jitter_max > -1.0
                && store_metric.jitter_min > -1.0
                && store_metric.jitter_stddev > -1.0)
                || (store_metric.ts > 0)
            {
                let s = format!(
                    "pl_gauge{{src=\"{}\", dst=\"{}\", service=\"aimsir\"}} {} {}\n",
                    tag_maps.get(src).unwrap_or(&String::from("unknown")),
                    tag_maps.get(dst).unwrap_or(&String::from("unknown")),
                    store_metric.pl,
                    store_metric.ts
                );
                result = result + &s;
            }
            if store_metric.jitter_min > -1.0 {
                let s = format!(
                    "jitter_min_gauge{{src=\"{}\", dst=\"{}\", service=\"aimsir\"}} {} {}\n",
                    tag_maps.get(src).unwrap(),
                    tag_maps.get(dst).unwrap(),
                    store_metric.jitter_min,
                    store_metric.ts
                );
                result = result + &s;
            }
            if store_metric.jitter_max > -1.0 {
                let s = format!(
                    "jitter_max_gauge{{src=\"{}\", dst=\"{}\", service=\"aimsir\"}} {} {}\n",
                    tag_maps.get(src).unwrap(),
                    tag_maps.get(dst).unwrap(),
                    store_metric.jitter_max,
                    store_metric.ts
                );
                result = result + &s;
            }
            if store_metric.jitter_stddev > -1.0 {
                let s = format!(
                    "jitter_stddev_gauge{{src=\"{}\", dst=\"{}\", service=\"aimsir\"}} {} {}\n",
                    tag_maps.get(src).unwrap(),
                    tag_maps.get(dst).unwrap(),
                    store_metric.jitter_stddev,
                    store_metric.ts
                );
                result = result + &s;
            }
        }
    }
    Ok(result)
}

pub async fn healthz(
    State(_metrics): State<BackendState>,
) -> Result<(), (StatusCode, String)> {
    Ok(())
}

pub async fn stats(
    State(metrics): State<BackendState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let result = metrics.metrics.read().await;
    Ok(Json(json!(*result)))
}

pub async fn stats_id(
    Path(id): Path<i32>,
    State(metrics): State<BackendState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if let Some(stats) = metrics.metrics.read().await.get(&id) {
        Ok(Json(json!(stats)))
    } else {
        Err((StatusCode::NOT_FOUND, "Id not found".to_string()))
    }
}

pub async fn tags(
    State(metrics): State<BackendState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let result = metrics
        .db
        .write()
        .await
        .get_tags()
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(json!(*result)))
}

pub async fn peers(
    State(metrics): State<BackendState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let result = metrics
        .db
        .write()
        .await
        .get_peers()
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(json!(*result)))
}

pub async fn peer_tags(
    State(metrics): State<BackendState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let result = metrics
        .db
        .write()
        .await
        .get_peer_tags()
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(json!(*result)))
}

pub async fn add_peer(
    State(metrics): State<BackendState>,
    Json(peer): Json<model::Peer>,
) -> Result<(), (StatusCode, String)> {
    let mut client =
        AimsirServiceClient::connect(String::from(metrics.grpc_server.read().await.clone()))
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to connect to gRPC server: {}", err.to_string()),
                )
            })?;
    client
        .add_peer(model::aimsir::Peer {
            id: peer.peer_id.clone(),
            ipaddress: "".into(),
        })
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to add peer to the server {}", err.to_string()),
            )
        })?;
    metrics
        .db
        .write()
        .await
        .add_peer(peer)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

pub async fn del_peer(
    Path(peer): Path<String>,
    State(metrics): State<BackendState>,
) -> Result<(), (StatusCode, String)> {
    let mut client =
        AimsirServiceClient::connect(String::from(metrics.grpc_server.read().await.clone()))
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to connect to gRPC server: {}", err.to_string()),
                )
            })?;
    // peer might not exists in the server
    let _ = client
        .remove_peer(model::aimsir::Peer {
            id: peer.clone(),
            ipaddress: "".into(),
        })
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to remove peer from the server {}", err.to_string()),
            )
        });
    metrics
        .db
        .write()
        .await
        .del_peer(peer)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

pub async fn disable_peer(
    Path(peer): Path<String>,
    State(metrics): State<BackendState>,
) -> Result<(), (StatusCode, String)> {
    metrics
        .db
        .write()
        .await
        .disable_peer(peer)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

pub async fn enable_peer(
    Path(peer): Path<String>,
    State(metrics): State<BackendState>,
) -> Result<(), (StatusCode, String)> {
    metrics
        .db
        .write()
        .await
        .enable_peer(peer)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

pub async fn add_tag(
    State(metrics): State<BackendState>,
    Json(tag): Json<model::Tag>,
) -> Result<(), (StatusCode, String)> {
    metrics
        .db
        .write()
        .await
        .add_tag(tag)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

pub async fn del_tag(
    Path(tag): Path<i32>,
    State(metrics): State<BackendState>,
) -> Result<(), (StatusCode, String)> {
    metrics
        .db
        .write()
        .await
        .del_tag(tag)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

pub async fn add_peer_tag(
    State(metrics): State<BackendState>,
    Json(peer_tag): Json<model::PeerTag>,
) -> Result<(), (StatusCode, String)> {
    metrics
        .db
        .write()
        .await
        .add_peer_tag(peer_tag)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

pub async fn del_peer_tag(
    Path((peer_id, tag_id)): Path<(String, i32)>,
    State(metrics): State<BackendState>,
) -> Result<(), (StatusCode, String)> {
    metrics
        .db
        .write()
        .await
        .del_peer_tag(peer_id, tag_id)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

async fn _create_result_hashmap(
    peers: &Vec<model::Peer>,
    tags: Vec<model::Tag>,
    peer_tags: Vec<model::PeerTag>,
) -> Result<(HashMap<i32, BackendTag>, HashMap<String, Vec<model::Tag>>), sqlx::Error> {
    let mut peers_with_tags: HashMap<String, Vec<model::Tag>> = HashMap::with_capacity(peers.len());
    for peer in peers {
        peers_with_tags
            .entry(peer.peer_id.clone())
            .or_insert_with(|| {
                peer_tags
                    .iter()
                    .filter(|x| x.peer_id == &*peer.peer_id)
                    .map(|x| {
                        tags.iter()
                            .find(|y| y.id.unwrap() == x.tag_id)
                            .unwrap_or(&model::Tag {
                                id: None,
                                parent: None,
                                name: "Not exists".into(),
                            })
                            .clone()
                    })
                    .collect()
            });
    }
    let mut levels: HashMap<i32, BackendTag> = HashMap::new();
    for level in &tags {
        let tag_list: Vec<model::Tag> = tags
            .clone()
            .into_iter()
            .filter(|x| x.parent == level.parent)
            .collect();
        levels.insert(
            level.id.unwrap(),
            BackendTag {
                values: tag_list
                    .clone()
                    .into_iter()
                    .map(|x| (x.id.unwrap(), model::StoreMetric::new_empty()))
                    .collect(),
            },
        );
    }
    return Ok((levels, peers_with_tags));
}

fn _parse_output_metrics(
    local_metrics: &HashMap<String, HashMap<String, model::StoreMetric>>,
    peers_with_tags: HashMap<String, Vec<model::Tag>>,
    levels: &mut HashMap<i32, BackendTag>,
    reconcile_time: u16,
    maintenance_peers: Vec<String>,
) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    for (dst, metric) in local_metrics {
        let dst_level = peers_with_tags.get(dst);
        // If we receive a metric for already deleted peer or for peer in maintenance, ignore it
        if maintenance_peers.contains(dst) || dst_level.is_none() {
            continue;
        }
        let dst_level = dst_level.unwrap();
        for (src, vals) in metric {
            // If we receive a metric for already deleted peer or for peer in maintenance, ignore it
            if maintenance_peers.contains(src) {
                continue;
            }
            let src_level = peers_with_tags.get(src);
            if src_level.is_none() {
                continue;
            }
            let src_level = src_level.unwrap();
            for level in src_level {
                if let Some(src_id) = level.id {
                    // if let Some(dst_tag) = dst_level.iter().find(|x| level.parent == x.parent && level.id != x.id) {
                    for dst_tag in dst_level.iter().filter(|x| level.parent == x.parent) {
                        if let Some(dst_id) = dst_tag.id {
                            levels.entry(src_id).and_modify(|x| {
                                x.values.entry(dst_id).and_modify(|z| {
                                    // we skip stale metrics
                                    if ts - z.ts <= ((reconcile_time as u64) * 1500) || z.ts == 0 {
                                        if z.ts < vals.ts {
                                            z.ts = vals.ts.clone();
                                        }
                                        z.pl += vals.pl;
                                        if z.jitter_min == -1.0 || z.jitter_min > vals.jitter_min {
                                            z.jitter_min = vals.jitter_min;
                                        }
                                        if z.jitter_max == -1.0 || z.jitter_max < vals.jitter_max {
                                            z.jitter_max = vals.jitter_max;
                                        }
                                        if z.jitter_stddev == -1.0 {
                                            z.jitter_stddev = 0.0;
                                        }
                                        z.jitter_stddev += vals.jitter_stddev;
                                        if z.jitter_stddev != vals.jitter_stddev {
                                            z.jitter_stddev /= 2.0;
                                        };
                                    };
                                });
                            });
                        }
                    }
                }
            }
        }
    }
}

pub async fn render_results(
    metrics: Arc<RwLock<HashMap<String, HashMap<String, model::StoreMetric>>>>,
    mut db: Box<dyn model::db::Db>,
    reconcile_time: u16,
    output_metrics: Arc<RwLock<HashMap<i32, BackendTag>>>,
    atomic_metrics: Arc<RwLock<HashMap<String, HashMap<String, model::StoreMetric>>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let sleep_duration = Duration::from_secs(reconcile_time.into());
    loop {
        async_time::sleep(sleep_duration).await;
        log::debug!("Updating output metrics");
        if let Ok(peers) = db.get_peers().await {
            if let Ok(tags) = db.get_tags().await {
                if let Ok(peer_tags) = db.get_peer_tags().await {
                    if let Ok((mut levels, peers_with_tags)) =
                        _create_result_hashmap(&peers, tags, peer_tags).await
                    {
                        {
                            let maintenance_peers: Vec<String> = peers
                                .into_iter()
                                .filter(|x| x.maintenance.unwrap_or(0) != 0)
                                .map(|x| x.peer_id)
                                .collect();
                            {
                                let mut atomic_metrics_to_update = atomic_metrics.write().await;
                                let local_metrics = metrics.read().await;
                                *atomic_metrics_to_update = local_metrics.clone();
                            }
                            let local_metrics = atomic_metrics.read().await;
                            _parse_output_metrics(
                                &*local_metrics,
                                peers_with_tags,
                                &mut levels,
                                reconcile_time,
                                maintenance_peers,
                            );
                        }
                        {
                            let mut new_output_metrics = output_metrics.write().await;
                            *new_output_metrics = levels;
                        }
                    } else {
                        log::warn!("Failed to create result hashmap");
                    }
                } else {
                    log::warn!("Failed to get peer tags");
                }
            } else {
                log::warn!("Failed to get tags");
            }
        } else {
            log::warn!("Failed to get peers");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::db::Db;

    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::{delete, get, post},
        Router,
    };
    use dotenv;
    use http_body_util::BodyExt;
    use model::aimsir::aimsir_service_server::AimsirService;
    use std::env;
    use std::net::SocketAddr;
    use tokio::{self, sync::mpsc};
    use tokio_stream::wrappers::ReceiverStream;
    use tower::ServiceExt;

    struct TestServer {
        tx: mpsc::Sender<model::aimsir::Peer>,
    }

    #[tonic::async_trait]
    impl AimsirService for TestServer {
        type RegisterStream = ReceiverStream<Result<model::aimsir::PeerUpdate, tonic::Status>>;
        type MetricsStream = ReceiverStream<Result<model::aimsir::MetricResponse, tonic::Status>>;
        async fn ping(
            &self,
            _request: tonic::Request<()>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            Ok(tonic::Response::new(()))
        }
        async fn metrics(
            &self,
            _request: tonic::Request<tonic::Streaming<model::aimsir::MetricMessage>>,
        ) -> std::result::Result<tonic::Response<Self::MetricsStream>, tonic::Status> {
            let (tx, rx) = mpsc::channel(1);
            tokio::spawn(async move {
                loop {
                    _ = tx
                        .send(Ok(model::aimsir::MetricResponse { ok: true }))
                        .await;
                }
            });
            Ok(tonic::Response::new(ReceiverStream::new(rx)))
        }
        async fn register(
            &self,
            _request: tonic::Request<model::aimsir::Peer>,
        ) -> std::result::Result<tonic::Response<Self::RegisterStream>, tonic::Status> {
            let (_tx, rx) = mpsc::channel(1);
            Ok(tonic::Response::new(ReceiverStream::new(rx)))
        }
        async fn add_peer(
            &self,
            request: tonic::Request<model::aimsir::Peer>,
        ) -> std::result::Result<tonic::Response<model::aimsir::PeerResponse>, tonic::Status>
        {
            let _ = self.tx.send(request.into_inner()).await;
            Ok(tonic::Response::new(model::aimsir::PeerResponse {
                ok: true,
            }))
        }
        async fn remove_peer(
            &self,
            request: tonic::Request<model::aimsir::Peer>,
        ) -> std::result::Result<tonic::Response<model::aimsir::PeerResponse>, tonic::Status>
        {
            let _ = self.tx.send(request.into_inner()).await;
            Ok(tonic::Response::new(model::aimsir::PeerResponse {
                ok: true,
            }))
        }
    }

    #[tokio::test]
    async fn test_get_stats() {
        dotenv::dotenv().expect("Could not load the .env file!");
        let database_url =
            env::var("DATABASE_URL").expect("The environment variable DATABASE_URL is missing!");
        let storemetric = model::StoreMetric {
            jitter_min: 0.0,
            jitter_max: 1.0,
            jitter_stddev: 0.5,
            pl: 5,
            ts: 1,
        };
        let backendtag = BackendTag {
            values: HashMap::from([(2, storemetric)]),
        };
        let metrics = Arc::new(RwLock::new(HashMap::new()));
        metrics.write().await.insert(0, backendtag);
        let atomic_metrics = Arc::new(RwLock::new(HashMap::new()));
        let db = Arc::new(RwLock::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        ));
        let backend = BackendState {
            metrics: metrics.clone(),
            db,
            grpc_server: Arc::new(RwLock::new("http://127.0.0.1:10000".into())),
            atomic_metrics: atomic_metrics.clone(),
        };
        let app = Router::new()
            .route("/stats", get(stats))
            .with_state(backend);
        let request = Request::builder()
            .uri("/stats")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!(*metrics.read().await));
    }
    #[tokio::test]
    async fn test_get_stats_id_non_existent() {
        dotenv::dotenv().expect("Could not load the .env file!");
        let database_url =
            env::var("DATABASE_URL").expect("The environment variable DATABASE_URL is missing!");
        let storemetric = model::StoreMetric {
            jitter_min: 0.0,
            jitter_max: 1.0,
            jitter_stddev: 0.5,
            pl: 5,
            ts: 1,
        };
        let backendtag = BackendTag {
            values: HashMap::from([(2, storemetric)]),
        };
        let metrics = Arc::new(RwLock::new(HashMap::new()));
        metrics.write().await.insert(0, backendtag);
        let atomic_metrics = Arc::new(RwLock::new(HashMap::new()));
        let db = Arc::new(RwLock::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        ));
        let backend = BackendState {
            metrics: metrics.clone(),
            db,
            grpc_server: Arc::new(RwLock::new("http://127.0.0.1:10000".into())),
            atomic_metrics: atomic_metrics.clone(),
        };
        let app = Router::new()
            .route("/stats/:statid", get(stats_id))
            .with_state(backend);
        let request = Request::builder()
            .uri("/stats/10")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body, b"Id not found"[..]);
    }
    #[tokio::test]
    async fn test_get_stats_id_exists() {
        dotenv::dotenv().expect("Could not load the .env file!");
        let database_url =
            env::var("DATABASE_URL").expect("The environment variable DATABASE_URL is missing!");
        let storemetric = model::StoreMetric {
            jitter_min: 0.0,
            jitter_max: 1.0,
            jitter_stddev: 0.5,
            pl: 5,
            ts: 1,
        };
        let backendtag = BackendTag {
            values: HashMap::from([(2, storemetric)]),
        };
        let metrics = Arc::new(RwLock::new(HashMap::new()));
        metrics.write().await.insert(0, backendtag);
        let atomic_metrics = Arc::new(RwLock::new(HashMap::new()));
        let db = Arc::new(RwLock::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        ));
        let backend = BackendState {
            metrics: metrics.clone(),
            db,
            grpc_server: Arc::new(RwLock::new("http://127.0.0.1:10000".into())),
            atomic_metrics: atomic_metrics.clone(),
        };
        let app = Router::new()
            .route("/stats/:statid", get(stats_id))
            .with_state(backend);
        let request = Request::builder()
            .uri("/stats/0")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!(*metrics.read().await.get(&0).unwrap()));
    }
    #[tokio::test]
    async fn test_api() {
        dotenv::dotenv().expect("Could not load the .env file!");
        let database_url =
            env::var("DATABASE_URL").expect("The environment variable DATABASE_URL is missing!");
        let storemetric = model::StoreMetric {
            jitter_min: 0.0,
            jitter_max: 1.0,
            jitter_stddev: 0.5,
            pl: 5,
            ts: 1,
        };

        let addr: SocketAddr = "127.0.0.1:10000".parse().unwrap();
        let (tx, mut rx) = mpsc::channel::<model::aimsir::Peer>(1);
        let svc = TestServer { tx };
        let server = model::aimsir::aimsir_service_server::AimsirServiceServer::new(svc);
        tokio::spawn(async move {
            let _ = tonic::transport::Server::builder()
                .add_service(server)
                .serve(addr)
                .await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let backendtag = BackendTag {
            values: HashMap::from([(2, storemetric.clone())]),
        };
        let metrics = Arc::new(RwLock::new(HashMap::new()));
        let atomic_metrics = Arc::new(RwLock::new(HashMap::new()));
        metrics.write().await.insert(0, backendtag);
        {
            let mut local_atomic_metrics = atomic_metrics.write().await;
            local_atomic_metrics
                .insert("0".into(), HashMap::from_iter([("1".into(), storemetric)]));
        }
        let db = Arc::new(RwLock::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        ));
        let backend = BackendState {
            metrics: metrics.clone(),
            db: db.clone(),
            grpc_server: Arc::new(RwLock::new("http://127.0.0.1:10000".into())),
            atomic_metrics: atomic_metrics.clone(),
        };
        let app = Router::new()
            .route("/peers", get(peers))
            .route("/peers", post(add_peer))
            .route("/peers/:peer", delete(del_peer))
            .route("/disable-peer/:peer", get(disable_peer))
            .route("/enable-peer/:peer", get(enable_peer))
            .route("/tags", get(tags))
            .route("/tags", post(add_tag))
            .route("/tags/:tag", delete(del_tag))
            .route("/peertags", get(peer_tags))
            .route("/peertags", post(add_peer_tag))
            .route("/peertags/:peer/:tag", delete(del_peer_tag))
            .route("/metrics", get(get_metrics))
            .route("/fullmetrics", get(get_full_metrics))
            .with_state(backend);
        // Creating new peer
        let new_peer = model::Peer {
            peer_id: "0".into(),
            name: "name".into(),
            maintenance: None,
        };
        let request = Request::builder()
            .method("POST")
            .header("Content-Type", "application/json")
            .uri("/peers")
            .body(Body::from(json!(new_peer).to_string()))
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        let received_peer = rx.recv().await.unwrap();
        assert_eq!(received_peer.id, "0");
        assert_eq!(response.status(), StatusCode::OK);
        let request = Request::builder()
            .method("GET")
            .uri("/peers")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!(vec![new_peer]));

        // Disable peer
        let request = Request::builder()
            .method("GET")
            .uri("/disable-peer/0")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let request = Request::builder()
            .method("GET")
            .uri("/peers")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        let new_peer = model::Peer {
            peer_id: "0".into(),
            name: "name".into(),
            maintenance: Some(1),
        };
        assert_eq!(body, json!(vec![new_peer]));

        // Enable peer
        let request = Request::builder()
            .method("GET")
            .uri("/enable-peer/0")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let request = Request::builder()
            .method("GET")
            .uri("/peers")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        let new_peer = model::Peer {
            peer_id: "0".into(),
            name: "name".into(),
            maintenance: Some(0),
        };
        assert_eq!(body, json!(vec![new_peer]));

        // Creating new tag
        let mut new_tag = model::Tag {
            id: Some(1),
            parent: None,
            name: "Top tag".into(),
        };
        let request = Request::builder()
            .method("POST")
            .header("Content-Type", "application/json")
            .uri("/tags")
            .body(Body::from(json!(new_tag).to_string()))
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let request = Request::builder()
            .method("GET")
            .uri("/tags")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        new_tag.id = Some(0);
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!(vec![new_tag]));

        // Creating new peer tag
        let new_peer_tag = model::PeerTag {
            peer_id: "0".into(),
            tag_id: 0,
        };
        let request = Request::builder()
            .method("POST")
            .header("Content-Type", "application/json")
            .uri("/peertags")
            .body(Body::from(json!(new_peer_tag).to_string()))
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let request = Request::builder()
            .method("GET")
            .uri("/peertags")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!(vec![new_peer_tag]));

        // delete peer_tag
        let request = Request::builder()
            .method("DELETE")
            .uri("/peertags/0/0")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let request = Request::builder()
            .method("GET")
            .uri("/peertags")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!([]));

        // delete peer
        let request = Request::builder()
            .method("DELETE")
            .uri("/peers/0")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        let received_peer = rx.recv().await.unwrap();
        assert_eq!(received_peer.id, "0");
        assert_eq!(response.status(), StatusCode::OK);
        let request = Request::builder()
            .method("GET")
            .uri("/peers")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!([]));

        // delete tag
        let request = Request::builder()
            .method("DELETE")
            .uri("/tags/0")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let request = Request::builder()
            .method("GET")
            .uri("/tags")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!([]));

        // get metrics
        {
            let mut local_db = db.write().await;
            let _ = local_db
                .add_tag(model::Tag {
                    id: Some(0),
                    parent: None,
                    name: "0".into(),
                })
                .await;
            let _ = local_db
                .add_tag(model::Tag {
                    id: Some(1),
                    parent: None,
                    name: "1".into(),
                })
                .await;
            let _ = local_db
                .add_tag(model::Tag {
                    id: Some(2),
                    parent: None,
                    name: "2".into(),
                })
                .await;
        };
        let request = Request::builder()
            .method("GET")
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .iter()
                .as_slice()
                .to_vec(),
        )
        .unwrap();
        let mut local_db = db.write().await;
        let _ = local_db.del_tag(0).await;
        let _ = local_db.del_tag(1).await;
        let _ = local_db.del_tag(2).await;
        assert_eq!(body, String::from("pl_gauge{src=\"0\", dst=\"2\", service=\"aimsir\"} 5 1\njitter_min_gauge{src=\"0\", dst=\"2\", service=\"aimsir\"} 0 1\njitter_max_gauge{src=\"0\", dst=\"2\", service=\"aimsir\"} 1 1\njitter_stddev_gauge{src=\"0\", dst=\"2\", service=\"aimsir\"} 0.5 1\n"));
        let request = Request::builder()
            .method("GET")
            .uri("/fullmetrics")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .iter()
                .as_slice()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(body, String::from("pl_gauge{src=\"1\", dst=\"0\", service=\"aimsir\"} 5 1\njitter_min_gauge{src=\"1\", dst=\"0\", service=\"aimsir\"} 0 1\njitter_max_gauge{src=\"1\", dst=\"0\", service=\"aimsir\"} 1 1\njitter_stddev_gauge{src=\"1\", dst=\"0\", service=\"aimsir\"} 0.5 1\n"));
    }

    #[tokio::test]
    async fn test_create_result_hashmap() {
        let tags: Vec<model::Tag> = vec![
            model::Tag {
                id: Some(0),
                parent: None,
                name: "root_tag1".into(),
            },
            model::Tag {
                id: Some(1),
                parent: None,
                name: "root_tag2".into(),
            },
            model::Tag {
                id: Some(2),
                parent: Some(0),
                name: "child_tag1".into(),
            },
            model::Tag {
                id: Some(3),
                parent: Some(0),
                name: "child_tag2".into(),
            },
        ];
        let peers: Vec<model::Peer> = vec![
            model::Peer {
                peer_id: "0".into(),
                name: "peer0".into(),
                maintenance: None,
            },
            model::Peer {
                peer_id: "1".into(),
                name: "peer1".into(),
                maintenance: None,
            },
            model::Peer {
                peer_id: "2".into(),
                name: "peer2".into(),
                maintenance: None,
            }
        ];
        let peer_tags: Vec<model::PeerTag> = vec![
            model::PeerTag {
                peer_id: "0".into(),
                tag_id: 0,
            },
            model::PeerTag {
                peer_id: "0".into(),
                tag_id: 2,
            },
            model::PeerTag {
                peer_id: "1".into(),
                tag_id: 0,
            },
            model::PeerTag {
                peer_id: "1".into(),
                tag_id: 3,
            },
        ];
        let (levels, peers_with_tags) = _create_result_hashmap(&peers, tags.clone(), peer_tags)
            .await
            .expect("ERR");
        let mut result_levels: HashMap<i32, BackendTag> = HashMap::new();
        result_levels.insert(
            0,
            BackendTag {
                values: HashMap::from([
                    (1, model::StoreMetric::new_empty()),
                    (0, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(
            1,
            BackendTag {
                values: HashMap::from([
                    (0, model::StoreMetric::new_empty()),
                    (1, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(
            2,
            BackendTag {
                values: HashMap::from([
                    (3, model::StoreMetric::new_empty()),
                    (2, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(
            3,
            BackendTag {
                values: HashMap::from([
                    (2, model::StoreMetric::new_empty()),
                    (3, model::StoreMetric::new_empty()),
                ]),
            },
        );
        assert_eq!(result_levels, levels);
        let mut result_peer_with_tags: HashMap<String, Vec<model::Tag>> = HashMap::new();
        result_peer_with_tags.insert("0".into(), vec![tags[0].clone(), tags[2].clone()]);
        result_peer_with_tags.insert("1".into(), vec![tags[0].clone(), tags[3].clone()]);
        result_peer_with_tags.insert("2".into(), Vec::new());
        assert_eq!(result_peer_with_tags, peers_with_tags);
    }
    #[tokio::test]
    async fn test_parse_output_metrics() {
        let tags: Vec<model::Tag> = vec![
            model::Tag {
                id: Some(0),
                parent: None,
                name: "root_tag1".into(),
            },
            model::Tag {
                id: Some(1),
                parent: None,
                name: "root_tag2".into(),
            },
            model::Tag {
                id: Some(2),
                parent: Some(0),
                name: "child_tag1".into(),
            },
            model::Tag {
                id: Some(3),
                parent: Some(0),
                name: "child_tag2".into(),
            },
        ];
        let mut result_levels: HashMap<i32, BackendTag> = HashMap::new();
        result_levels.insert(
            0,
            BackendTag {
                values: HashMap::from([
                    (1, model::StoreMetric::new_empty()),
                    (0, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(
            1,
            BackendTag {
                values: HashMap::from([
                    (0, model::StoreMetric::new_empty()),
                    (1, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(
            2,
            BackendTag {
                values: HashMap::from([
                    (3, model::StoreMetric::new_empty()),
                    (2, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(
            3,
            BackendTag {
                values: HashMap::from([
                    (2, model::StoreMetric::new_empty()),
                    (3, model::StoreMetric::new_empty()),
                ]),
            },
        );
        let mut result_peer_with_tags: HashMap<String, Vec<model::Tag>> = HashMap::new();
        result_peer_with_tags.insert("0".into(), vec![tags[0].clone(), tags[2].clone()]);
        result_peer_with_tags.insert("1".into(), vec![tags[0].clone(), tags[3].clone()]);
        let mut local_metrics: HashMap<String, HashMap<String, model::StoreMetric>> =
            HashMap::new();
        let mut levels = result_levels.clone();
        local_metrics.insert(
            "0".into(),
            HashMap::from([("1".into(), model::StoreMetric::new_empty())]),
        );
        local_metrics.insert(
            "1".into(),
            HashMap::from([("0".into(), model::StoreMetric::new_empty())]),
        );
        local_metrics.entry("0".into()).and_modify(|x| {
            x.entry("1".into()).and_modify(|y| {
                y.ts = 0;
                y.jitter_min = 1.0;
                y.jitter_max = 1.0;
                y.jitter_stddev = 1.0;
            });
        });
        local_metrics.entry("1".into()).and_modify(|x| {
            x.entry("0".into()).and_modify(|y| {
                y.ts = 0;
                y.jitter_min = 1.0;
                y.jitter_max = 2.0;
                y.jitter_stddev = 2.0;
            });
        });
        _parse_output_metrics(
            &local_metrics,
            result_peer_with_tags,
            &mut levels,
            60,
            Vec::new(),
        );
        result_levels.entry(3).and_modify(|x| {
            x.values.entry(2).and_modify(|z| {
                z.ts = levels.get(&3).unwrap().values.get(&2).unwrap().ts;
                z.jitter_stddev = 1.0;
                z.jitter_max = 1.0;
                z.jitter_min = 1.0;
            });
        });
        result_levels.entry(2).and_modify(|x| {
            x.values.entry(3).and_modify(|z| {
                z.ts = levels.get(&2).unwrap().values.get(&3).unwrap().ts;
                z.jitter_stddev = 2.0;
                z.jitter_max = 2.0;
                z.jitter_min = 1.0;
            });
        });
        result_levels.entry(0).and_modify(|x| {
            x.values.entry(0).and_modify(|z| {
                z.ts = levels.get(&0).unwrap().values.get(&0).unwrap().ts;
                z.jitter_stddev = 1.5;
                z.jitter_max = 2.0;
                z.jitter_min = 1.0;
            });
        });
        assert_eq!(levels, result_levels);
    }
    #[tokio::test]
    async fn test_parse_output_metrics_maintenance() {
        let tags: Vec<model::Tag> = vec![
            model::Tag {
                id: Some(0),
                parent: None,
                name: "root_tag1".into(),
            },
            model::Tag {
                id: Some(1),
                parent: None,
                name: "root_tag2".into(),
            },
            model::Tag {
                id: Some(2),
                parent: Some(0),
                name: "child_tag1".into(),
            },
            model::Tag {
                id: Some(3),
                parent: Some(0),
                name: "child_tag2".into(),
            },
        ];
        let mut result_levels: HashMap<i32, BackendTag> = HashMap::new();
        result_levels.insert(
            0,
            BackendTag {
                values: HashMap::from([
                    (1, model::StoreMetric::new_empty()),
                    (0, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(
            1,
            BackendTag {
                values: HashMap::from([
                    (0, model::StoreMetric::new_empty()),
                    (1, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(
            2,
            BackendTag {
                values: HashMap::from([
                    (3, model::StoreMetric::new_empty()),
                    (2, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(
            3,
            BackendTag {
                values: HashMap::from([
                    (2, model::StoreMetric::new_empty()),
                    (3, model::StoreMetric::new_empty()),
                ]),
            },
        );
        let mut result_peer_with_tags: HashMap<String, Vec<model::Tag>> = HashMap::new();
        result_peer_with_tags.insert("0".into(), vec![tags[0].clone(), tags[2].clone()]);
        result_peer_with_tags.insert("1".into(), vec![tags[0].clone(), tags[3].clone()]);
        let mut local_metrics: HashMap<String, HashMap<String, model::StoreMetric>> =
            HashMap::new();
        let mut levels = result_levels.clone();
        local_metrics.insert(
            "0".into(),
            HashMap::from([("1".into(), model::StoreMetric::new_empty())]),
        );
        local_metrics.insert(
            "1".into(),
            HashMap::from([("0".into(), model::StoreMetric::new_empty())]),
        );
        local_metrics.entry("0".into()).and_modify(|x| {
            x.entry("1".into()).and_modify(|y| {
                y.ts = 0;
                y.jitter_min = 1.0;
                y.jitter_max = 1.0;
                y.jitter_stddev = 1.0;
            });
        });
        local_metrics.entry("1".into()).and_modify(|x| {
            x.entry("0".into()).and_modify(|y| {
                y.ts = 0;
                y.jitter_min = 1.0;
                y.jitter_max = 2.0;
                y.jitter_stddev = 2.0;
            });
        });
        _parse_output_metrics(
            &local_metrics,
            result_peer_with_tags,
            &mut levels,
            60,
            vec![String::from("0")],
        );
        result_levels.entry(3).and_modify(|x| {
            x.values.entry(2).and_modify(|z| {
                z.ts = levels.get(&3).unwrap().values.get(&2).unwrap().ts;
                z.jitter_stddev = -1.0;
                z.jitter_max = -1.0;
                z.jitter_min = -1.0;
            });
        });
        result_levels.entry(2).and_modify(|x| {
            x.values.entry(3).and_modify(|z| {
                z.ts = levels.get(&2).unwrap().values.get(&3).unwrap().ts;
                z.jitter_stddev = -1.0;
                z.jitter_max = -1.0;
                z.jitter_min = -1.0;
            });
        });
        result_levels.entry(0).and_modify(|x| {
            x.values.entry(0).and_modify(|z| {
                z.ts = levels.get(&0).unwrap().values.get(&0).unwrap().ts;
                z.jitter_stddev = -1.0;
                z.jitter_max = -1.0;
                z.jitter_min = -1.0;
            });
        });
        assert_eq!(levels, result_levels);
    }
}
