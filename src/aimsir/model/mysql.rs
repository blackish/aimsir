use std::str::FromStr;

use crate::model;

use sqlx::{self, mysql::MySqlConnectOptions};
use tonic::async_trait;

pub struct MysqlDb {
    conn: sqlx::MySqlPool,
}

impl MysqlDb {
    pub async fn new(database_url: String) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = sqlx::MySqlPool::connect_with(
            MySqlConnectOptions::from_str(database_url.as_str())?
                .pipes_as_concat(false)
                .no_engine_substitution(false)
                .timezone(None),
        )
        .await?;
        Ok(Self { conn })
    }
}

#[async_trait]
impl model::db::Db for MysqlDb {
    async fn get_peers(&mut self) -> Result<Vec<model::Peer>, sqlx::Error> {
        let mut conn = self.conn.acquire().await?;
        let results = sqlx::query_as!(model::Peer, "SELECT * FROM peers",)
            .fetch_all(&mut *conn)
            .await?;
        Ok(results)
    }
    async fn get_tags(&mut self) -> Result<Vec<model::Tag>, sqlx::Error> {
        let mut conn = self.conn.acquire().await?;
        let results = sqlx::query_as!(model::Tag, "SELECT * FROM tags",)
            // .fetch_all(&self.conn)
            .fetch_all(&mut *conn)
            .await?;
        Ok(results)
    }
    async fn add_tag(&mut self, tag: model::Tag) -> Result<(), sqlx::Error> {
        let mut conn = self.conn.begin().await?;
        let max_id_request = sqlx::query!("SELECT IFNULL(MAX(id), -1) as id FROM tags",)
            // .fetch_one(&self.conn)
            .fetch_all(&mut *conn)
            .await?;
        let next_id = max_id_request[0].id + 1;
        let _results = sqlx::query!(
            "INSERT INTO tags VALUES (?, ?, ?)",
            next_id,
            tag.parent,
            tag.name,
        )
        .execute(&mut *conn)
        // .execute(&self.conn)
        .await?;
        conn.commit().await?;
        Ok(())
    }
    async fn del_tag(&mut self, tag_id: i32) -> Result<(), sqlx::Error> {
        let mut conn = self.conn.begin().await?;
        let tag = sqlx::query_as!(model::Tag, "SELECT * FROM tags WHERE id = ?", tag_id,)
            .fetch_one(&mut *conn)
            .await?;
        let _results = sqlx::query!(
            "UPDATE tags SET parent = ? WHERE parent = ?",
            tag.parent,
            tag_id
        )
        .execute(&mut *conn)
        // .execute(&self.conn)
        .await?;
        let _results = sqlx::query!("DELETE FROM tags WHERE id = ?", tag_id,)
            .execute(&mut *conn)
            // .execute(&self.conn)
            .await?;
        let _results = sqlx::query!("DELETE FROM peer_tags WHERE tag_id = ?", tag_id,)
            .execute(&mut *conn)
            // .execute(&self.conn)
            .await?;
        conn.commit().await?;
        Ok(())
    }
    async fn add_peer_tag(&mut self, peer_tag: model::PeerTag) -> Result<(), sqlx::Error> {
        let mut conn = self.conn.begin().await?;
        let _results = sqlx::query!(
            "INSERT INTO peer_tags VALUES (?, ?)",
            peer_tag.peer_id,
            peer_tag.tag_id,
        )
        .execute(&mut *conn)
        // .execute(&self.conn)
        .await?;
        conn.commit().await?;
        Ok(())
    }
    async fn del_peer_tag(&mut self, peer_id: String, tag_id: i32) -> Result<(), sqlx::Error> {
        let mut conn = self.conn.begin().await?;
        let _results = sqlx::query!(
            "DELETE FROM peer_tags WHERE peer_id = ? AND tag_id = ?",
            peer_id,
            tag_id,
        )
        .execute(&mut *conn)
        .await?;
        conn.commit().await?;
        Ok(())
    }
    async fn get_peer_tags(&mut self) -> Result<Vec<model::PeerTag>, sqlx::Error> {
        let mut conn = self.conn.acquire().await?;
        let results = sqlx::query_as!(model::PeerTag, "SELECT * FROM peer_tags",)
            .fetch_all(&mut *conn)
            .await?;
        Ok(results)
    }
    async fn add_peer(&mut self, peer: model::Peer) -> Result<(), sqlx::Error> {
        let mut conn = self.conn.begin().await?;
        let _results = sqlx::query!(
            "INSERT INTO peers VALUES (?, ?, ?)",
            peer.peer_id,
            peer.name,
            peer.maintenance
        )
        .execute(&mut *conn)
        .await?;
        conn.commit().await?;
        Ok(())
    }
    async fn del_peer(&mut self, peer_id: String) -> Result<(), sqlx::Error> {
        let mut conn = self.conn.begin().await?;
        let _results = sqlx::query!("DELETE FROM peers WHERE peer_id = ?", peer_id,)
            .execute(&mut *conn)
            .await?;
        conn.commit().await?;
        Ok(())
    }
    async fn disable_peer(&mut self, peer_id: String) -> Result<(), sqlx::Error> {
        let mut conn = self.conn.begin().await?;
        let _results = sqlx::query!(
            "UPDATE peers SET maintenance = true WHERE peer_id = ?",
            peer_id,
        )
        .execute(&mut *conn)
        .await?;
        conn.commit().await?;
        Ok(())
    }
    async fn enable_peer(&mut self, peer_id: String) -> Result<(), sqlx::Error> {
        let mut conn = self.conn.begin().await?;
        let _results = sqlx::query!(
            "UPDATE peers SET maintenance = false WHERE peer_id = ?",
            peer_id,
        )
        .execute(&mut *conn)
        .await?;
        conn.commit().await?;
        Ok(())
    }
}
