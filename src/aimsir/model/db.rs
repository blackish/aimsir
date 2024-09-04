use crate::model;

use sqlx::{query_as, query};
use tonic::async_trait;

#[async_trait]
pub trait Db: Send + Sync {
    async fn get_peers(&mut self) -> Result<Vec::<model::Peer>, sqlx::Error>;
    async fn get_tags(&mut self) -> Result<Vec::<model::Tag>, sqlx::Error>;
    async fn get_tag_levels(&mut self) -> Result<Vec::<model::TagLevel>, sqlx::Error>;
    async fn get_peer_tags(&mut self) -> Result<Vec::<model::PeerTag>, sqlx::Error>;
    async fn add_peer(&mut self, peer: model::Peer) -> Result<(), sqlx::Error>;
    async fn del_peer(&mut self, peer_id: String) -> Result<(), sqlx::Error>;
}

pub struct SqliteDb {
    conn: sqlx::SqlitePool,
}

impl SqliteDb {
    pub async fn new(database_url: String) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = sqlx::SqlitePool::connect(&database_url).await?;
        Ok(Self {conn})
    }
}

#[async_trait]
impl Db for SqliteDb {
    async fn get_peers(&mut self) -> Result<Vec::<model::Peer>, sqlx::Error> {
        let results = query_as!(
            model::Peer,
            "SELECT * FROM peers",
        )
            .fetch_all(&self.conn)
        .await?;
        Ok(results)
    }
    async fn get_tags(&mut self) -> Result<Vec::<model::Tag>, sqlx::Error> {
        let results = query_as!(
            model::Tag,
            "SELECT * FROM tags",
        )
            .fetch_all(&self.conn)
        .await?;
        Ok(results)
    }
    async fn get_tag_levels(&mut self) -> Result<Vec::<model::TagLevel>, sqlx::Error> {
        let results = query_as!(
            model::TagLevel,
            "SELECT * FROM tag_levels",
        )
            .fetch_all(&self.conn)
        .await?;
        Ok(results)
    }
    async fn get_peer_tags(&mut self) -> Result<Vec::<model::PeerTag>, sqlx::Error> {
        let results = query_as!(
            model::PeerTag,
            "SELECT * FROM peer_tags",
        )
            .fetch_all(&self.conn)
        .await?;
        Ok(results)
    }
    async fn add_peer(&mut self, peer: model::Peer) -> Result<(), sqlx::Error> {
        let _results = query!(
            "INSERT INTO peers VALUES ($1, $2)",
            peer.peer_id,
            peer.name
        )
            .execute(&self.conn)
        .await?;
        Ok(())
    }
    async fn del_peer(&mut self, peer_id: String) -> Result<(), sqlx::Error> {
        let _results = query!(
            "DELETE FROM peers WHERE peer_id = $1",
            peer_id,
        )
            .execute(&self.conn)
        .await?;
        Ok(())
    }
}
