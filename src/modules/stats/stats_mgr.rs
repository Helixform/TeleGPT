use std::thread::{Builder as ThreadBuilder, JoinHandle};

use anyhow::Error;
use rusqlite::Connection as SqliteConnection;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::oneshot::channel as oneshot_channel;

use super::DatabaseProvider;

pub struct StatsManager {
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
            join_handle: Some(join_handle),
            work_tx: Some(work_tx),
        })
    }
}

impl Drop for StatsManager {
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
