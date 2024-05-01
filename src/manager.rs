pub mod aimsir {
    tonic::include_proto!("aimsir");
}
use tonic;
use crate::model;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use aimsir::aimsir_service_client::AimsirServiceClient;
use crate::peers;


struct ManagerController {
    uplink_client: AimsirServiceClient<tonic::transport::Channel>,
    measurement_receiver: Receiver<Vec<model::Measurement>>,
    peer_update_sender: Sender<model::NeighbourUpdate>
}

impl ManagerController {
    pub async fn new(uri: Box<str>, id: Box<str>) -> Result<Self, Box<dyn std::error::Error>> {
        let client = AimsirServiceClient::connect(
            String::from(uri)
        ).await?;
        let (meas_tx, meas_rx) = channel(10);
        let (peer_tx, peer_rx) = channel(10);
        let mut peer_ctrl = peers::PeerController::new(id, peer_rx, meas_tx).await;
        tokio::spawn(async move {
            peer_ctrl.work().await;
        });
        Ok(
            Self{
                uplink_client: client,
                measurement_receiver: meas_rx,
                peer_update_sender: peer_tx
            }
        )
    }
    pub async fn worker(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

