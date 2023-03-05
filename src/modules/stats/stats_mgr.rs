use std::sync::Arc;
use std::thread::{Builder as ThreadBuilder, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Error;
use rusqlite::{Connection as SqliteConnection, OptionalExtension};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::oneshot::channel as oneshot_channel;

use super::DatabaseProvider;

pub struct StatsManager {
    inner: Arc<StatsManagerInner>,
}

pub struct StatsManagerInner {
    join_handle: Option<JoinHandle<()>>,
    work_tx: Option<Sender<Box<dyn DatabaseThreadWork>>>,
}

impl StatsManager {
    pub async fn with_db_provider<P>(provider: P) -> Result<Self, Error>
    where
        P: DatabaseProvider,
    {
        let conn = provider.provide_db()?;
        let (work_tx, work_rx) = channel(10);

        let db_thread = StatsManagerDatabaseThread { conn, work_rx };
        let join_handle = db_thread.start();

        // Initialize the database before returning.
        let (res_tx, res_rx) = oneshot_channel();
        work_tx
            .send(AnyDatabaseThreadWork::new_boxed(move |thread| {
                res_tx.send(thread.init_tables()).unwrap();
            }))
            .await
            .ok()
            .unwrap();
        res_rx.await.unwrap()?;

        Ok(Self {
            inner: Arc::new(StatsManagerInner {
                join_handle: Some(join_handle),
                work_tx: Some(work_tx),
            }),
        })
    }

    pub async fn add_usage(&self, user_id: String, tokens: i64) -> bool {
        let now = SystemTime::now();
        let unix_timestamp = now.duration_since(UNIX_EPOCH).unwrap();
        let hours: i64 = (unix_timestamp.as_secs() / 3600 * 3600) as _;

        let (res_tx, res_rx) = oneshot_channel();
        self.inner
            .work_tx
            .as_ref()
            .unwrap()
            .send(AnyDatabaseThreadWork::new_boxed(move |thread| {
                let ok = thread.add_usage(&user_id, hours, tokens).unwrap_or(false);
                res_tx.send(ok).unwrap();
            }))
            .await
            .ok()
            .unwrap();
        res_rx.await.unwrap()
    }

    pub async fn query_usage(&self, user_id: Option<String>) -> i64 {
        let (res_tx, res_rx) = oneshot_channel();
        self.inner
            .work_tx
            .as_ref()
            .unwrap()
            .send(AnyDatabaseThreadWork::new_boxed(move |thread| {
                let usage = if let Some(user_id) = user_id {
                    thread.query_usage_of_user(&user_id)
                } else {
                    thread.query_total_usage()
                }
                .unwrap_or(0);
                res_tx.send(usage).unwrap();
            }))
            .await
            .ok()
            .unwrap();
        res_rx.await.unwrap()
    }
}

impl Clone for StatsManager {
    fn clone(&self) -> Self {
        StatsManager {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Drop for StatsManagerInner {
    fn drop(&mut self) {
        self.work_tx.take();
        self.join_handle.take().unwrap().join().unwrap();
    }
}

struct StatsManagerDatabaseThread {
    conn: SqliteConnection,
    work_rx: Receiver<Box<dyn DatabaseThreadWork>>,
}

impl StatsManagerDatabaseThread {
    fn start(self) -> JoinHandle<()> {
        ThreadBuilder::new()
            .name("StatsManagerDatabaseThread".to_owned())
            .spawn(move || {
                let mut thread = self;
                thread.thread_main()
            })
            .unwrap()
    }

    fn thread_main(&mut self) {
        loop {
            if let Some(mut work) = self.work_rx.blocking_recv() {
                work.perform(self);
            } else {
                // No more work to perform, the thread is requested to terminate.
                return;
            }
        }
    }

    fn init_tables(&mut self) -> Result<(), Error> {
        let token_usage_create_sql = "CREATE TABLE IF NOT EXISTS token_usage (user_id TEXT NOT NULL, time INTEGER NOT NULL, tokens INTEGER NOT NULL, PRIMARY KEY (user_id, time));";
        self.conn.execute(token_usage_create_sql, ())?;
        Ok(())
    }

    fn add_usage(&mut self, user_id: &str, time: i64, tokens: i64) -> Result<bool, Error> {
        let sql = "INSERT OR REPLACE INTO token_usage VALUES (?, ?, COALESCE((SELECT tokens FROM token_usage WHERE user_id = ? AND time = ?), 0) + ?);";
        let mut stmt = self.conn.prepare(sql)?;
        let updated_rows = stmt.execute((user_id, time, user_id, time, tokens))?;
        Ok(updated_rows > 0)
    }

    fn query_usage_of_user(&mut self, user_id: &str) -> Result<i64, Error> {
        let sql = "SELECT SUM(tokens) FROM token_usage WHERE user_id = ?";
        let result = self
            .conn
            .query_row(sql, (user_id,), |row| row.get(0))
            .optional()?;
        Ok(result.unwrap_or(0))
    }

    fn query_total_usage(&mut self) -> Result<i64, Error> {
        let sql = "SELECT SUM(tokens) FROM token_usage";
        let result = self.conn.query_row(sql, (), |row| row.get(0)).optional()?;
        Ok(result.unwrap_or(0))
    }
}

trait DatabaseThreadWork: Send {
    fn perform(&mut self, thread: &mut StatsManagerDatabaseThread);
}

struct AnyDatabaseThreadWork<F>
where
    F: FnOnce(&mut StatsManagerDatabaseThread) + Send,
{
    f: Option<F>,
}

impl<F> AnyDatabaseThreadWork<F>
where
    F: FnOnce(&mut StatsManagerDatabaseThread) + Send,
{
    fn new_boxed(f: F) -> Box<Self> {
        Box::new(Self { f: Some(f) })
    }
}

impl<F> DatabaseThreadWork for AnyDatabaseThreadWork<F>
where
    F: FnOnce(&mut StatsManagerDatabaseThread) + Send,
{
    fn perform(&mut self, thread: &mut StatsManagerDatabaseThread) {
        let f = self.f.take().unwrap();
        f.call_once((thread,))
    }
}
