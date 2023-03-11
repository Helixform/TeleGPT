use std::fmt::Debug;

use anyhow::Error;
use serde::{de::DeserializeOwned, Serialize};

use crate::database::DatabaseManager;

#[derive(Clone)]
pub(crate) struct PreferencesManager {
    db_mgr: DatabaseManager,
}

impl PreferencesManager {
    pub async fn with_db_manager(db_mgr: DatabaseManager) -> Result<Self, Error> {
        // Initialize the database table before returning.
        let ok = db_mgr.query(|conn| {
            let sql = "CREATE TABLE IF NOT EXISTS preferences (pref_key TEXT NOT NULL PRIMARY KEY, value TEXT);";
            conn.execute(sql, ()).unwrap();
            true
        }).await?;
        if !ok {
            return Err(anyhow!("Failed to initialize database table"));
        }

        Ok(Self { db_mgr })
    }

    pub async fn set_value<V>(&self, key: &str, value: &V) -> Result<(), Error>
    where
        V: Serialize,
    {
        let key = key.to_owned();
        let serialized_value = serde_json::to_string(value)?;

        self.db_mgr
            .enqueue_work(move |conn| {
                let sql = "INSERT OR REPLACE INTO preferences VALUES (?, ?);";
                let mut stmt = conn.prepare(sql).unwrap();

                match stmt.execute((key, serialized_value)) {
                    Ok(1) => {}
                    Ok(updated_row) => {
                        error!("Unexpected updated rows: {}", updated_row)
                    }
                    Err(err) => {
                        error!("Failed to insert row: {}", err);
                    }
                }
            })
            .await?;

        Ok(())
    }

    pub async fn get_value<V>(&self, key: &str) -> Result<V, Error>
    where
        V: DeserializeOwned + Default + Send + Debug + 'static,
    {
        let key = key.to_owned();
        let value = self
            .db_mgr
            .query(move |conn| {
                let sql = "SELECT value FROM preferences WHERE pref_key = ?";
                let value_str = conn.query_row(sql, (key,), |row| row.get(0) as Result<String, _>);
                if let Ok(value_str) = value_str {
                    serde_json::from_str(&value_str)
                } else {
                    Ok(V::default())
                }
            })
            .await
            .and_then(|res| res.map_err(|err| anyhow!(err)))?;

        Ok(value)
    }
}
