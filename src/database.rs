use std::fmt::Debug;
use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::thread::{Builder as ThreadBuilder, JoinHandle};

use anyhow::Error;
use rusqlite::Connection;
use tokio::sync::mpsc::{channel, Receiver, Sender};

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

pub(crate) struct DatabaseManager {
    inner: Arc<DatabaseManagerInner>,
}

impl DatabaseManager {
    pub fn with_db_provider<P>(provider: P) -> Result<Self, Error>
    where
        P: DatabaseProvider,
    {
        let conn = provider.provide_db()?;
        let (cmd_tx, cmd_rx) = channel(10);

        let db_thread = DatabaseThread::new(conn, cmd_rx);
        let join_handle = ManuallyDrop::new(db_thread.start());

        Ok(Self {
            inner: Arc::new(DatabaseManagerInner {
                join_handle,
                cmd_tx,
            }),
        })
    }

    pub async fn enqueue_work<F>(&self, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut Connection) + Send + 'static,
    {
        let work = AnyDatabaseThreadWork::new_boxed(f);
        self.inner
            .cmd_tx
            .send(DatabaseThreadCommand::Work(work))
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
    cmd_tx: Sender<DatabaseThreadCommand>,
}

impl Drop for DatabaseManagerInner {
    fn drop(&mut self) {
        self.cmd_tx
            .blocking_send(DatabaseThreadCommand::Shutdown)
            .unwrap();
        let join_handle = unsafe { ManuallyDrop::take(&mut self.join_handle) };
        join_handle.join().unwrap();
    }
}

struct DatabaseThread {
    conn: Connection,
    cmd_rx: Receiver<DatabaseThreadCommand>,
    shutdown: bool,
}

impl DatabaseThread {
    fn new(conn: Connection, cmd_rx: Receiver<DatabaseThreadCommand>) -> Self {
        Self {
            conn,
            cmd_rx,
            shutdown: false,
        }
    }

    fn start(self) -> JoinHandle<()> {
        ThreadBuilder::new()
            .name("DatabaseThread".to_owned())
            .spawn(move || {
                let mut thread = self;
                thread.thread_main()
            })
            .unwrap()
    }

    fn thread_main(&mut self) {
        loop {
            if self.shutdown {
                return;
            }

            if let Some(cmd) = self.cmd_rx.blocking_recv() {
                self.handle_cmd(cmd);
            } else {
                // No more work to perform, the thread is requested to terminate.
                return;
            }
        }
    }

    fn handle_cmd(&mut self, cmd: DatabaseThreadCommand) {
        match cmd {
            DatabaseThreadCommand::Work(mut work) => work.perform(&mut self.conn),
            DatabaseThreadCommand::Shutdown => {
                info!("Database thread is shutting down...");
                self.shutdown = true
            }
        }
    }
}

enum DatabaseThreadCommand {
    Work(Box<dyn DatabaseThreadWork>),
    Shutdown,
}

impl Debug for DatabaseThreadCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Work(_) => write!(f, "Work"),
            Self::Shutdown => write!(f, "Shutdown"),
        }
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
    fn new_boxed(f: F) -> Box<Self> {
        Box::new(Self { f: Some(f) })
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
