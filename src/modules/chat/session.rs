use std::collections::VecDeque;

use async_openai::types::{ChatCompletionRequestMessage as Message, Role};

#[derive(Debug)]
pub struct Session {
    system_message: Option<Message>,
    messages: VecDeque<Message>,
    pending_message: Option<Message>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            system_message: None,
            messages: VecDeque::with_capacity(6),
            pending_message: None,
        }
    }

    pub fn reset(&mut self) {
        self.system_message = None;
        self.messages.clear();
        self.pending_message = None;
    }

    pub fn add_message(&mut self, msg: Message) {
        if matches!(msg.role, Role::System) {
            // Replace the previous system message, we only support
            // one system message at the same time.
            self.system_message = Some(msg);
            return;
        }

        if self.messages.len() >= 30 {
            self.messages.pop_front();
        }
        self.messages.push_back(msg);
    }

    pub fn get_history_messages(&self) -> Vec<Message> {
        let msg_iter = self.messages.iter().cloned();
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
