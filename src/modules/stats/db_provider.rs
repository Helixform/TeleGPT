use anyhow::Error;
use rusqlite::Connection;

pub trait DatabaseProvider {
    fn provide_db(&self) -> Result<Connection, Error>;
}

pub struct InMemDatabaseProvider;

impl DatabaseProvider for InMemDatabaseProvider {
    fn provide_db(&self) -> Result<Connection, Error> {
        let conn = Connection::open_in_memory()?;
        Ok(conn)
    }
}
