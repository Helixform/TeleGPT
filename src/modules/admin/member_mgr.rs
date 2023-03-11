use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Error;
use serde::{Deserialize, Serialize};

use crate::{config::SharedConfig, database::DatabaseManager, modules::prefs::PreferencesManager};

const PUBLIC_USABLE_PREF_KEY: &str = "PublicUsable";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct PublicUsableValue(bool);

impl Default for PublicUsableValue {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Clone)]
pub(crate) struct MemberManager {
    db_mgr: DatabaseManager,
    pref_mgr: PreferencesManager,
    config: SharedConfig,
}

impl MemberManager {
    pub async fn new(
        db_mgr: DatabaseManager,
        pref_mgr: PreferencesManager,
        config: SharedConfig,
    ) -> Result<Self, Error> {
        // Initialize the database table before returning.
        let ok = db_mgr.query(|conn| {
            let sql = "CREATE TABLE IF NOT EXISTS members (username TEXT NOT NULL PRIMARY KEY, disabled INTEGER, created_at INTEGER NOT NULL);";
            conn.execute(sql, ()).unwrap();
            true
        }).await?;
        if !ok {
            return Err(anyhow!("Failed to initialize database table"));
        }

        Ok(Self {
            db_mgr,
            pref_mgr,
            config,
        })
    }

    pub async fn add_member(&self, username: String) -> Result<bool, Error> {
        let unix_timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let result = self
            .db_mgr
            .query(move |conn| {
                let sql = "INSERT OR IGNORE INTO members VALUES (?, 0, ?);";
                let mut stmt = conn.prepare(sql).unwrap();

                match stmt.execute((&username, unix_timestamp_secs)) {
                    Ok(1) => {
                        info!("User \"{}\" is added", username);
                    }
                    Ok(_) => {
                        warn!("User \"{}\" had already been added", username)
                    }
                    Err(err) => {
                        error!("Failed to insert row: {}", err);
                        return false;
                    }
                }

                true
            })
            .await?;

        Ok(result)
    }

    pub async fn delete_member(&self, username: String) -> Result<bool, Error> {
        let result = self
            .db_mgr
            .query(move |conn| {
                let sql = "DELETE FROM members WHERE username = ?";
                let mut stmt = conn.prepare(sql).unwrap();

                match stmt.execute((&username,)) {
                    Ok(1) => {
                        info!("User \"{}\" is deleted", username);
                        return true;
                    }
                    Ok(_) => {
                        warn!("User \"{}\" is not found", username);
                    }
                    Err(err) => {
                        error!("Failed to delete row: {}", err);
                    }
                }

                false
            })
            .await?;

        Ok(result)
    }

    pub async fn is_member_allowed(&self, username: String) -> Result<bool, Error> {
        let public_usable: PublicUsableValue =
            self.pref_mgr.get_value(PUBLIC_USABLE_PREF_KEY).await?;
        if public_usable.0 {
            return Ok(true);
        }

        if self.config.admin_usernames.contains(&username) {
            return Ok(true);
        }

        let result = self
            .db_mgr
            .query(move |conn| {
                let sql = "SELECT username, disabled FROM members WHERE username = ?";
                let disabled_result: Result<bool, _> =
                    conn.query_row(sql, (&username,), |row| row.get(1));

                match disabled_result {
                    Ok(disabled) => !disabled,
                    Err(_) => false,
                }
            })
            .await?;

        Ok(result)
    }

    pub async fn set_public_usable(&self, public_usable: bool) -> Result<(), Error> {
        self.pref_mgr
            .set_value(PUBLIC_USABLE_PREF_KEY, &PublicUsableValue(public_usable))
            .await?;
        Ok(())
    }
}
