use serde::{Serialize, Deserialize};

// internal structures
pub struct Peer {
    pub id: Box<str>,
    pub ipaddress: Box<str>
}

pub struct Neighbour {
    pub peer: Peer,
    pub last_seq: u64,
    pub last_ts: u64,
    pub last_seen: u64
}

// internal messages
pub enum UpdateType {
    Add,
    Remove
}

#[derive(Debug)]
pub enum MeasurementType {
    PL,
    Delay,
    Jitter
}

pub struct NeighbourUpdate {
    pub update_type: UpdateType,
    pub update: Vec<Peer>
}

#[derive(Debug)]
pub struct Measurement {
    pub id: Box<str>,
    pub measurement_type: MeasurementType,
    pub value: f64
}

#[derive(Serialize, Deserialize)]
pub struct Probe {
    pub id: Box<str>,
    pub seq: u64,
    pub ts: u64
}
