use crate::schema;
use crate::model;
use diesel::ExpressionMethods;
use diesel::{
    self,
    query_dsl::methods::SelectDsl,
    Connection,
    RunQueryDsl,
    SelectableHelper
};

pub trait Db {
    fn get_peers(&mut self) -> Result<Vec::<model::Peer>, Box<dyn std::error::Error>>;
    fn add_peer(&mut self, peer: model::Peer) -> Result<(), Box<dyn std::error::Error>>;
    fn del_peer(&mut self, peer_id: String) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct SqliteDb {
    conn: diesel::SqliteConnection,
}

impl SqliteDb {
    pub fn new(database_url: String) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = diesel::sqlite::SqliteConnection::establish(&database_url)?;
        Ok(Self {conn})
    }
}

impl Db for SqliteDb {
    fn get_peers(&mut self) -> Result<Vec::<model::Peer>, Box<dyn std::error::Error>> {
        let results = schema::peers::dsl::peers.select(model::Peer::as_select()).load(&mut self.conn)?;
        Ok(results)
    }
    fn add_peer(&mut self, peer: model::Peer) -> Result<(), Box<dyn std::error::Error>> {
        diesel::insert_into(schema::peers::table)
            .values(peer)
            .execute(&mut self.conn)?;
        Ok(())
    }
    fn del_peer(&mut self, peer_id: String) -> Result<(), Box<dyn std::error::Error>> {
        diesel::delete(schema::peers::table)
            .filter(schema::peers::peer_id.eq(peer_id))
            .execute(&mut self.conn)?;
        Ok(())
    }
}
