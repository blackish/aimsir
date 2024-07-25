use log;
use crate::model::aimsir::{self, aimsir_service_server};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, broadcast};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

pub struct ServerController {
    clients: Arc<RwLock<Vec<aimsir::Peer>>>,
    update_tx: broadcast::Sender<aimsir::PeerUpdate>,
    probe_interval: u32,
    aggregate_interval: u32,
}

impl ServerController {
    pub async fn new(probe_interval: u32, aggregate_interval: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let (update_tx, _update_rx) = broadcast::channel::<aimsir::PeerUpdate>(128);
        let clients = Arc::new(
            RwLock::new(
                Vec::new()
            )
        );
        let new_server = ServerController{
            clients,
            update_tx,
            probe_interval,
            aggregate_interval,
        };
        Ok(new_server)
    }

    async fn add_peer(
        &self,
        request: tonic::Request<super::Peer>,
    ) -> std::result::Result<tonic::Response<super::PeerResponse>, tonic::Status>;
    async fn remove_peer(
        &self,
        request: tonic::Request<super::Peer>,
    ) -> std::result::Result<tonic::Response<super::PeerResponse>, tonic::Status>;
    pub async fn add_peer(&self, id: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut local_clients = self.clients.write().await;
        if let Some(_index) = local_clients.iter().position(|x| x.id == id) {
            // peer already exists in DB
            return Ok(())
        };
        let new_peer = aimsir::Peer {
            id,
            ipaddress: String::from(""),
        };
        local_clients.push(new_peer.clone());
        let sender = self.update_tx.clone();
        if let Ok(_) = sender.send(
            aimsir::PeerUpdate {
                update_type: aimsir::PeerUpdateType::Add.into(),
                probe_interval: self.probe_interval.clone(),
                aggregate_interval: self.aggregate_interval.clone(),
                update: vec! [new_peer],
            }
        ) {
            return Ok(())
        }
        return Err(Box::from("Failed to create peer"))
    }

    pub async fn remove_peer(&self, id: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut local_clients = self.clients.write().await;
        let position = local_clients.iter().position(|x| x.id == id);
        if position.is_none() {
            // peer does not exist in DB
            return Ok(())
        };
        let new_peer = local_clients.remove(position.unwrap());
        let sender = self.update_tx.clone();
        if let Ok(_) = sender.send(
            aimsir::PeerUpdate {
                update_type: aimsir::PeerUpdateType::Remove.into(),
                probe_interval: self.probe_interval.clone(),
                aggregate_interval: self.aggregate_interval.clone(),
                update: vec! [new_peer],
            }
        ) {
            return Ok(())
        }
        return Err(Box::from("Failed to remove peer"))
    }
}

#[tonic::async_trait]
impl aimsir_service_server::AimsirService for ServerController {
    type RegisterStream = ReceiverStream<Result<aimsir::PeerUpdate, tonic::Status>>;
    // Got new peer
    async fn register(
        &self,
        request: tonic::Request<aimsir::Peer>,
    ) -> std::result::Result<tonic::Response<Self::RegisterStream>, tonic::Status>{
        let (tx, rx) = mpsc::channel(2);
        let mut receiver: broadcast::Receiver<aimsir::PeerUpdate>;
        let sender = self.update_tx.clone();
        let local_peer = request.into_inner();
        let probe_interval = self.probe_interval.clone();
        let aggregate_interval = self.aggregate_interval.clone();
        log::debug!("Got connection from {}, id: {}", local_peer.ipaddress, local_peer.id);
        {
            let mut local_clients = self.clients.write().await;
            // if peer id exists in DB, proceed
            if let Some(my_index) = local_clients.iter().position(|x| x.id == local_peer.id) {
                // send a list of known peers to him
                let peer_update = aimsir::PeerUpdate {
                    update_type: aimsir::PeerUpdateType::Add.into(),
                    probe_interval: probe_interval.clone(),
                    aggregate_interval: aggregate_interval.clone(),
                    update: local_clients.clone().iter().filter(|x| x.ipaddress != "").cloned().collect(),
                };
                let _ = tx.send(Ok(peer_update));
                // if reported ip address of this peer is differ from what we know, update DB
                if local_clients[my_index].ipaddress != local_peer.ipaddress {
                    local_clients[my_index].ipaddress = local_peer.ipaddress.clone();
                    let peer_update = aimsir::PeerUpdate {
                        update_type: aimsir::PeerUpdateType::Add.into(),
                        probe_interval: probe_interval.clone(),
                        aggregate_interval: aggregate_interval.clone(),
                        update: vec![
                            aimsir::Peer {
                                id: local_peer.id,
                                ipaddress: local_peer.ipaddress.clone(),
                            }
                        ],
                    };
                    let _ = sender.send(peer_update);
                }
                receiver = self.update_tx.subscribe();
            } else {
                // If peer is not known to server, return error
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

    async fn metrics(
        &self,
        request: tonic::Request<tonic::Streaming<aimsir::MetricMessage>>,
    ) -> std::result::Result<tonic::Response<aimsir::MetricResponse>, tonic::Status>{
        let mut metric_stream = request.into_inner();
        while let Some(metric) = metric_stream.next().await {
            if let Ok(metric) = metric {
                for single_metric in metric.metric {
                    println!("Got metric: id: {}, value: {}", single_metric.peer_id, single_metric.value);
                }
            }
        }
        Ok(tonic::Response::new(aimsir::MetricResponse{ok: true}))
    }
}
