use crate::model;

use sqlx;
use tonic::async_trait;

#[async_trait]
pub trait Db: Send + Sync {
    async fn get_peers(&mut self) -> Result<Vec<model::Peer>, sqlx::Error>;
    async fn get_tags(&mut self) -> Result<Vec<model::Tag>, sqlx::Error>;
    async fn get_tag_levels(&mut self) -> Result<Vec<model::TagLevel>, sqlx::Error>;
    async fn get_peer_tags(&mut self) -> Result<Vec<model::PeerTag>, sqlx::Error>;
    async fn add_peer(&mut self, peer: model::Peer) -> Result<(), sqlx::Error>;
    async fn del_peer(&mut self, peer_id: String) -> Result<(), sqlx::Error>;
    async fn add_tag(&mut self, tag: model::Tag) -> Result<(), sqlx::Error>;
    async fn del_tag(&mut self, tag_id: i32) -> Result<(), sqlx::Error>;
    async fn add_tag_level(&mut self, level: model::TagLevel) -> Result<(), sqlx::Error>;
    async fn del_tag_level(&mut self, level_id: i32) -> Result<(), sqlx::Error>;
    async fn add_peer_tag(&mut self, peer_tag: model::PeerTag) -> Result<(), sqlx::Error>;
    async fn del_peer_tag(&mut self, peer_id: String, tag_id: i32) -> Result<(), sqlx::Error>;
}
