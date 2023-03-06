use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Error;
use rusqlite::{Connection as SqliteConnection, OptionalExtension};

use crate::database::DatabaseManager;

#[derive(Clone)]
pub(crate) struct StatsManager {
    db_mgr: DatabaseManager,
}

impl StatsManager {
    pub async fn with_db_manager(db_mgr: DatabaseManager) -> Result<Self, Error> {
        // Initialize the database table before returning.
        let ok = db_mgr.query(|conn| {
            let sql = "CREATE TABLE IF NOT EXISTS token_usage (user_id TEXT NOT NULL, time INTEGER NOT NULL, tokens INTEGER NOT NULL, PRIMARY KEY (user_id, time));";
            conn.execute(sql, ()).unwrap();
            true
        }).await?;
        if !ok {
            return Err(anyhow!("Failed to initialize database table"));
        }

        Ok(Self { db_mgr })
    }

    pub async fn add_usage(&self, user_id: String, tokens: i64) -> Result<(), Error> {
        let now = SystemTime::now();
        let unix_timestamp = now.duration_since(UNIX_EPOCH).unwrap();
        let hour_grouped_timestamp_secs: i64 = (unix_timestamp.as_secs() / 3600 * 3600) as _;

        self.db_mgr.enqueue_work(move |conn| {
            let sql = "INSERT OR REPLACE INTO token_usage VALUES (?, ?, COALESCE((SELECT tokens FROM token_usage WHERE user_id = ? AND time = ?), 0) + ?);";
            let mut stmt = conn.prepare(sql).unwrap();

            let user_id = &user_id;
            let time = hour_grouped_timestamp_secs;
            let updated_rows = stmt.execute((user_id, time, user_id, time, tokens)).unwrap_or(0);
            if updated_rows != 1 {
                error!("Unexpected updated rows: {}", updated_rows);
            }
        }).await?;

        Ok(())
    }

    pub async fn query_usage(&self, user_id: Option<String>) -> Result<i64, Error> {
        let usage = self
            .db_mgr
            .query(|conn| {
                let usage = if let Some(user_id) = user_id {
                    Self::query_usage_of_user(conn, &user_id)
                } else {
                    Self::query_total_usage(conn)
                };

                match usage {
                    Ok(usage) => usage,
                    Err(err) => {
                        error!("Failed to query usage: {}", err);
                        0
                    }
                }
            })
            .await?;

        Ok(usage)
    }
}

impl StatsManager {
    fn query_usage_of_user(conn: &mut SqliteConnection, user_id: &str) -> Result<i64, Error> {
        let sql = "SELECT SUM(tokens) FROM token_usage WHERE user_id = ?";
        let result = conn
            .query_row(sql, (user_id,), |row| row.get(0))
            .optional()?;
        Ok(result.unwrap_or(0))
    }

    fn query_total_usage(conn: &mut SqliteConnection) -> Result<i64, Error> {
        let sql = "SELECT SUM(tokens) FROM token_usage";
        let result = conn.query_row(sql, (), |row| row.get(0)).optional()?;
        Ok(result.unwrap_or(0))
    }
}
