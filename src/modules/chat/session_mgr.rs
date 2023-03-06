use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_openai::types::ChatCompletionRequestMessage as Message;

use super::Session;
use crate::config::SharedConfig;

pub struct SessionManager {
    inner: Arc<Mutex<SessionManagerInner>>,
}

struct SessionManagerInner {
    sessions: HashMap<String, Session>,
    config: SharedConfig,
}

impl SessionManager {
    pub fn new(config: SharedConfig) -> Self {
        let inner = SessionManagerInner {
            sessions: HashMap::new(),
            config,
        };

        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn reset_session(&self, key: String) {
        self.with_mut_session(key, |session| session.reset());
    }

    pub fn add_message_to_session(&self, key: String, msg: Message) {
        self.with_mut_session(key, |session| session.add_message(msg));
    }

    pub fn get_history_messages(&self, key: &str) -> Vec<Message> {
        self.with_mut_inner(|inner| {
            inner
                .sessions
                .get(key)
                .map(|s| s.get_history_messages())
                .unwrap_or(vec![])
        })
    }

    pub fn swap_session_pending_message(
        &self,
        key: String,
        msg: Option<Message>,
    ) -> Option<Message> {
        self.with_mut_session(key, |session| session.swap_pending_message(msg))
    }

    fn with_mut_inner<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut SessionManagerInner) -> R,
    {
        let mut inner_mut = self.inner.lock().unwrap();
        f(&mut inner_mut)
    }

    fn with_mut_session<F, R>(&self, key: String, f: F) -> R
    where
        F: FnOnce(&mut Session) -> R,
    {
        self.with_mut_inner(|inner| {
            let session_mut = inner
                .sessions
                .entry(key)
                .or_insert(Session::new(inner.config.clone()));
            f(session_mut)
        })
    }
}

impl Clone for SessionManager {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
