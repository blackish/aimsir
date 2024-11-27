use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
};
use log;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{self, sync::RwLock, time as async_time};

use crate::model;
use crate::model::aimsir::aimsir_service_client::AimsirServiceClient;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct BackendTag {
    values: HashMap<i32, model::StoreMetric>,
}

#[derive(Clone)]
pub struct BackendState {
    pub metrics: Arc<RwLock<HashMap<i32, HashMap<i32, BackendTag>>>>,
    pub db: Arc<RwLock<dyn model::db::Db + Send + Sync>>,
    pub grpc_server: Arc<RwLock<String>>,
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

pub async fn tag_levels(
    State(metrics): State<BackendState>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let result = metrics
        .db
        .write()
        .await
        .get_tag_levels()
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
    client
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
        })?;
    metrics
        .db
        .write()
        .await
        .del_peer(peer)
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

pub async fn add_tag_level(
    State(metrics): State<BackendState>,
    Json(tag_level): Json<model::TagLevel>,
) -> Result<(), (StatusCode, String)> {
    metrics
        .db
        .write()
        .await
        .add_tag_level(tag_level)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

pub async fn del_tag_level(
    Path(tag_level): Path<i32>,
    State(metrics): State<BackendState>,
) -> Result<(), (StatusCode, String)> {
    metrics
        .db
        .write()
        .await
        .del_tag_level(tag_level)
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

pub async fn levels(metrics: &State<BackendState>) -> Result<Json<Value>, (StatusCode, String)> {
    let tag_levels = metrics
        .db
        .write()
        .await
        .get_tag_levels()
        .await
        .map_err(|err: sqlx::Error| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(json!(tag_levels)))
}

async fn _create_result_hashmap(
    peers: Vec<model::Peer>,
    tags: Vec<model::Tag>,
    tag_levels: Vec<model::TagLevel>,
    peer_tags: Vec<model::PeerTag>,
) -> Result<
    (
        HashMap<i32, HashMap<i32, BackendTag>>,
        HashMap<String, HashMap<i32, i32>>,
    ),
    sqlx::Error,
> {
    let mut peers_with_tags: HashMap<String, HashMap<i32, i32>> =
        HashMap::with_capacity(peers.len());
    for peer in peers {
        peers_with_tags
            .entry(peer.peer_id.clone())
            .or_insert_with(|| {
                let mut hm = HashMap::with_capacity(tag_levels.len());
                for tag in peer_tags.iter().filter(|x| x.peer_id == &*peer.peer_id) {
                    if let Some(level) = tags.iter().find(|x| x.id.unwrap() == tag.tag_id) {
                        hm.insert(level.level, tag.tag_id);
                    }
                }
                hm
            });
    }
    let mut levels: HashMap<i32, HashMap<i32, BackendTag>> =
        HashMap::with_capacity(tag_levels.len());
    for level in &tag_levels {
        let tag_list: Vec<model::Tag> = tags
            .clone()
            .into_iter()
            .filter(|x| x.level == level.id.unwrap())
            .collect();
        let mut tags_in_level: HashMap<i32, BackendTag> = HashMap::with_capacity(tag_list.len());
        for tag in &tag_list {
            tags_in_level.insert(
                tag.id.unwrap(),
                BackendTag {
                    values: tag_list
                        .clone()
                        .into_iter()
                        .map(|x| (x.id.unwrap(), model::StoreMetric::new_empty()))
                        .collect(),
                },
            );
        }
        levels.insert(level.id.unwrap(), tags_in_level);
    }
    return Ok((levels, peers_with_tags));
}

fn _parse_output_metrics(
    local_metrics: &HashMap<String, HashMap<String, model::StoreMetric>>,
    peers_with_tags: HashMap<String, HashMap<i32, i32>>,
    levels: &mut HashMap<i32, HashMap<i32, BackendTag>>,
) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    for (key, metric) in local_metrics {
        let dst = key;
        for (src, vals) in metric {
            let src_level = peers_with_tags[src].clone();
            let dst_level = peers_with_tags[dst].clone();
            for (level, tag) in src_level {
                if let Some(dst_tag) = dst_level.get(&level) {
                    levels.entry(level).and_modify(|x| {
                        x.entry(tag).and_modify(|y| {
                            y.values.entry(*dst_tag).and_modify(|z| {
                                z.ts = ts.clone();
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
                            });
                        });
                    });
                }
            }
        }
    }
}

pub async fn render_results(
    metrics: Arc<RwLock<HashMap<String, HashMap<String, model::StoreMetric>>>>,
    mut db: Box<dyn model::db::Db>,
    reconcile_time: u16,
    output_metrics: Arc<RwLock<HashMap<i32, HashMap<i32, BackendTag>>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let sleep_duration = Duration::from_secs(reconcile_time.into());
    loop {
        async_time::sleep(sleep_duration).await;
        log::debug!("Updating output metrics");
        let peers = db.get_peers().await?;
        let tags = db.get_tags().await?;
        let tag_levels = db.get_tag_levels().await?;
        let peer_tags = db.get_peer_tags().await?;
        let (mut levels, peers_with_tags) =
            _create_result_hashmap(peers, tags, tag_levels, peer_tags).await?;

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
        async fn metrics(
            &self,
            _request: tonic::Request<tonic::Streaming<model::aimsir::MetricMessage>>,
        ) -> std::result::Result<tonic::Response<model::aimsir::MetricResponse>, tonic::Status>
        {
            Ok(tonic::Response::new(model::aimsir::MetricResponse {
                ok: true,
            }))
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
        metrics
            .write()
            .await
            .insert(0, HashMap::from([(1, backendtag)]));
        let db = Arc::new(RwLock::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        ));
        let backend = BackendState {
            metrics: metrics.clone(),
            db,
            grpc_server: Arc::new(RwLock::new("http://127.0.0.1:10000".into())),
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
        metrics
            .write()
            .await
            .insert(0, HashMap::from([(1, backendtag)]));
        let db = Arc::new(RwLock::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        ));
        let backend = BackendState {
            metrics: metrics.clone(),
            db,
            grpc_server: Arc::new(RwLock::new("http://127.0.0.1:10000".into())),
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
        metrics
            .write()
            .await
            .insert(0, HashMap::from([(1, backendtag)]));
        let db = Arc::new(RwLock::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        ));
        let backend = BackendState {
            metrics: metrics.clone(),
            db,
            grpc_server: Arc::new(RwLock::new("http://127.0.0.1:10000".into())),
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
    async fn test_get_levels() {
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
        metrics
            .write()
            .await
            .insert(0, HashMap::from([(1, backendtag)]));
        let db = Arc::new(RwLock::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        ));
        let backend = BackendState {
            metrics: metrics.clone(),
            db,
            grpc_server: Arc::new(RwLock::new("http://127.0.0.1:10000".into())),
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
            values: HashMap::from([(2, storemetric)]),
        };
        let metrics = Arc::new(RwLock::new(HashMap::new()));
        metrics
            .write()
            .await
            .insert(0, HashMap::from([(1, backendtag)]));
        let db = Arc::new(RwLock::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        ));
        let backend = BackendState {
            metrics: metrics.clone(),
            db: db.clone(),
            grpc_server: Arc::new(RwLock::new("http://127.0.0.1:10000".into())),
        };
        let app = Router::new()
            .route("/peers", get(peers))
            .route("/peers", post(add_peer))
            .route("/peers/:peer", delete(del_peer))
            .route("/tags", get(tags))
            .route("/tags", post(add_tag))
            .route("/tags/:tag", delete(del_tag))
            .route("/levels", get(tag_levels))
            .route("/levels", post(add_tag_level))
            .route("/levels/:level", delete(del_tag_level))
            .route("/peertags", get(peer_tags))
            .route("/peertags", post(add_peer_tag))
            .route("/peertags/:peer/:tag", delete(del_peer_tag))
            .with_state(backend);
        // Creating new peer
        let new_peer = model::Peer {
            peer_id: "0".into(),
            name: "name".into(),
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

        // Creating new level
        let mut new_level = model::TagLevel {
            id: Some(1),
            parent: None,
            name: "Top level".into(),
        };
        let request = Request::builder()
            .method("POST")
            .header("Content-Type", "application/json")
            .uri("/levels")
            .body(Body::from(json!(new_level).to_string()))
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let request = Request::builder()
            .method("GET")
            .uri("/levels")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        new_level.id = Some(0);
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!(vec![new_level]));

        // Creating new tag
        let mut new_tag = model::Tag {
            id: Some(1),
            level: 0,
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

        // delete level
        let request = Request::builder()
            .method("DELETE")
            .uri("/levels/0")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let request = Request::builder()
            .method("GET")
            .uri("/levels")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.expect("ERR");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!([]));
    }

    #[tokio::test]
    async fn test_create_result_hashmap() {
        let tag_levels: Vec<model::TagLevel> = vec![
            model::TagLevel {
                id: Some(0),
                parent: None,
                name: "root".into(),
            },
            model::TagLevel {
                id: Some(1),
                parent: Some(1),
                name: "child".into(),
            },
        ];
        let tags: Vec<model::Tag> = vec![
            model::Tag {
                id: Some(0),
                level: 0,
                name: "root_tag1".into(),
            },
            model::Tag {
                id: Some(1),
                level: 0,
                name: "root_tag2".into(),
            },
            model::Tag {
                id: Some(2),
                level: 1,
                name: "child_tag1".into(),
            },
            model::Tag {
                id: Some(3),
                level: 1,
                name: "child_tag2".into(),
            },
        ];
        let peers: Vec<model::Peer> = vec![
            model::Peer {
                peer_id: "0".into(),
                name: "peer0".into(),
            },
            model::Peer {
                peer_id: "1".into(),
                name: "peer1".into(),
            },
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
                tag_id: 1,
            },
            model::PeerTag {
                peer_id: "1".into(),
                tag_id: 3,
            },
        ];
        let (levels, peers_with_tags) = _create_result_hashmap(peers, tags, tag_levels, peer_tags)
            .await
            .expect("ERR");
        let mut result_levels: HashMap<i32, HashMap<i32, BackendTag>> = HashMap::new();
        let mut level_map: HashMap<i32, BackendTag> = HashMap::new();
        level_map.insert(
            0,
            BackendTag {
                values: HashMap::from([
                    (0, model::StoreMetric::new_empty()),
                    (1, model::StoreMetric::new_empty()),
                ]),
            },
        );
        level_map.insert(
            1,
            BackendTag {
                values: HashMap::from([
                    (1, model::StoreMetric::new_empty()),
                    (0, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(0, level_map);
        let mut level_map: HashMap<i32, BackendTag> = HashMap::new();
        level_map.insert(
            2,
            BackendTag {
                values: HashMap::from([
                    (2, model::StoreMetric::new_empty()),
                    (3, model::StoreMetric::new_empty()),
                ]),
            },
        );
        level_map.insert(
            3,
            BackendTag {
                values: HashMap::from([
                    (3, model::StoreMetric::new_empty()),
                    (2, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(1, level_map);
        assert_eq!(result_levels, levels);
        let mut result_peer_with_tags: HashMap<String, HashMap<i32, i32>> = HashMap::new();
        let peer_hashmap: HashMap<i32, i32> = HashMap::from([(0, 0), (1, 2)]);
        result_peer_with_tags.insert("0".into(), peer_hashmap);
        let peer_hashmap: HashMap<i32, i32> = HashMap::from([(0, 1), (1, 3)]);
        result_peer_with_tags.insert("1".into(), peer_hashmap);
        assert_eq!(result_peer_with_tags, peers_with_tags);
    }
    #[tokio::test]
    async fn test_parse_output_metrics() {
        let mut result_levels: HashMap<i32, HashMap<i32, BackendTag>> = HashMap::new();
        let mut level_map: HashMap<i32, BackendTag> = HashMap::new();
        level_map.insert(
            0,
            BackendTag {
                values: HashMap::from([
                    (0, model::StoreMetric::new_empty()),
                    (1, model::StoreMetric::new_empty()),
                ]),
            },
        );
        level_map.insert(
            1,
            BackendTag {
                values: HashMap::from([
                    (1, model::StoreMetric::new_empty()),
                    (0, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(0, level_map);
        let mut level_map: HashMap<i32, BackendTag> = HashMap::new();
        level_map.insert(
            2,
            BackendTag {
                values: HashMap::from([
                    (2, model::StoreMetric::new_empty()),
                    (3, model::StoreMetric::new_empty()),
                ]),
            },
        );
        level_map.insert(
            3,
            BackendTag {
                values: HashMap::from([
                    (3, model::StoreMetric::new_empty()),
                    (2, model::StoreMetric::new_empty()),
                ]),
            },
        );
        result_levels.insert(1, level_map);
        let mut result_peer_with_tags: HashMap<String, HashMap<i32, i32>> = HashMap::new();
        let peer_hashmap: HashMap<i32, i32> = HashMap::from([(0, 0), (1, 2)]);
        result_peer_with_tags.insert("0".into(), peer_hashmap);
        let peer_hashmap: HashMap<i32, i32> = HashMap::from([(0, 1), (1, 3)]);
        result_peer_with_tags.insert("1".into(), peer_hashmap);
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
                y.ts = 1;
                y.jitter_min = 1.0;
                y.jitter_max = 1.0;
                y.jitter_stddev = 1.0;
            });
        });
        local_metrics.entry("1".into()).and_modify(|x| {
            x.entry("0".into()).and_modify(|y| {
                y.ts = 1;
                y.jitter_min = 1.0;
                y.jitter_max = 2.0;
                y.jitter_stddev = 2.0;
            });
        });
        _parse_output_metrics(&local_metrics, result_peer_with_tags, &mut levels);
        result_levels.entry(1).and_modify(|x| {
            x.entry(3).and_modify(|y| {
                y.values.entry(2).and_modify(|z| {
                    z.ts = levels
                        .get(&1)
                        .unwrap()
                        .get(&3)
                        .unwrap()
                        .values
                        .get(&2)
                        .unwrap()
                        .ts;
                    z.jitter_stddev = 1.0;
                    z.jitter_max = 1.0;
                    z.jitter_min = 1.0;
                });
            });
        });
        result_levels.entry(1).and_modify(|x| {
            x.entry(2).and_modify(|y| {
                y.values.entry(3).and_modify(|z| {
                    z.ts = levels
                        .get(&1)
                        .unwrap()
                        .get(&2)
                        .unwrap()
                        .values
                        .get(&3)
                        .unwrap()
                        .ts;
                    z.jitter_stddev = 2.0;
                    z.jitter_max = 2.0;
                    z.jitter_min = 1.0;
                });
            });
        });
        result_levels.entry(0).and_modify(|x| {
            x.entry(0).and_modify(|y| {
                y.values.entry(1).and_modify(|z| {
                    z.ts = levels
                        .get(&0)
                        .unwrap()
                        .get(&0)
                        .unwrap()
                        .values
                        .get(&1)
                        .unwrap()
                        .ts;
                    z.jitter_stddev = 2.0;
                    z.jitter_max = 2.0;
                    z.jitter_min = 1.0;
                });
            });
        });
        result_levels.entry(0).and_modify(|x| {
            x.entry(1).and_modify(|y| {
                y.values.entry(0).and_modify(|z| {
                    z.ts = levels
                        .get(&0)
                        .unwrap()
                        .get(&1)
                        .unwrap()
                        .values
                        .get(&0)
                        .unwrap()
                        .ts;
                    z.jitter_stddev = 1.0;
                    z.jitter_max = 1.0;
                    z.jitter_min = 1.0;
                });
            });
        });
        assert_eq!(levels, result_levels);
    }
}
