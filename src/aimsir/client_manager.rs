use crate::model;
use crate::model::aimsir;
use crate::model::aimsir::{
    aimsir_service_client::AimsirServiceClient, Metric, MetricMessage, MetricType,
};
use crate::peers_controller;
// use async_stream::stream;
use log;
use core::panic;
use std::sync::Arc;
use tokio::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
    task::JoinSet,
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{transport, Request, Streaming};

pub struct ManagerController {
    handles: JoinSet<()>,
}

pub struct GRPCClient {
    uri: String,
    client: Arc<Mutex<Option<AimsirServiceClient<transport::Channel>>>>,
}

impl GRPCClient {
    pub async fn new(uri: String) -> Self {
        let client = Arc::new(Mutex::new(None));
        Self { uri, client }
    }

    // Method to get the client when it's ready
    pub async fn get_client(
        &self,
    ) -> AimsirServiceClient<transport::Channel> {
        // Wait until the client is available
        let mut client_lock = self.client.lock().await;
        loop {
            if let Some(ref orig_client) = *client_lock {
                let mut client = orig_client.clone();
                if let Ok(_) = client.ping(Request::new(())).await {
                    return client;
                }
            }
            log::debug!("Reconnecting to server");
            match transport::Endpoint::from_shared(self.uri.clone())
                .unwrap()
                .connect()
                .await
            {
                Ok(channel) => {
                    let aimsir_client = AimsirServiceClient::new(channel);
                    *client_lock = Some(aimsir_client);
                }
                Err(e) => {
                    log::error!("Failed to create endpoint: {}", e);
                }
            }
        }
    }
}

impl ManagerController {
    pub async fn new(
        uri: Box<str>,
        id: Box<str>,
        ipaddress: Box<str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        log::info!("Starting manager");
        let client_handler = Arc::new(Mutex::new(GRPCClient::new(uri.to_string()).await));
        log::debug!("Connected to server");
        let (meas_tx, meas_rx) = channel(10);
        let (peer_tx, peer_rx) = channel(10);
        let mut handles = JoinSet::new();
        let mut peer_ctrl =
            peers_controller::PeerController::new(id.clone(), 1, 60, peer_rx, meas_tx, false).await;
        log::debug!("Created peer controller");
        handles.spawn(async move {
            log::debug!("Spawning peer controller worker");
            peer_ctrl.work().await;
        });
        let send_client = client_handler.clone();
        let subscribe_client = client_handler.clone();
        let local_id = String::from(id.clone());
        handles.spawn(async move {
            log::info!("Spawning metric composer");
            metric_composer(local_id, meas_rx, send_client).await
        });
        handles.spawn(async move {
            loop {
                log::info!("Registering on the server");
                let mut register_client = subscribe_client.lock().await.get_client().await;
                match register_client.register(Request::new(aimsir::Peer { id: id.to_string(), ipaddress: ipaddress.to_string()})).await {
                    Ok(clients) => {
                        log::info!("Spawning update processor");
                        update_processor(peer_tx.clone(), clients.into_inner()).await;
                    },
                    Err(_) => {
                        log::error!("Failed to register");
                    }
                }
            }
        });
        Ok(Self {
            handles,
        })
    }

    pub async fn worker(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(res) = self.handles.join_next().await {
            let out = res?;
            log::error!("One of worker has exited: {:?}", out);
        }
        Ok(())
    }
}

async fn metric_composer(
    local_id: String,
    mut meas_rx: Receiver<Vec<model::Measurement>>,
    client: Arc<Mutex<GRPCClient>>,
) {
    log::debug!("Start sending metrics");
    let (tx, mut rx) = channel::<Sender<MetricMessage>>(1);

    tokio::spawn(async move {
        while let Some(sender) = rx.recv().await {
            while let Some(res_vec) = meas_rx.recv().await {
                let mut metric_vec: Vec<Metric> = Vec::new();
                for single_res in res_vec {
                    log::trace!("New metric for {}", single_res.id.to_string());
                    metric_vec.push(Metric {
                        metric_type: MetricType::from_str_name("PL").unwrap() as i32,
                        value: single_res.pl as f32,
                        local_id: local_id.clone(),
                        peer_id: single_res.id.to_string(),
                    });
                    metric_vec.push(Metric {
                        metric_type: MetricType::from_str_name("JitterStdDev").unwrap() as i32,
                        value: single_res.jitter_stddev as f32,
                        local_id: local_id.clone(),
                        peer_id: single_res.id.to_string(),
                    });
                    metric_vec.push(Metric {
                        metric_type: MetricType::from_str_name("JitterMin").unwrap() as i32,
                        value: single_res.jitter_min as f32,
                        local_id: local_id.clone(),
                        peer_id: single_res.id.to_string(),
                    });
                    metric_vec.push(Metric {
                        metric_type: MetricType::from_str_name("JitterMax").unwrap() as i32,
                        value: single_res.jitter_max as f32,
                        local_id: local_id.clone(),
                        peer_id: single_res.id.to_string(),
                    });
                }
                _ = sender.send(MetricMessage { metric: metric_vec }).await;
            }
        }
    });
    loop {
        log::debug!("Waiting for server connection");
        let mut metric_client = client.lock().await.get_client().await;
        let (metric_tx, metric_rx) = channel(1);
        log::debug!("Sending handler to metric generator");
        if let Err(e) = tx.send(metric_tx).await {
            panic!("Metric sender thread paniced: {}", e);
        }
        log::debug!("Calling server");
        match metric_client
            .metrics(Request::new(ReceiverStream::new(metric_rx)))
            .await
        {
            Ok(response) => {
                let mut response_stream = response.into_inner();
                while let Some(received) = response_stream.next().await {
                    match received {
                        Ok(result) => {
                            log::debug!("Metric sent: {}", result.ok);
                        }
                        Err(err) => {
                            log::warn!("Failed to send metric: {}", err);
                        }
                    }
                }
                log::debug!("Log sender exited");
            }
            Err(e) => {
                log::error!("Failed to connect to metric: {}", e.to_string());
            }
        }
    }
}

async fn update_processor(
    peer_tx: Sender<model::NeighbourUpdate>,
    mut client: Streaming<model::aimsir::PeerUpdate>,
) {
    while let Ok(peer_update) = client.message().await {
        if let Some(update) = peer_update {
            log::trace!("Neighgor update");
            let new_update = model::NeighbourUpdate {
                aggregate_timer: update.aggregate_interval as u64,
                probe_timer: update.probe_interval as u64,
                update_type: model::UpdateType::from_proto(update.update_type).unwrap(),
                update: update.update,
            };
            let _ = peer_tx.send(new_update).await;
        }
    }
    log::debug!("Update processor exited");
}
#[cfg(test)]
mod tests {
    use super::*;
    use model::aimsir::aimsir_service_server::AimsirService;
    use std::net::SocketAddr;
    use tokio::{self, sync::mpsc};
    use tokio_stream::{wrappers::ReceiverStream, StreamExt};
    use tonic;
    struct TestServer {
        tx: mpsc::Sender<model::aimsir::Peer>,
        metric_tx: mpsc::Sender<model::aimsir::MetricMessage>,
    }

    #[tonic::async_trait]
    impl AimsirService for TestServer {
        type RegisterStream = ReceiverStream<Result<model::aimsir::PeerUpdate, tonic::Status>>;
        type MetricsStream = ReceiverStream<Result<aimsir::MetricResponse, tonic::Status>>;
        async fn ping(
            &self,
            _request: tonic::Request<()>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            Ok(tonic::Response::new(()))
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
                        let _ = metric_tx.send(metric).await;
                    };
                    _ = tx.send(Ok(aimsir::MetricResponse { ok: true })).await;
                }
            });
            Ok(tonic::Response::new(ReceiverStream::new(rx)))
        }
        async fn register(
            &self,
            request: tonic::Request<model::aimsir::Peer>,
        ) -> std::result::Result<tonic::Response<Self::RegisterStream>, tonic::Status> {
            let (tx, rx) = mpsc::channel(1);
            let _ = self.tx.send(request.get_ref().clone()).await;
            tokio::spawn(async move {
                let peer = model::aimsir::PeerUpdate {
                    update_type: model::aimsir::PeerUpdateType::Add.into(),
                    probe_interval: 1,
                    aggregate_interval: 60,
                    update: vec![model::aimsir::Peer {
                        id: String::from("01"),
                        ipaddress: String::from("1.1.1.1"),
                    }],
                };
                let _ = tx.send(Ok(peer)).await;
            });
            Ok(tonic::Response::new(ReceiverStream::new(rx)))
        }
        async fn add_peer(
            &self,
            _request: tonic::Request<model::aimsir::Peer>,
        ) -> std::result::Result<tonic::Response<model::aimsir::PeerResponse>, tonic::Status>
        {
            Ok(tonic::Response::new(model::aimsir::PeerResponse {
                ok: true,
            }))
        }
        async fn remove_peer(
            &self,
            _request: tonic::Request<model::aimsir::Peer>,
        ) -> std::result::Result<tonic::Response<model::aimsir::PeerResponse>, tonic::Status>
        {
            Ok(tonic::Response::new(model::aimsir::PeerResponse {
                ok: true,
            }))
        }
    }
    #[tokio::test]
    async fn test_update_processor() {
        let _ = env_logger::try_init();
        let addr: SocketAddr = "127.0.0.1:10000".parse().unwrap();
        let (tx, mut rx) = mpsc::channel::<model::aimsir::Peer>(1);
        let (metric_tx, _metric_rx) = mpsc::channel::<model::aimsir::MetricMessage>(1);
        let svc = TestServer { tx, metric_tx };
        let server = model::aimsir::aimsir_service_server::AimsirServiceServer::new(svc);
        tokio::spawn(async move {
            let _ = tonic::transport::Server::builder()
                .add_service(server)
                .serve(addr)
                .await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let mut ctrl = super::ManagerController::new(
            "http://127.0.0.1:10000".into(),
            "01".into(),
            "127.0.0.1".into(),
        )
        .await
        .unwrap();
        tokio::spawn(async move {
            let _ = ctrl.worker().await;
        });
        let received = rx.recv().await;
        if let Some(peer) = received {
            assert_eq!(peer.id, String::from("01"));
            assert_eq!(peer.ipaddress, String::from("127.0.0.1"));
        } else {
            assert!(false);
        }
    }
    #[tokio::test]
    async fn test_metrics_processor() {
        let _ = env_logger::try_init();
        let addr: SocketAddr = "127.0.0.1:10000".parse().unwrap();
        let (metric_tx, mut metric_rx) = mpsc::channel::<model::aimsir::MetricMessage>(1);
        let (tx, _rx) = mpsc::channel::<model::aimsir::Peer>(1);
        let (probe_tx, probe_rx) = mpsc::channel::<Vec<model::Measurement>>(1);
        let svc = TestServer { tx, metric_tx };
        let server = model::aimsir::aimsir_service_server::AimsirServiceServer::new(svc);
        tokio::spawn(async move {
            let _ = tonic::transport::Server::builder()
                .add_service(server)
                .serve(addr)
                .await;
        });
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let client = Arc::new(Mutex::new(GRPCClient::new(String::from("http://127.0.0.1:10000")).await));
        // let client = AimsirServiceClient::connect(String::from("http://127.0.0.1:10000"))
            // .await
            // .unwrap();
        tokio::spawn(async move {
            super::metric_composer("0".into(), probe_rx, client).await;
        });
        let measurement = vec![model::Measurement {
            id: "1".into(),
            count: 10,
            pl: 1,
            jitter_min: 1.0,
            jitter_max: 2.0,
            jitter_stddev: 1.5,
        }];
        let send_result = probe_tx.send(measurement).await;
        assert!(send_result.is_ok());
        let result = metric_rx.recv().await;
        if let Some(metrics) = result {
            for metric in metrics.metric {
                match metric.metric_type() {
                    aimsir::MetricType::Pl => {
                        assert_eq!(metric.value, 1.0);
                    }
                    aimsir::MetricType::JitterMin => {
                        assert_eq!(metric.value, 1.0);
                    }
                    aimsir::MetricType::JitterMax => {
                        assert_eq!(metric.value, 2.0);
                    }
                    aimsir::MetricType::JitterStdDev => {
                        assert_eq!(metric.value, 1.5);
                    }
                }
            }
        } else {
            assert!(false);
        }
    }
}
