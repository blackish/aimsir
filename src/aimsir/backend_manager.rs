use std::{
    collections::HashMap,
    sync::Arc,
    time,
};
use rocket::{self, form::validate::Len};
use tokio::{
    self,
    time as async_time,
    sync::RwLock,
};

use crate::model;

struct LevelMetrics {
    levels: HashMap<i32, Vec<BackendTag>>
}

struct BackendPeer {
    peer_id: String,
    levels: HashMap<i32, BackendTag>,
}

struct BackendTag {
    tag_id: i32,
    values: HashMap<i32, model::StoreMetric>,
}

pub async fn render_results(
    metrics: Arc<RwLock<HashMap<String, HashMap<String, model::StoreMetric>>>>,
    mut db: Box<dyn model::db::Db>,
    reconcile_time: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let sleep_duration = time::Duration::from_secs(reconcile_time.into());
    loop {
        async_time::sleep(sleep_duration).await;
        let _peers = db.get_peers()?;
        let tags = db.get_tags()?;
        let tag_levels = db.get_tag_levels()?;
        let _peer_tags = db.get_peer_tags()?;
        let mut levels: HashMap<i32, HashMap<i32, BackendTag>> = HashMap::with_capacity(tag_levels.len());
        for level in &tag_levels {
            let tag_list: Vec<model::Tag> = tags.clone().into_iter().filter(|x| x.level == level.id).collect();
            let mut tags_in_level: HashMap<i32, BackendTag> = HashMap::with_capacity(tag_list.len());
            for tag in &tag_list {
                tags_in_level.insert(
                    tag.id,
                    BackendTag {
                        tag_id: tag.id,
                        values: tag_list
                                .clone()
                                .into_iter()
                                .map(
                                    |x|
                                    (x.id, model::StoreMetric::new_empty())
                                )
                                .collect()
                    }
                );
            }
            levels.insert(level.id, tags_in_level);
        }

        {
            let _local_metrics = metrics.read().await;

        }
    }
    // Ok(())
}
