pub mod aimsir {
    tonic::include_proto!("aimsir");
}
pub mod db;
use diesel::prelude::*;
use std::{time::{SystemTime, UNIX_EPOCH}, u32};
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
    Remove,
    Full,
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
            2 => {
                Some(UpdateType::Full)
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

#[derive(Clone,Debug)]
pub struct StoreMetric {
    pub ts: u64,
    pub pl: u32,
    pub jitter_stddev: f32,
    pub jitter_min: f32,
    pub jitter_max: f32
}

impl StoreMetric {
    pub fn new(metric: aimsir::Metric) -> Self {
        let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
        let mut new_metric = Self{ts, pl: 0, jitter_min: -1.0, jitter_max: -1.0, jitter_stddev: -1.0, };
        new_metric.update(metric);
        new_metric
    }
    pub fn new_empty() -> Self {
        let new_metric = Self{ts: 0, pl: 0, jitter_min: -1.0, jitter_max: -1.0, jitter_stddev: -1.0, };
        new_metric
    }
    pub fn update(&mut self, metric: aimsir::Metric) {
        self.ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
        match aimsir::MetricType::try_from(metric.metric_type) {
            Ok(aimsir::MetricType::Pl) => {
                self.pl = metric.value as u32
            },
            Ok(aimsir::MetricType::JitterMin) => {
                self.jitter_min = metric.value
            },
            Ok(aimsir::MetricType::JitterMax) => {
                self.jitter_max = metric.value
            },
            Ok(aimsir::MetricType::JitterStdDev) => {
                self.jitter_stddev = metric.value
            },
            _ => {}
        }
    }
}

#[derive(Insertable, Queryable, Selectable)]
#[diesel(table_name = crate::schema::peers)]
pub struct Peer {
    pub peer_id: String,
    pub name: String,
}

#[derive(Insertable, Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::tags)]
pub struct Tag {
    pub id: i32,
    pub level: i32,
    pub name: String,
}

#[derive(Insertable, Queryable, Selectable)]
#[diesel(table_name = crate::schema::tag_levels)]
pub struct TagLevel {
    pub id: i32,
    pub parent: Option<i32>,
    pub name: String,
}

#[derive(Insertable, Queryable, Selectable)]
#[diesel(table_name = crate::schema::peer_tags)]
pub struct PeerTag {
    pub peer_id: String,
    pub tag_id: i32,
}
