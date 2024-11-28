use crate::model::{
    self,
    aimsir::{self, aimsir_service_server},
};
use log;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

pub struct ServerController {
    clients: Arc<RwLock<Vec<aimsir::Peer>>>,
    update_tx: broadcast::Sender<aimsir::PeerUpdate>,
    probe_interval: u32,
    aggregate_interval: u32,
    metric_tx: mpsc::Sender<aimsir::Metric>,
    metrics: Arc<RwLock<HashMap<String, HashMap<String, model::StoreMetric>>>>,
}

impl ServerController {
    pub async fn new(
        probe_interval: u32,
        aggregate_interval: u32,
        mut db: Box<dyn model::db::Db>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (update_tx, _update_rx) = broadcast::channel::<aimsir::PeerUpdate>(128);
        let (metric_tx, metric_rx) = mpsc::channel::<aimsir::Metric>(128);
        log::info!("Starting server controller");
        let clients = Arc::new(RwLock::new(
            // clients
            db.get_peers()
                .await?
                .into_iter()
                .map(|x| aimsir::Peer {
                    id: x.peer_id,
                    ipaddress: "".into(),
                })
                .collect(),
        ));
        let new_server = ServerController {
            clients,
            update_tx,
            probe_interval,
            aggregate_interval,
            metric_tx,
            metrics: Arc::new(HashMap::new().into()),
        };
        let receiver_metrics = new_server.metrics.clone();
        tokio::spawn(async move {
            log::info!("Spawning metric processor");
            let _ = metric_processor(receiver_metrics, metric_rx).await;
        });
        log::info!("Server started");
        Ok(new_server)
    }

    pub fn get_metrics(&self) -> Arc<RwLock<HashMap<String, HashMap<String, model::StoreMetric>>>> {
        self.metrics.clone()
    }
}

async fn metric_processor(
    metrics: Arc<RwLock<HashMap<String, HashMap<String, model::StoreMetric>>>>,
    mut metric_rx: mpsc::Receiver<aimsir::Metric>,
) -> Result<(), Box<dyn std::error::Error>> {
    while let Some(metric) = metric_rx.recv().await {
        let mut local_metrics = metrics.write().await;
        local_metrics
            .entry(metric.local_id.clone())
            .and_modify(|x| {
                x.entry(metric.peer_id.clone())
                    .and_modify(|y| {
                        y.update(metric.clone());
                    })
                    .or_insert(model::StoreMetric::new(metric.clone()));
            })
            .or_insert_with(|| {
                let mut new_map = HashMap::new();
                new_map.insert(metric.peer_id.clone(), model::StoreMetric::new(metric));
                new_map
            });
    }
    Ok(())
}

#[tonic::async_trait]
impl aimsir_service_server::AimsirService for ServerController {
    type RegisterStream = ReceiverStream<Result<aimsir::PeerUpdate, tonic::Status>>;
    type MetricsStream = ReceiverStream<Result<aimsir::MetricResponse, tonic::Status>>;
    // Got new peer
    async fn register(
        &self,
        request: tonic::Request<aimsir::Peer>,
    ) -> std::result::Result<tonic::Response<Self::RegisterStream>, tonic::Status> {
        let (tx, rx) = mpsc::channel(10);
        let mut receiver: broadcast::Receiver<aimsir::PeerUpdate>;
        let sender = self.update_tx.clone();
        let local_peer = request.into_inner();
        let probe_interval = self.probe_interval.clone();
        let aggregate_interval = self.aggregate_interval.clone();
        log::debug!(
            "Got connection from {}, id: {}",
            local_peer.ipaddress,
            local_peer.id
        );
        {
            let mut local_clients = self.clients.write().await;
            // if peer id exists in DB, proceed
            if let Some(my_index) = local_clients.iter().position(|x| x.id == local_peer.id) {
                // send a list of known peers to him
                let peer_update = aimsir::PeerUpdate {
                    update_type: aimsir::PeerUpdateType::Full.into(),
                    probe_interval: probe_interval.clone(),
                    aggregate_interval: aggregate_interval.clone(),
                    update: local_clients
                        .clone()
                        .iter()
                        .filter(|x| x.ipaddress != "")
                        .cloned()
                        .collect(),
                };
                let _ = tx.send(Ok(peer_update)).await;
                // if reported ip address of this peer is differ from what we know, update DB
                if local_clients[my_index].ipaddress != local_peer.ipaddress {
                    local_clients[my_index].ipaddress = local_peer.ipaddress.clone();
                    let peer_update = aimsir::PeerUpdate {
                        update_type: aimsir::PeerUpdateType::Add.into(),
                        probe_interval: probe_interval.clone(),
                        aggregate_interval: aggregate_interval.clone(),
                        update: vec![aimsir::Peer {
                            id: local_peer.id,
                            ipaddress: local_peer.ipaddress.clone(),
                        }],
                    };
                    let _ = sender.send(peer_update);
                }
                receiver = self.update_tx.subscribe();
            } else {
                // If peer is not known to server, return error
                log::debug!("Peer not found");
                return Err(tonic::Status::not_found("Peer not found"));
            }
        };
        tokio::spawn(async move {
            // spawn update process for this peer
            while !tx.is_closed() {
                if let Ok(update) = receiver.recv().await {
                    let _ = tx.send(Ok(update)).await;
                }
            }
            log::debug!("Worker {} disconnected", local_peer.ipaddress);
        });
        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }

    async fn add_peer(
        &self,
        request: tonic::Request<aimsir::Peer>,
    ) -> std::result::Result<tonic::Response<aimsir::PeerResponse>, tonic::Status> {
        let mut local_clients = self.clients.write().await;
        let new_peer = request.into_inner();
        if let Some(_index) = local_clients.iter().position(|x| x.id == new_peer.id) {
            // peer already exists in DB
            return Err(tonic::Status::already_exists("Peer already exists"));
        };
        let new_peer = aimsir::Peer {
            id: new_peer.id,
            ipaddress: String::from(""),
        };
        let sender = self.update_tx.clone();
        if sender.receiver_count() == 0 {
            local_clients.push(new_peer.clone());
            return Ok(tonic::Response::new(aimsir::PeerResponse { ok: true }));
        }
        if let Ok(_) = sender.send(aimsir::PeerUpdate {
            update_type: aimsir::PeerUpdateType::Add.into(),
            probe_interval: self.probe_interval.clone(),
            aggregate_interval: self.aggregate_interval.clone(),
            update: vec![new_peer.clone()],
        }) {
            local_clients.push(new_peer);
            return Ok(tonic::Response::new(aimsir::PeerResponse { ok: true }));
        }
        return Err(tonic::Status::internal("Failed to send peers"));
    }

    async fn remove_peer(
        &self,
        request: tonic::Request<aimsir::Peer>,
    ) -> std::result::Result<tonic::Response<aimsir::PeerResponse>, tonic::Status> {
        let mut local_clients = self.clients.write().await;
        let new_peer = request.into_inner();
        let position = local_clients.iter().position(|x| x.id == new_peer.id);
        if position.is_none() {
            // peer does not exist in DB
            return Err(tonic::Status::not_found("Peer not found"));
        };
        let new_peer = local_clients.get(position.unwrap()).unwrap();
        let sender = self.update_tx.clone();
        if sender.receiver_count() == 0 {
            _ = local_clients.remove(position.unwrap());
            return Ok(tonic::Response::new(aimsir::PeerResponse { ok: true }));
        }
        if let Ok(_) = sender.send(aimsir::PeerUpdate {
            update_type: aimsir::PeerUpdateType::Remove.into(),
            probe_interval: self.probe_interval.clone(),
            aggregate_interval: self.aggregate_interval.clone(),
            update: vec![new_peer.clone()],
        }) {
            _ = local_clients.remove(position.unwrap());
            return Ok(tonic::Response::new(aimsir::PeerResponse { ok: true }));
        }
        return Err(tonic::Status::internal("Failed to send peers"));
    }

    async fn metrics(
        &self,
        request: tonic::Request<tonic::Streaming<aimsir::MetricMessage>>,
    ) -> std::result::Result<tonic::Response<Self::MetricsStream>, tonic::Status> {
        let (tx, rx) = mpsc::channel(1);
        let mut metric_stream = request.into_inner();
        let metric_tx = self.metric_tx.clone();
        tokio::spawn(async move {
            while let Some(metric) = metric_stream.next().await {
                if let Ok(metric) = metric {
                    for single_metric in metric.metric {
                        let _ = metric_tx.send(single_metric).await;
                    }
                };
                _ = tx.send(Ok(aimsir::MetricResponse { ok: true })).await;
            }
        });
        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::db::Db;
    use aimsir::{
        self, aimsir_service_client::AimsirServiceClient,
        aimsir_service_server::AimsirServiceServer,
    };
    use std::net::SocketAddr;
    use tokio::{self, sync::mpsc};
    use tokio_stream;
    use tonic;
    use dotenv;
    use std::env;

    #[tokio::test]
    async fn test_connect_nonexisting_peer() {
        let _ = env_logger::try_init();
        dotenv::dotenv().expect("Could not load the .env file!");
        let database_url =
            env::var("DATABASE_URL").expect("The environment variable DATABASE_URL is missing!");
        let db = Box::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        );
        let server = ServerController::new(1, 60, db).await.unwrap();
        let node_ip: SocketAddr = "127.0.0.1:10000".parse().unwrap();
        let server = AimsirServiceServer::new(server);
        tokio::spawn(async move {
            let _ = tonic::transport::Server::builder()
                .add_service(server)
                .serve(node_ip)
                .await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let mut client = AimsirServiceClient::connect("http://127.0.0.1:10000")
            .await
            .unwrap();
        let response = client
            .register(tonic::Request::new(aimsir::Peer {
                id: "0".to_string(),
                ipaddress: "127.0.0.1".to_string(),
            }))
            .await;
        assert!(response.is_err() && response.unwrap_err().message() == "Peer not found");
    }

    #[tokio::test]
    async fn test_connect_existing_peer() {
        let _ = env_logger::try_init();
        dotenv::dotenv().expect("Could not load the .env file!");
        let database_url =
            env::var("DATABASE_URL").expect("The environment variable DATABASE_URL is missing!");
        let mut local_db = model::mysql::MysqlDb::new(database_url.to_string())
            .await
            .unwrap();
        local_db
            .add_peer(model::Peer {
                peer_id: "0".into(),
                name: "noname".into(),
            })
            .await
            .unwrap();
        let db = Box::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        );
        let server = ServerController::new(1, 60, db).await.unwrap();
        let node_ip: SocketAddr = "127.0.0.1:10000".parse().unwrap();
        let server = AimsirServiceServer::new(server);
        tokio::spawn(async move {
            let _ = tonic::transport::Server::builder()
                .add_service(server)
                .serve(node_ip)
                .await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let (add_tx, mut add_rx) = mpsc::channel::<model::aimsir::PeerUpdate>(1);
        let (full_tx, mut full_rx) = mpsc::channel::<model::aimsir::PeerUpdate>(1);
        let mut client = AimsirServiceClient::connect("http://127.0.0.1:10000")
            .await
            .unwrap();
        let mut register_client = client.clone();
        tokio::spawn(async move {
            let mut response = register_client
                .register(tonic::Request::new(aimsir::Peer {
                    id: "0".to_string(),
                    ipaddress: "127.0.0.1".to_string(),
                }))
                .await
                .unwrap()
                .into_inner();
            loop {
                if let Ok(message) = response.message().await {
                    let _ = add_tx.send(message.unwrap()).await;
                } else {
                    break;
                }
            }
        });
        let _ = client
            .add_peer(tonic::Request::new(aimsir::Peer {
                id: "1".into(),
                ipaddress: "".into(),
            }))
            .await;
        let _ = tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        tokio::spawn(async move {
            let response = client
                .register(tonic::Request::new(aimsir::Peer {
                    id: "1".to_string(),
                    ipaddress: "127.0.0.1".to_string(),
                }))
                .await
                .unwrap();
            if let Ok(message) = response.into_inner().message().await {
                let _ = full_tx.send(message.unwrap()).await;
            }
        });
        local_db.del_peer("0".into()).await.unwrap();
        if let Some(message) = full_rx.recv().await {
            assert_eq!(
                message,
                aimsir::PeerUpdate {
                    update_type: 2,
                    probe_interval: 1,
                    aggregate_interval: 60,
                    update: vec![aimsir::Peer {
                        id: "0".into(),
                        ipaddress: "127.0.0.1".into()
                    }]
                },
            );
        } else {
            assert! {false};
        }
        if let Some(update) = add_rx.recv().await {
            assert_eq!(
                update,
                aimsir::PeerUpdate {
                    update_type: 2,
                    update: Vec::new(),
                    probe_interval: 1,
                    aggregate_interval: 60,
                }
            )
        } else {
            assert! {false};
        }
        if let Some(update) = add_rx.recv().await {
            assert_eq!(
                update,
                aimsir::PeerUpdate {
                    update_type: 0,
                    update: vec![aimsir::Peer {
                        id: "1".into(),
                        ipaddress: "127.0.0.1".into()
                    }],
                    probe_interval: 1,
                    aggregate_interval: 60,
                }
            )
        } else {
            assert! {false};
        }
    }

    #[tokio::test]
    async fn test_metrics() {
        let _ = env_logger::try_init();
        dotenv::dotenv().expect("Could not load the .env file!");
        let database_url =
            env::var("DATABASE_URL").expect("The environment variable DATABASE_URL is missing!");
        let mut local_db = model::mysql::MysqlDb::new(database_url.to_string())
            .await
            .unwrap();
        local_db
            .add_peer(model::Peer {
                peer_id: "0".into(),
                name: "noname".into(),
            })
            .await
            .unwrap();
        let db = Box::new(
            model::mysql::MysqlDb::new(database_url.to_string())
                .await
                .unwrap(),
        );
        let server = ServerController::new(1, 60, db).await.unwrap();
        local_db.del_peer("0".into()).await.unwrap();
        let received_metrics = server.get_metrics();
        let node_ip: SocketAddr = "127.0.0.1:10000".parse().unwrap();
        let server = AimsirServiceServer::new(server);
        tokio::spawn(async move {
            let _ = tonic::transport::Server::builder()
                .add_service(server)
                .serve(node_ip)
                .await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let mut client = AimsirServiceClient::connect("http://127.0.0.1:10000")
            .await
            .unwrap();
        let mut register_client = client.clone();
        tokio::spawn(async move {
            let mut response = register_client
                .register(tonic::Request::new(aimsir::Peer {
                    id: "0".to_string(),
                    ipaddress: "127.0.0.1".to_string(),
                }))
                .await
                .unwrap()
                .into_inner();
            loop {
                if let Err(_message) = response.message().await {
                    break;
                }
            }
        });
        let metrics = tokio_stream::iter(vec![aimsir::MetricMessage {
            metric: vec![aimsir::Metric {
                metric_type: 0,
                peer_id: "1".into(),
                local_id: "0".into(),
                value: 0.0,
            }],
        }]);
        client.metrics(metrics).await.unwrap();
        let unwrapped_metrics = received_metrics.read().await;
        if let Some(peer) = unwrapped_metrics.get(&String::from("0")) {
            if let Some(store_metric) = peer.get(&String::from("1")) {
                assert_eq!(store_metric.pl, 0,);
                assert_eq!(store_metric.jitter_stddev, -1.0,);
                assert_eq!(store_metric.jitter_min, -1.0,);
                assert_eq!(store_metric.jitter_max, -1.0,);
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }
}
