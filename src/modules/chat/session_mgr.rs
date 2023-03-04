use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_openai::types::ChatCompletionRequestMessage as Message;

use super::Session;

pub struct SessionManager {
    inner: Arc<Mutex<SessionManagerInner>>,
}

struct SessionManagerInner {
    sessions: HashMap<String, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        let inner = SessionManagerInner {
            sessions: HashMap::new(),
        };

        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn reset_session(&self, key: String) {
        self.with_mut_inner(|inner| {
            inner.sessions.insert(key, Session::new());
        });
    }

    pub fn add_message_to_session(&self, key: String, msg: Message) {
        self.with_mut_inner(|inner| {
            let session = inner.sessions.entry(key).or_insert(Session::new());
            session.add_message(msg);
        });
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

    fn with_mut_inner<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut SessionManagerInner) -> R,
    {
        let mut inner_mut = self.inner.lock().unwrap();
        f(&mut inner_mut)
    }
}

impl Clone for SessionManager {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
