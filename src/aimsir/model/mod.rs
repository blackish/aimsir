pub mod aimsir {
    tonic::include_proto!("aimsir");
}
use serde::{Serialize, Deserialize};

pub struct Neighbour {
    pub peer: aimsir::Peer,
    pub last_seq: u64,
    pub last_latency: u64,
    pub last_seen: u64
}

// internal messages
pub enum UpdateType {
    Add,
    Remove
}

impl UpdateType {
    pub fn from_proto(update_type: i32) -> Option<Self> {
        match update_type {
            0 => {
                Some(UpdateType::Add)
            },
            1 => {
                Some(UpdateType::Remove)
            },
            _ => {
                None
            }
        }
    }
}

pub struct NeighbourUpdate {
    pub update_type: UpdateType,
    pub probe_timer: u64,
    pub aggregate_timer: u64,
    pub update: Vec<aimsir::Peer>
}

pub struct PeerUpdate {
    pub probe_timer: u64,
    pub peers: Vec<String>
}

#[derive(Debug)]
pub struct Measurement {
    pub id: String,
    pub count: u16,
    pub pl: u64,
    pub jitter_stddev: f64,
    pub jitter_min: f64,
    pub jitter_max: f64
}

#[derive(Serialize, Deserialize)]
pub struct Probe {
    pub id: String,
    pub seq: u64,
    pub ts: u64
}
