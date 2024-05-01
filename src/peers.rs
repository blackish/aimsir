use log;
use std::{
    time::{SystemTime, UNIX_EPOCH},
    collections::HashMap,
    sync::Arc
};
use tokio::{
    sync::{
        Mutex,
        mpsc::{Receiver, Sender, channel}
    },
    net::UdpSocket,
    time::{self, Duration},
    select
};

use serde::{self, Serialize, Deserialize};
use flexbuffers::{FlexbufferSerializer, Reader};

use crate::model;
use crate::constants;

pub struct PeerController {
    peers: Mutex<HashMap<Box<str>, model::Neighbour>>,
    peer_receiver: Receiver<model::Probe>,
    peer_sender: Sender<Vec<String>>,
    manager_receiver: Receiver<model::NeighbourUpdate>,
    manager_sender: Sender<Vec<model::Measurement>>
}

fn make_peer_list(peers: &HashMap<Box<str>, model::Neighbour>) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for key in peers.values() {
        result.push(String::from(key.peer.ipaddress.clone()));
    }
    return result;
}

impl PeerController {
    pub async fn new(
        local_id: Box<str>,
        mgr_rcv: Receiver<model::NeighbourUpdate>,
        mgr_send: Sender<Vec<model::Measurement>>
    ) -> Self {
        let (tx, mut rx) = channel::<Vec<String>>(2);
        let mut listen_address = String::from("0.0.0.0");
        listen_address.push_str(constants::PORT);
        let udp_socket = UdpSocket::bind(listen_address).await.unwrap();
        let udp_sender = Arc::new(udp_socket);
        let udp_receiver = udp_sender.clone();
        log::info!("Start sending thread");
        tokio::spawn(async move {
            let id = local_id.clone();
            let mut seq: u64 = 0;
            while let Some(peer_list) = rx.recv().await {
                let ts = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
    
                let probe = model::Probe {
                    id: id.clone(),
                    seq: seq.clone(),
                    ts
                };
                let mut serializer = FlexbufferSerializer::new();
                probe.serialize(&mut serializer).unwrap();
                for mut dst in peer_list {
                    dst.push_str(constants::PORT);
                    let _ = udp_sender.send_to(serializer.view(), dst).await;
                };
                seq += 1;
            }
        });
        let (receiver_tx, receiver_rx) = channel::<model::Probe>(1_024);
        log::info!("Start receiving thread");
        tokio::spawn(async move {
            let mut buf = [0; 1024];
            loop {
                let(len, addr) = udp_receiver.recv_from(&mut buf).await.unwrap();
                log::debug!("Got a package from {}", addr);
                if let Ok(reader) = Reader::get_root(&buf[..len]) {
                    if let Ok(probe) = model::Probe::deserialize(reader) {
                        log::debug!("Decoded a package: seq={}, ts={}", probe.seq, probe.ts);
                        let _ = receiver_tx.send(probe).await;
                    }
                }
            }
        });
        Self {
            peers: Mutex::new(HashMap::new()),
            peer_receiver: receiver_rx,
            peer_sender: tx,
            manager_receiver: mgr_rcv,
            manager_sender: mgr_send
        }
    }
    pub async fn work(&mut self) {
        let mut timer = time::interval(Duration::from_secs(1));
        let peer_list = Mutex::new(Vec::<String>::new());
        loop {
            select! {
                _res = async {
                    if let Some(peer_msg) = self.peer_receiver.recv().await {
                        log::debug!("Got probe from: {}", peer_msg.id);
                        let mut peers = self.peers.lock().await;
                        let mut measurements: Vec<model::Measurement> = Vec::new();
                        peers
                            .entry(peer_msg.id.clone())
                            .and_modify(|entry|
                                {
                                    if entry.last_ts > 0 {
                                        measurements.push(
                                            model::Measurement{
                                                id: peer_msg.id.clone(),
                                                measurement_type: model::MeasurementType::PL,
                                                value: (peer_msg.seq - entry.last_seq - 1) as f64
                                            }
                                        );
                                        measurements.push(
                                            model::Measurement{
                                                id: peer_msg.id.clone(),
                                                measurement_type: model::MeasurementType::Delay,
                                                value: (peer_msg.ts - entry.last_ts) as f64
                                            }
                                        );
                                    }
                                    entry.last_ts = peer_msg.ts;
                                    entry.last_seq = peer_msg.seq;
                                }
                            );
                        if measurements.len() > 0 {
                            let _ = self.manager_sender.send(measurements).await;
                            log::debug!("Sent all measurements");
                        };
                    }
                } => {}
                _res = async {
                    if let Some(mgr_msg) = self.manager_receiver.recv().await {
                        log::debug!("Got peer update");
                        let mut peers = self.peers.lock().await;
                        match mgr_msg.update_type {
                            model::UpdateType::Add => {
                                for key in mgr_msg.update {
                                    if !peers.contains_key(&key.id) {
                                        peers.insert(
                                            key.id.clone(),
                                            model::Neighbour {
                                                peer: model::Peer {
                                                    id: key.id,
                                                    ipaddress: key.ipaddress
                                                },
                                                last_seq: 0,
                                                last_ts: 0,
                                                last_seen: 0
                                            }
                                        );
                                    }
                                }
                            }
                            model::UpdateType::Remove => {
                                for key in mgr_msg.update {
                                    let _ = peers.remove(&key.id);
                                }
                            }
                        }
                        let mut new_peer_list = peer_list.lock().await;
                        *new_peer_list = make_peer_list(&peers);
                        log::debug!("Peers updated");
                    }
                } => {}
                _res = async {
                    loop {
                        timer.tick().await;
                        log::debug!("Triggering send event");
                        let _ = self.peer_sender.send(peer_list.lock().await.clone()).await;
                    }
                } => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        self,
        sync::mpsc
    };

    #[test]
    fn test_make_peer_list() {
        let mut neighbour_hashmap: HashMap<Box<str>, model::Neighbour> = HashMap::new();
        neighbour_hashmap.insert(
            "id1".into(),
            model::Neighbour{
                peer: model::Peer{
                    id: "id1".into(),
                    ipaddress: "192.168.1.1".into()
                },
                last_seq: 0,
                last_ts: 0,
                last_seen: 0

            }
        );
        neighbour_hashmap.insert(
            "id2".into(),
            model::Neighbour{
                peer: model::Peer{
                    id: "id2".into(),
                    ipaddress: "192.168.1.2".into()
                },
                last_seq: 0,
                last_ts: 0,
                last_seen: 0

            }
        );
        let neighbour_list = make_peer_list(&neighbour_hashmap);
        let mut target_list = HashMap::new();
        target_list.insert(String::from("192.168.1.1"), true);
        target_list.insert(String::from("192.168.1.2"), true);
        for neighbour in neighbour_list {
            assert!(target_list.remove(&neighbour).is_some());
        }
        assert!(target_list.is_empty());
    }
    #[tokio::test]
    async fn test_measurement() {
        let (tx, rx) = mpsc::channel(1);
        let (mgr_tx, mut mgr_rx) = mpsc::channel(1);
        let neighbour = model::Peer{id: "0".into(), ipaddress: "127.0.0.1".into()};
        let neighbour_update = model::NeighbourUpdate{
            update_type: model::UpdateType::Add,
            update: vec![neighbour]
        };
        tokio::spawn(async move {
            let mut ctrl = PeerController::new("0".into(), rx, mgr_tx).await;
            ctrl.work().await;
        });
        assert!(tx.send(neighbour_update).await.is_ok());
        let result = mgr_rx.recv().await;
        assert!(result.is_some());
        if let Some(measurements) = result {
            for measurement in measurements {
                match measurement.measurement_type {
                    model::MeasurementType::PL => {
                        assert!(measurement.value == 0.0);
                    },
                    model::MeasurementType::Delay => {
                        assert!(measurement.value == 1.0);
                    },
                    _ => {
                        assert!(false);
                    }
                }
            }
        }
    }
}
