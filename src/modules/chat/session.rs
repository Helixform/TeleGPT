use std::collections::{HashMap, VecDeque};

use async_openai::types::{ChatCompletionRequestMessage as Message, Role};

use crate::config::SharedConfig;

#[derive(Debug, Clone)]
pub struct HistoryMessage {
    pub id: i64,
    pub message: Message,
}

#[derive(Debug, Default)]
struct HistoryMessagePool {
    current_id: i64,
    messages: HashMap<i64, HistoryMessage>,
    deque: VecDeque<i64>,
}

impl HistoryMessagePool {
    fn prepare_message(&mut self, message: Message) -> HistoryMessage {
        let (id, _) = self.current_id.overflowing_add(1);
        self.current_id = id;

        HistoryMessage { id, message }
    }

    fn push_message(&mut self, message: HistoryMessage) {
        let id = message.id;
        self.messages.insert(id, message);
        self.deque.push_back(id);
    }

    fn pop_message(&mut self) {
        if let Some(evicted_id) = self.deque.pop_front() {
            self.messages.remove(&evicted_id);
        }
    }

    fn clear(&mut self) {
        self.deque.clear();
        self.messages.clear();
    }

    fn len(&self) -> usize {
        self.deque.len()
    }

    fn get_message(&self, id: &i64) -> Option<&HistoryMessage> {
        self.messages.get(id)
    }

    fn iter(&self) -> impl Iterator<Item = &HistoryMessage> + '_ {
        self.deque.iter().filter_map(|id| self.messages.get(id))
    }
}

#[derive(Debug)]
pub struct Session {
    system_message: Option<Message>,
    history_messages: HistoryMessagePool,
    pending_message: Option<Message>,
    config: SharedConfig,
}

impl Session {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            system_message: None,
            history_messages: Default::default(),
            pending_message: None,
            config,
        }
    }

    pub fn reset(&mut self) {
        self.system_message = None;
        self.history_messages.clear();
        self.pending_message = None;
    }

    pub fn prepare_history_message(&mut self, message: Message) -> HistoryMessage {
        self.history_messages.prepare_message(message)
    }

    pub fn add_history_message(&mut self, message: HistoryMessage) {
        if matches!(message.message.role, Role::System) {
            // Replace the previous system message, we only support
            // one system message at the same time.
            self.system_message = Some(message.message);
            return;
        }

        if self.history_messages.len() >= (self.config.conversation_limit as usize) {
            self.history_messages.pop_message();
        }
        self.history_messages.push_message(message);
    }

    pub fn get_history_message(&self, id: i64) -> Option<Message> {
        self.history_messages
            .get_message(&id)
            .map(|m| m.message.clone())
    }

    pub fn get_history_messages(&self) -> Vec<Message> {
        let msg_iter = self.history_messages.iter().map(|m| m.message.clone());
        if let Some(sys_msg) = &self.system_message {
            let prepend = [sys_msg.to_owned()];
            prepend.into_iter().chain(msg_iter).collect()
        } else {
            msg_iter.collect()
        }
    }

    pub fn swap_pending_message(&mut self, msg: Option<Message>) -> Option<Message> {
        if let Some(msg) = msg {
            self.pending_message.replace(msg)
        } else {
            self.pending_message.take()
        }
    }
}
