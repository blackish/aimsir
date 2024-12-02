use log;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::UdpSocket,
    select,
    sync::mpsc::{channel, Receiver, Sender},
    time::{self, Duration},
};

use flexbuffers::{FlexbufferSerializer, Reader};
use serde::{self, Deserialize, Serialize};

use crate::constants;
use crate::model;

pub struct PeerController {
    probe_timer: u64,
    aggregate_timer: u64,
    peers: HashMap<String, model::Neighbour>,
    peer_receiver: Receiver<model::Probe>,
    peer_sender: Sender<model::PeerUpdate>,
    manager_receiver: Receiver<model::NeighbourUpdate>,
    manager_sender: Sender<Vec<model::Measurement>>,
    local_id: Box<str>,
    test: bool,
}

fn make_peer_list(peers: &HashMap<String, model::Neighbour>) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for key in peers.values() {
        result.push(String::from(key.peer.ipaddress.clone()));
    }
    return result;
}

impl PeerController {
    pub async fn new(
        local_id: Box<str>,
        probe_timer: u64,
        aggregate_timer: u64,
        mgr_rcv: Receiver<model::NeighbourUpdate>,
        mgr_send: Sender<Vec<model::Measurement>>,
        test: bool,
    ) -> Self {
        let (tx, mut rx) = channel::<model::PeerUpdate>(2);
        let (receiver_tx, receiver_rx) = channel::<model::Probe>(1_024);
        let mut listen_address = String::from("0.0.0.0");
        listen_address.push_str(constants::PORT);
        let udp_socket = UdpSocket::bind(listen_address).await.unwrap();
        let udp_sender = Arc::new(udp_socket);
        let udp_receiver = udp_sender.clone();
        let sender_probe_timer = probe_timer.clone();
        let id = local_id.clone();
        log::info!("Start sending thread");
        if !test {
            tokio::spawn(async move {
                let mut seq: u64 = 0;
                let mut peer_list: Vec<String> = Vec::new();
                let mut timer = time::interval(Duration::from_secs(sender_probe_timer));
                loop {
                    select! {
                        res = rx.recv() => {
                            if let Some(new_peer_list) = res {
                                peer_list = new_peer_list.peers;
                                timer = time::interval(Duration::from_secs(new_peer_list.probe_timer));
                            }
                        },
                        _res = timer.tick() => {
                            if peer_list.len() > 0 {
                                let ts = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap()
                                            .as_millis() as u64;

                                let probe = model::Probe {
                                    id: id.to_string(),
                                    seq: seq.clone(),
                                    ts: ts as u64
                                };
                                let mut serializer = FlexbufferSerializer::new();
                                probe.serialize(&mut serializer).unwrap();
                                for dst_addr in &peer_list[..] {
                                    let mut dst = dst_addr.clone();
                                    dst.push_str(constants::PORT);
                                    let _ = udp_sender.send_to(serializer.view(), dst).await;
                                };
                                seq += 1;
                            }
                        }
                    }
                }
            });
            log::info!("Start receiving thread");
            tokio::spawn(async move {
                let mut buf = [0; 1024];
                loop {
                    let (len, addr) = udp_receiver.recv_from(&mut buf).await.unwrap();
                    log::debug!("Got a package from {}", addr);
                    if let Ok(reader) = Reader::get_root(&buf[..len]) {
                        if let Ok(probe) = model::Probe::deserialize(reader) {
                            log::debug!("Decoded a package: seq={}, ts={}", probe.seq, probe.ts);
                            let _ = receiver_tx.send(probe).await;
                        }
                    }
                }
            });
        }
        Self {
            peers: HashMap::new(),
            probe_timer,
            aggregate_timer,
            peer_receiver: receiver_rx,
            peer_sender: tx,
            manager_receiver: mgr_rcv,
            manager_sender: mgr_send,
            local_id,
            test,
        }
    }
    pub async fn work(&mut self) {
        let mut peer_stats: HashMap<String, model::Measurement> = HashMap::new();
        let mut last_aggregate_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let mut aggregate_timer = time::interval(Duration::from_secs(self.aggregate_timer));
        let _ = aggregate_timer.tick().await;
        loop {
            select! {
                res = self.peer_receiver.recv() => {
                    if let Some(peer_msg) = res {
                        self.peers
                            .entry(peer_msg.id.to_string())
                            .and_modify(|entry|
                                {
                                    let current_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                                    let latency = (current_ts as u64) - peer_msg.ts;
                                    if entry.last_latency < u64::MAX {
                                        let jitter = (latency as f64 - entry.last_latency as f64).abs();
                                        let pl : u64;
                                        if entry.last_seq > peer_msg.seq {
                                            pl = (peer_msg.seq - 1) as u64;
                                        } else {
                                            pl = (peer_msg.seq - entry.last_seq - 1) as u64;
                                        }
                                        peer_stats.entry(peer_msg.id.to_string())
                                        .and_modify(|stat_entry|
                                            {
                                                stat_entry.pl += pl;
                                                stat_entry.count += 1;
                                                stat_entry.jitter_stddev += jitter.abs();
                                                if jitter > stat_entry.jitter_max {
                                                    stat_entry.jitter_max = jitter;
                                                }
                                                if jitter < stat_entry.jitter_min || stat_entry.jitter_min == 0.0 {
                                                    stat_entry.jitter_min = jitter;
                                                }
                                                log::debug!("Got probe from: {}. jitter: {}, pl: {}", peer_msg.id, jitter, stat_entry.pl);
                                            }).or_insert_with(||
                                                {
                                                    let mut new_entry = model::Measurement{
                                                        id: peer_msg.id.clone(),
                                                        count: 1,
                                                        pl: pl,
                                                        jitter_stddev: jitter,
                                                        jitter_min: jitter,
                                                        jitter_max: jitter
                                                    };
                                                    if new_entry.pl > 1 {
                                                        let estimate_pl = (current_ts as u64 - last_aggregate_ts as u64)/(self.probe_timer * 1000);
                                                        new_entry.pl = estimate_pl as u64;
                                                    };
                                                    log::debug!("Got probe from: {}. jitter: {}, pl: {}", peer_msg.id, jitter, new_entry.pl);
                                                    new_entry
                                                }
                                            );
                                    }
                                    entry.last_latency = current_ts as u64 - peer_msg.ts;
                                    entry.last_seq = peer_msg.seq;
                                    entry.last_seen = current_ts as u64;
                                }
                            );
                    }
                },
                res = self.manager_receiver.recv() => {
                    if let Some(mgr_msg) = res {
                        log::debug!("Got peer update");
                        match mgr_msg.update_type {
                            model::UpdateType::Full => {
                                _ = self.peers.drain();
                                for key in mgr_msg.update {
                                    if key.id == self.local_id.to_string() {
                                        continue;
                                    }
                                    self.peers.insert(key.id.clone(),
                                            model::Neighbour {
                                                peer: model::aimsir::Peer {
                                                    id: key.id,
                                                    ipaddress: key.ipaddress
                                                },
                                                last_seq: 0,
                                                last_latency: u64::MAX,
                                                last_seen: 0
                                            }
                                        );
                                }

                            }
                            model::UpdateType::Add => {
                                for key in mgr_msg.update {
                                    if key.id == self.local_id.to_string() {
                                        continue;
                                    }
                                    self.peers
                                        .entry(key.id.clone())
                                        .and_modify(|x| x.peer.ipaddress=key.ipaddress.clone())
                                        .or_insert(
                                            model::Neighbour {
                                                peer: model::aimsir::Peer {
                                                    id: key.id,
                                                    ipaddress: key.ipaddress
                                                },
                                                last_seq: 0,
                                                last_latency: u64::MAX,
                                                last_seen: 0
                                            }
                                        );
                                }
                            }
                            model::UpdateType::Remove => {
                                for key in mgr_msg.update {
                                    let _ = self.peers.remove(&key.id);
                                }
                            }
                        }
                        self.probe_timer = mgr_msg.probe_timer;
                        self.aggregate_timer = mgr_msg.aggregate_timer;
                        if !self.test {
                            let _ = self.peer_sender.send(
                                model::PeerUpdate{
                                    peers: make_peer_list(&self.peers),
                                    probe_timer: self.probe_timer.clone()
                                }
                            ).await;
                        }
                        log::debug!("Peers updated");
                    }
                },
                _res = aggregate_timer.tick() => {
                    last_aggregate_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                    let result = self.make_aggregate(&mut peer_stats, last_aggregate_ts as u64);
                    if !self.test {
                        let _ = self.manager_sender.send(result).await;
                    };
                }
            }
        }
    }
    pub fn make_aggregate(
        &self,
        peer_stats: &mut HashMap<String, model::Measurement>,
        ts: u64,
    ) -> Vec<model::Measurement> {
        let mut result = Vec::new();
        for peer in self.peers.values() {
            let last_stat_ts = (ts as u64 - peer.last_seen) / (1000 * self.probe_timer);
            let mut stat = peer_stats
                .remove(&peer.peer.id)
                .unwrap_or(model::Measurement {
                    id: peer.peer.id.clone().into(),
                    count: 0,
                    pl: 0,
                    jitter_stddev: 0.0,
                    jitter_min: 0.0,
                    jitter_max: 0.0,
                });
            if last_stat_ts > 0 {
                stat.pl += last_stat_ts as u64;
            }
            if stat.count > 0 {
                stat.jitter_stddev /= stat.count as f64;
            };
            log::debug!("Aggregate: {} {} {} {} {}", stat.id, stat.pl, stat.jitter_stddev, stat.jitter_max, stat.jitter_min);
            result.push(stat);
        }
        return result;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{self, sync::mpsc};

    #[test]
    fn test_make_peer_list() {
        let mut neighbour_hashmap: HashMap<String, model::Neighbour> = HashMap::new();
        neighbour_hashmap.insert(
            String::from("id1"),
            model::Neighbour {
                peer: model::aimsir::Peer {
                    id: String::from("id1"),
                    ipaddress: String::from("192.168.1.1"),
                },
                last_seq: 0,
                last_latency: 0,
                last_seen: 0,
            },
        );
        neighbour_hashmap.insert(
            String::from("id2"),
            model::Neighbour {
                peer: model::aimsir::Peer {
                    id: "id2".into(),
                    ipaddress: "192.168.1.2".into(),
                },
                last_seq: 0,
                last_latency: 0,
                last_seen: 0,
            },
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
        let _ = env_logger::try_init();
        let (tx, rx) = mpsc::channel(1);
        let (mgr_tx, mut mgr_rx) = mpsc::channel(1);
        let neighbour = model::aimsir::Peer {
            id: "0".into(),
            ipaddress: "127.0.0.1".into(),
        };
        let neighbour_update = model::NeighbourUpdate {
            update_type: model::UpdateType::Add,
            probe_timer: 1,
            aggregate_timer: 10,
            update: vec![neighbour],
        };
        tokio::spawn(async move {
            let mut ctrl = PeerController::new("0".into(), 1, 10, rx, mgr_tx, false).await;
            ctrl.work().await;
        });
        assert!(tx.send(neighbour_update).await.is_ok());
        let result = mgr_rx.recv().await;
        assert!(result.is_some());
        if let Some(measurements) = result {
            for measurement in measurements {
                assert!(measurement.pl == 0);
                assert!(measurement.jitter_min < 1.1);
            }
        }
    }
}
