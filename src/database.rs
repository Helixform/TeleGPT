use std::fmt::Debug;
use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{Builder as ThreadBuilder, JoinHandle};

use anyhow::Error;
use rusqlite::Connection;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Notify;

pub(crate) trait DatabaseProvider {
    fn provide_db(&self) -> Result<Connection, Error>;
}

pub(crate) struct InMemDatabaseProvider;

impl DatabaseProvider for InMemDatabaseProvider {
    fn provide_db(&self) -> Result<Connection, Error> {
        let conn = Connection::open_in_memory()?;
        Ok(conn)
    }
}

pub(crate) struct FileDatabaseProvider {
    path: PathBuf,
}

impl FileDatabaseProvider {
    pub fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            path: path.as_ref().to_owned(),
        }
    }
}

impl DatabaseProvider for FileDatabaseProvider {
    fn provide_db(&self) -> Result<Connection, Error> {
        let conn = Connection::open(&self.path)?;
        Ok(conn)
    }
}

pub(crate) struct DatabaseManager {
    inner: Arc<DatabaseManagerInner>,
}

impl DatabaseManager {
    pub fn with_db_provider<P>(provider: P) -> Result<Self, Error>
    where
        P: DatabaseProvider,
    {
        let conn = provider.provide_db()?;
        let (work_tx, work_rx) = channel(10);
        let shutdown_notify = Arc::new(Notify::new());

        let rt_handle = Handle::current().clone();

        let db_thread = DatabaseThread::new(conn, rt_handle, work_rx, Arc::clone(&shutdown_notify));
        let join_handle = ManuallyDrop::new(db_thread.start());

        Ok(Self {
            inner: Arc::new(DatabaseManagerInner {
                join_handle,
                work_tx,
                shutdown_notify,
            }),
        })
    }

    pub async fn enqueue_work<F>(&self, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut Connection) + Send + 'static,
    {
        let work = AnyDatabaseThreadWork::new(f);
        self.inner
            .work_tx
            .send(Box::new(work))
            .await
            .map_err(|err| anyhow!(err.to_string()))?;

        Ok(())
    }

    pub async fn query<F, R>(&self, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Connection) -> R + Send + 'static,
        R: Send + Debug + 'static,
    {
        let (res_tx, res_rx) = tokio::sync::oneshot::channel();
        self.enqueue_work(move |conn| {
            let res = f.call_once((conn,));
            res_tx.send(res).unwrap();
        })
        .await?;

        res_rx.await.map_err(|err| anyhow!(err.to_string()))
    }
}

impl Clone for DatabaseManager {
    fn clone(&self) -> Self {
        DatabaseManager {
            inner: Arc::clone(&self.inner),
        }
    }
}

struct DatabaseManagerInner {
    join_handle: ManuallyDrop<JoinHandle<()>>,
    work_tx: Sender<Box<dyn DatabaseThreadWork>>,
    shutdown_notify: Arc<Notify>,
}

impl Drop for DatabaseManagerInner {
    fn drop(&mut self) {
        // Gracefully shutdown the database thread.
        self.shutdown_notify.notify_one();
        let join_handle = unsafe { ManuallyDrop::take(&mut self.join_handle) };
        join_handle.join().unwrap();

        debug!("Database thread has shutdown");
    }
}

struct DatabaseThread {
    conn: Connection,
    rt_handle: Handle,
    work_rx: Receiver<Box<dyn DatabaseThreadWork>>,
    shutdown_notify: Arc<Notify>,
    shutdown: bool,
}

impl DatabaseThread {
    fn new(
        conn: Connection,
        rt_handle: Handle,
        work_rx: Receiver<Box<dyn DatabaseThreadWork>>,
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            conn,
            rt_handle,
            work_rx,
            shutdown_notify,
            shutdown: false,
        }
    }

    fn start(self) -> JoinHandle<()> {
        ThreadBuilder::new()
            .name("DatabaseThread".to_owned())
            .spawn(move || {
                let mut thread = self;
                let handle = thread.rt_handle.clone();
                handle.block_on(async move {
                    thread.run_loop().await;
                });
            })
            .unwrap()
    }

    async fn run_loop(&mut self) {
        while !self.shutdown {
            self.poll_once().await;
        }
    }

    async fn poll_once(&mut self) {
        tokio::select! {
            _ = self.shutdown_notify.notified() => {
                self.shutdown = true;
            },
            maybe_work = self.work_rx.recv() => {
                if let Some(mut work) = maybe_work {
                    work.perform(&mut self.conn);
                } else {
                    // No more work to perform, the thread is requested to terminate.
                    self.shutdown = true;
                }
            }
        };
    }
}

trait DatabaseThreadWork: Send {
    fn perform(&mut self, conn: &mut Connection);
}

struct AnyDatabaseThreadWork<F>
where
    F: FnOnce(&mut Connection) + Send,
{
    f: Option<F>,
}

impl<F> AnyDatabaseThreadWork<F>
where
    F: FnOnce(&mut Connection) + Send,
{
    fn new(f: F) -> Self {
        Self { f: Some(f) }
    }
}

impl<F> Debug for AnyDatabaseThreadWork<F>
where
    F: FnOnce(&mut Connection) + Send,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DatabaseThreadWork")
    }
}

impl<F> DatabaseThreadWork for AnyDatabaseThreadWork<F>
where
    F: FnOnce(&mut Connection) + Send,
{
    fn perform(&mut self, conn: &mut Connection) {
        let f = self.f.take().unwrap();
        f.call_once((conn,))
    }
}
