use std::collections::{hash_map::Entry as HashMapEntry, HashMap};
use std::sync::{Arc, Mutex, Weak};

use teloxide::dptree::di::DependencySupplier;
use teloxide::dptree::from_fn_with_description;
use teloxide::dptree::HandlerDescription;
use teloxide::prelude::*;

use crate::types::TeloxideHandler;

/// A type to store the state associated with a conversation.
pub(crate) struct Conversation<S> {
    chat_id: ChatId,
    user_id: Option<UserId>,
    owner: WeakConversationManager,
    state: Arc<Mutex<S>>,
}

impl<S> Conversation<S>
where
    S: 'static,
{
    fn new(
        chat_id: ChatId,
        user_id: Option<UserId>,
        owner: WeakConversationManager,
        state: S,
    ) -> Self {
        Self {
            chat_id,
            user_id,
            owner,
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub fn update_state<F, R>(&self, updater: F) -> R
    where
        F: FnOnce(&mut S) -> R,
    {
        let mut state = self.state.lock().unwrap();
        updater(&mut *state)
    }

    pub fn end(&self) {
        if let Some(owner) = self.owner.upgrade() {
            owner.end_conversation(self.chat_id, self.user_id)
        }
    }
}

impl<S> Conversation<S>
where
    S: Clone,
{
    pub fn get_state(&self) -> S {
        self.state.lock().unwrap().clone()
    }
}

impl<S> Clone for Conversation<S> {
    fn clone(&self) -> Self {
        Self {
            chat_id: self.chat_id,
            user_id: self.user_id,
            owner: self.owner.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

/// An object to manage conversations and their state across the chats.
#[derive(Clone)]
pub(crate) struct ConversationManager {
    inner: Arc<Mutex<ConversationManagerInner>>,
}

#[derive(Default)]
struct ConversationManagerInner {
    chats: HashMap<ChatId, Chat>,
}

#[derive(Default)]
struct Chat {
    is_global_state: bool,
    user_conversations: HashMap<u64, TeloxideHandler>,
}

impl ConversationManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Default::default())),
        }
    }

    pub fn start_conversation<S>(
        &self,
        chat_id: ChatId,
        user_id: Option<UserId>,
        state: S,
        handler: TeloxideHandler,
    ) where
        S: Send + Sync + 'static,
    {
        let conversation = Conversation::new(chat_id, user_id, self.weak_self(), state);
        let state_injector = from_fn_with_description(
            HandlerDescription::user_defined(),
            move |container: DependencyMap, cont| {
                let conversation = conversation.clone();
                async move {
                    // Temporarily insert the conversation object for its handler.
                    let mut intermediate = container.clone();
                    intermediate.insert(conversation);
                    match cont(intermediate).await {
                        ControlFlow::Continue(_) => ControlFlow::Continue(container),
                        ControlFlow::Break(result) => ControlFlow::Break(result),
                    }
                }
            },
        );
        let conversation_handler = state_injector.chain(handler);

        self.with_mut_inner(|inner| {
            let chat = match inner.chats.entry(chat_id) {
                HashMapEntry::Occupied(occupied) => occupied.into_mut(),
                HashMapEntry::Vacant(vacant) => vacant.insert(Default::default()),
            };
            if let Some(user_id) = user_id {
                chat.user_conversations
                    .insert(user_id.0, conversation_handler);
            } else {
                chat.user_conversations.insert(0, conversation_handler);
                chat.is_global_state = true;
            }
        });
    }

    pub fn end_conversation(&self, chat_id: ChatId, user_id: Option<UserId>) {
        self.with_mut_inner(|inner| {
            if let HashMapEntry::Occupied(mut chat_entry) = inner.chats.entry(chat_id) {
                if let Some(user_id) = user_id {
                    let chat = chat_entry.get_mut();
                    chat.user_conversations.remove(&user_id.0);
                    if !chat.user_conversations.is_empty() {
                        return;
                    }
                }
                chat_entry.remove();
            }
        });
    }

    pub fn make_handler(&self) -> TeloxideHandler {
        let self_cloned = self.clone();
        let next = from_fn_with_description(
            HandlerDescription::user_defined(),
            move |container: DependencyMap, cont| {
                let self_cloned = self_cloned.clone();
                async move {
                    let message: Arc<Message> = container.get();
                    let handler = self_cloned.with_mut_inner(|inner| {
                        inner
                            .chats
                            .get(&message.chat.id)
                            .and_then(|chat| {
                                if chat.is_global_state {
                                    chat.user_conversations.get(&0)
                                } else {
                                    message
                                        .from()
                                        .and_then(|user| chat.user_conversations.get(&user.id.0))
                                }
                            })
                            .map(|handler| handler.clone())
                    });

                    if let Some(handler) = handler {
                        return handler.execute(container, cont).await;
                    }

                    match cont(container.clone()).await {
                        ControlFlow::Continue(_) => ControlFlow::Continue(container),
                        ControlFlow::Break(result) => ControlFlow::Break(result),
                    }
                }
            },
        );
        Update::filter_message().chain(next)
    }

    fn with_mut_inner<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut ConversationManagerInner) -> R,
    {
        let mut inner_mut = self.inner.lock().unwrap();
        f(&mut inner_mut)
    }

    fn weak_self(&self) -> WeakConversationManager {
        WeakConversationManager {
            ptr: Arc::downgrade(&self.inner),
        }
    }
}

struct WeakConversationManager {
    ptr: Weak<Mutex<ConversationManagerInner>>,
}

impl WeakConversationManager {
    fn upgrade(&self) -> Option<ConversationManager> {
        self.ptr
            .upgrade()
            .map(|inner| ConversationManager { inner })
    }
}

impl Clone for WeakConversationManager {
    fn clone(&self) -> Self {
        Self {
            ptr: Weak::clone(&self.ptr),
        }
    }
}
