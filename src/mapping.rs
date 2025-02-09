use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
    sync::Mutex,
};

use bytes::Bytes;
use tokio::sync::Notify;

#[derive(Debug)]
pub(crate) struct MappingDropGuard {
    mapping: Mapping,
}

#[derive(Debug, Clone)]
pub(crate) struct Mapping {
    shared: Arc<SharedState>,
}

#[derive(Debug)]
struct SharedState {
    state: Mutex<State>,
    background_task: Notify,
}

#[derive(Debug)]
struct State {
    entries: HashMap<String, Message>,
    shutdown: bool,
    messages_order: BTreeSet<(usize, String)>,
    last_message: usize,
}

#[derive(Debug)]
struct Message {
    data: Bytes,
    order: usize,
}

impl MappingDropGuard {
    pub(crate) fn new() -> MappingDropGuard {
        MappingDropGuard {
            mapping: Mapping::new(),
        }
    }

    /// Gets underlying mapping, increasing its reference count.
    pub(crate) fn mapping(&self) -> Mapping {
        self.mapping.clone()
    }
}

impl Drop for MappingDropGuard {
    fn drop(&mut self) {
        self.mapping.shutdown();
    }
}

impl Mapping {
    pub(crate) fn new() -> Mapping {
        let shared = Arc::new(SharedState {
            state: Mutex::new(State {
                entries: HashMap::new(),
                messages_order: BTreeSet::new(),
                shutdown: false,
                last_message: 0,
            }),
            background_task: Notify::new(),
        });

        Mapping { shared }
    }

    fn shutdown(&self) {
        let mut state = self.shared.state.lock().unwrap();
        state.shutdown = true;

        drop(state);
        self.shared.background_task.notify_one();
    }

    pub(crate) fn get(&self, key: &str) -> Option<Bytes> {
        let state = self.shared.state.lock().unwrap();
        state.entries.get(key).map(|entry| entry.data.clone())
    }

    pub(crate) fn set(&self, key: String, value: Bytes) {
        let mut state = self.shared.state.lock().unwrap();

        let last_msg = state.last_message;

        state.entries.insert(
            key.clone(),
            Message {
                data: value,
                order: last_msg,
            },
        );

        state.last_message += 1;
    }

    fn shutdown_task(&self) {
        let mut state = self.shared.state.lock().unwrap();
        state.shutdown = true;

        drop(state);
        self.shared.background_task.notify_one();
    }
}

impl SharedState {
    /// Remove streamed messages and returns the order for the next message.
    // fn remove_streamed_messages(&self) -> Option<usize> {
    //     let mut state = self.state.lock().unwrap();

    //     if state.shutdown {
    //         return None;
    //     }

    //  }

    fn is_shutdown(&self) -> bool {
        self.state.lock().unwrap().shutdown
    }
}

impl State {
    fn next_message(&self) -> Option<usize> {
        self.messages_order.iter().next().map(|order| order.0)
    }
}
