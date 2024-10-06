use crate::model;

use sqlx;
use tonic::async_trait;

pub struct SqliteDb {
    conn: sqlx::SqlitePool,
}

impl SqliteDb {
    pub async fn new(database_url: String) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = sqlx::SqlitePool::connect(&database_url).await?;
        Ok(Self { conn })
    }
}

#[async_trait]
impl model::db::Db for SqliteDb {
    async fn get_peers(&mut self) -> Result<Vec<model::Peer>, sqlx::Error> {
        let results = sqlx::query_as!(model::Peer, "SELECT * FROM peers",)
            .fetch_all(&self.conn)
            .await?;
        Ok(results)
    }
    async fn get_tags(&mut self) -> Result<Vec<model::Tag>, sqlx::Error> {
        let results = sqlx::query_as!(model::Tag, "SELECT * FROM tags",)
            .fetch_all(&self.conn)
            .await?;
        Ok(results)
    }
    async fn add_tag(&mut self, tag: model::Tag) -> Result<(), sqlx::Error> {
        let max_id_request = sqlx::query!("SELECT IFNULL(MAX(id), -1) as id FROM tags",)
            .fetch_one(&self.conn)
            .await?;
        let next_id = max_id_request.id + 1;
        let _results = sqlx::query!(
            "INSERT INTO tags VALUES ($1, $2, $3)",
            next_id,
            tag.level,
            tag.name,
        )
        .execute(&self.conn)
        .await?;
        Ok(())
    }
    async fn del_tag(&mut self, tag_id: i64) -> Result<(), sqlx::Error> {
        let _results = sqlx::query!("DELETE FROM tags WHERE id = $1", tag_id,)
            .execute(&self.conn)
            .await?;
        let _results = sqlx::query!("DELETE FROM peer_tags WHERE tag_id = $1", tag_id,)
            .execute(&self.conn)
            .await?;
        Ok(())
    }
    async fn get_tag_levels(&mut self) -> Result<Vec<model::TagLevel>, sqlx::Error> {
        let results = sqlx::query_as!(model::TagLevel, "SELECT * FROM tag_levels",)
            .fetch_all(&self.conn)
            .await?;
        Ok(results)
    }
    async fn add_tag_level(&mut self, level: model::TagLevel) -> Result<(), sqlx::Error> {
        let max_id_request = sqlx::query!("SELECT IFNULL(MAX(id), -1) as id FROM tag_levels",)
            .fetch_one(&self.conn)
            .await?;
        let next_id = max_id_request.id + 1;
        let _results = sqlx::query!(
            "INSERT INTO tag_levels VALUES ($1, $2, $3)",
            next_id,
            level.parent,
            level.name,
        )
        .execute(&self.conn)
        .await?;
        Ok(())
    }
    async fn del_tag_level(&mut self, level_id: i64) -> Result<(), sqlx::Error> {
        let _results = sqlx::query!(
            "DELETE FROM peer_tags WHERE tag_id in (SELECT id FROM tags WHERE level = $1)",
            level_id
        )
        .execute(&self.conn)
        .await?;
        let _results = sqlx::query!("DELETE FROM tags WHERE level = $1", level_id,)
            .execute(&self.conn)
            .await?;
        let _results = sqlx::query!(
            "UPDATE tag_levels SET parent=(SELECT parent FROM tag_levels WHERE id = $1 LIMIT 1) WHERE parent = $2",
            level_id,
            level_id,
        )
        .execute(&self.conn)
        .await?;
        let _results = sqlx::query!("DELETE FROM tag_levels WHERE id = $1", level_id,)
            .execute(&self.conn)
            .await?;
        Ok(())
    }
    async fn add_peer_tag(&mut self, peer_tag: model::PeerTag) -> Result<(), sqlx::Error> {
        let _results = sqlx::query!(
            "INSERT INTO peer_tags VALUES ($1, $2)",
            peer_tag.peer_id,
            peer_tag.tag_id,
        )
        .execute(&self.conn)
        .await?;
        Ok(())
    }
    async fn del_peer_tag(&mut self, peer_id: String, tag_id: i64) -> Result<(), sqlx::Error> {
        let _results = sqlx::query!(
            "DELETE FROM peer_tags WHERE peer_id = $1 AND tag_id = $2",
            peer_id,
            tag_id,
        )
        .execute(&self.conn)
        .await?;
        Ok(())
    }
    async fn get_peer_tags(&mut self) -> Result<Vec<model::PeerTag>, sqlx::Error> {
        let results = sqlx::query_as!(model::PeerTag, "SELECT * FROM peer_tags",)
            .fetch_all(&self.conn)
            .await?;
        Ok(results)
    }
    async fn add_peer(&mut self, peer: model::Peer) -> Result<(), sqlx::Error> {
        let _results = sqlx::query!("INSERT INTO peers VALUES ($1, $2)", peer.peer_id, peer.name)
            .execute(&self.conn)
            .await?;
        Ok(())
    }
    async fn del_peer(&mut self, peer_id: String) -> Result<(), sqlx::Error> {
        let _results = sqlx::query!("DELETE FROM peers WHERE peer_id = $1", peer_id,)
            .execute(&self.conn)
            .await?;
        Ok(())
    }
}
