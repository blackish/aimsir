pub mod aimsir {
    tonic::include_proto!("aimsir");
}
pub mod db;
pub mod mysql;
use serde::{Deserialize, Serialize};
use std::{
    time::{SystemTime, UNIX_EPOCH},
    u32,
};

pub struct Neighbour {
    pub peer: aimsir::Peer,
    pub last_seq: u64,
    pub last_latency: u64,
    pub last_seen: u64,
}

// internal messages
pub enum UpdateType {
    Add,
    Remove,
    Full,
}

impl UpdateType {
    pub fn from_proto(update_type: i32) -> Option<Self> {
        match update_type {
            0 => Some(UpdateType::Add),
            1 => Some(UpdateType::Remove),
            2 => Some(UpdateType::Full),
            _ => None,
        }
    }
}

pub struct NeighbourUpdate {
    pub update_type: UpdateType,
    pub probe_timer: u64,
    pub aggregate_timer: u64,
    pub update: Vec<aimsir::Peer>,
}

pub struct PeerUpdate {
    pub probe_timer: u64,
    pub peers: Vec<String>,
}

#[derive(Debug)]
pub struct Measurement {
    pub id: String,
    pub count: u16,
    pub pl: u64,
    pub jitter_stddev: f64,
    pub jitter_min: f64,
    pub jitter_max: f64,
}

#[derive(Debug)]
pub struct PeerMeasurement {
    pub pl: u64,
    pub jitters: Vec<f64>,
}

impl PeerMeasurement {
    pub fn make_measurement(&self, peer_id: String) -> Measurement {
        let mut jitter_min: f64 = -1.0;
        let mut jitter_max: f64 = -1.0;
        let count: f64 = self.jitters.len() as f64;
        let avg: f64 = self.jitters.iter().fold(0.0, |acc, x| acc + x) / count;
        let jitter_stddev: f64 = (self
            .jitters
            .iter()
            .fold(0.0, |acc, x| acc + ((x - avg) * (x - avg)))
            / count)
            .sqrt();
        self.jitters.iter().for_each(|x| {
            if *x < jitter_min || jitter_min == -1.0 {
                jitter_min = x.clone();
            }
            if *x > jitter_max || jitter_max == -1.0 {
                jitter_max = x.clone();
            }
        });
        Measurement {
            id: peer_id,
            count: count as u16,
            pl: self.pl,
            jitter_stddev: jitter_stddev,
            jitter_min: jitter_min,
            jitter_max: jitter_max,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Probe {
    pub id: String,
    pub seq: u64,
    pub ts: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct StoreMetric {
    pub ts: u64,
    pub pl: u32,
    pub jitter_stddev: f32,
    pub jitter_min: f32,
    pub jitter_max: f32,
}

impl StoreMetric {
    pub fn new(metric: aimsir::Metric) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let mut new_metric = Self {
            ts,
            pl: 0,
            jitter_min: -1.0,
            jitter_max: -1.0,
            jitter_stddev: -1.0,
        };
        new_metric.update(metric);
        new_metric
    }
    pub fn new_empty() -> Self {
        let new_metric = Self {
            ts: 0,
            pl: 0,
            jitter_min: -1.0,
            jitter_max: -1.0,
            jitter_stddev: -1.0,
        };
        new_metric
    }
    pub fn update(&mut self, metric: aimsir::Metric) {
        self.ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        match aimsir::MetricType::try_from(metric.metric_type) {
            Ok(aimsir::MetricType::Pl) => self.pl = metric.value as u32,
            Ok(aimsir::MetricType::JitterMin) => self.jitter_min = metric.value,
            Ok(aimsir::MetricType::JitterMax) => self.jitter_max = metric.value,
            Ok(aimsir::MetricType::JitterStdDev) => self.jitter_stddev = metric.value,
            _ => {}
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Peer {
    pub peer_id: String,
    pub name: String,
    pub maintenance: Option<i8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Tag {
    pub id: Option<i32>,
    pub parent: Option<i32>,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PeerTag {
    pub peer_id: String,
    pub tag_id: i32,
}
