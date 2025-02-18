use anyhow::Context;
use bytes::Bytes;
use log::debug;
use serde::{de, Deserialize, Deserializer};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    sync::Arc,
};

use tokio::sync::RwLock;

#[derive(Debug)]
pub(crate) struct MappingGuard {
    mapping: Mapping,
}

#[derive(Debug, Clone)]
pub(crate) struct Mapping {
    pub state: Arc<RwLock<MappingState>>,
}

#[derive(Debug)]
pub(crate) struct MappingState {
    pub mapping_name: String,
    pub name_to_message: HashMap<String, Bytes>,
    pub message_actions: VecDeque<MessageAction>,
}

#[derive(Debug)]
pub struct MappingFile {
    name: String,
    messages: HashMap<String, Bytes>,
    actions: VecDeque<MessageAction>,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct MessageAction {
    /// Unique message name from mapping
    #[serde(default)]
    pub message: String,

    /// Action to be executed
    pub execute: Action,

    /// Optional waiting time, in seconds. Defaults to zero.
    #[serde(default)]
    pub wait_for: u64,
}

/// Defines actions the server can perform
#[derive(Deserialize, Debug, Clone)]
pub(crate) enum Action {
    /// Send a mapped message
    Send,
    /// Receive a mapped message
    Recv,
    /// Shutdown the server, closing all connections
    Shutdown,
    /// Placeholder
    Unknown,
}

impl MappingGuard {
    pub(crate) fn new(config: String) -> MappingGuard {
        MappingGuard {
            mapping: Mapping::new(config),
        }
    }

    /// Gets underlying mapping, increasing its reference count.
    pub(crate) fn mapping(&self) -> Mapping {
        self.mapping.clone()
    }
}

impl Mapping {
    pub(crate) fn new(config: String) -> Mapping {
        let state = Arc::new(RwLock::new(MappingState::from_file(config).unwrap()));

        Mapping { state }
    }
}

impl MappingState {
    pub fn from_file(config_path: String) -> crate::Result<MappingState> {
        let file_content = fs::read_to_string(config_path).unwrap();

        let parsed: MappingFile = serde_yaml::from_str(file_content.as_str())
            .with_context(|| "error parsing the mapping file")?;

        let mut name_to_message: HashMap<String, Bytes> = HashMap::new();

        for (msg_name, msg_value) in &parsed.messages {
            name_to_message.insert(msg_name.clone(), msg_value.clone());
            debug!("mapped msg: {:#?}", msg_value);
        }

        debug!("parsed file: {:?}", parsed);
        Ok(MappingState {
            mapping_name: parsed.name,
            name_to_message,
            message_actions: parsed.actions,
        })
    }
}

impl<'de> Deserialize<'de> for MappingFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            name: String,
            messages: HashMap<String, String>,
            actions: VecDeque<MessageAction>,
        }

        let helper = Helper::deserialize(deserializer)?;

        for action in &helper.actions {
            if action.message.is_empty() && action.execute != Action::Shutdown {
                return Err(de::Error::custom(format!(
                    "Action {:?} requires a mapped message",
                    action.execute
                )));
            }
        }

        let mut messages = HashMap::new();
        for (k, v) in helper.messages {
            messages.insert(k, Bytes::from(v));
        }
        // To not complicate even more the flow of ConnHandler,
        // a null byte mapping for shutdown action
        messages.insert("".to_string(), Bytes::from("\x00"));

        Ok(MappingFile {
            name: helper.name,
            messages,
            actions: helper.actions,
        })
    }
}

impl PartialEq for Action {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Action::Send, Action::Send)
                | (Action::Recv, Action::Recv)
                | (Action::Shutdown, Action::Shutdown)
                | (Action::Unknown, Action::Unknown),
        )
    }
}

impl PartialEq for MessageAction {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message && self.execute == other.execute
    }
}

#[cfg(test)]
mod tests {
    use claims::{assert_err, assert_ok};

    use super::*;

    static MAPPING_YAML: &str = r#"
        name: good test

        messages:
            msg1: "\x01\x02\x03"
            msg2: "\x04\x05\x06"

        actions:
            - message: msg1
              execute: Send
            - message: msg2
              execute: Recv
    "#;

    static MAPPING_WRONG_YAML: &str = r#"
        name: bad test

        messages:
            msg1: "\x01\x02\x03"
            msg2: "\x04\x05\x06"

        actions:
            - message: msg1
              execute: Send
            - execute: Recv
    "#;

    #[test]
    fn test_yaml_file_is_correctly_deserialized() {
        let parsed: Result<MappingFile, serde_yaml::Error> = serde_yaml::from_str(MAPPING_YAML);

        assert_ok!(parsed);
    }

    #[test]
    fn test_yaml_file_fails_to_serialize_action_without_a_mapped_msg() {
        let parsed: Result<MappingFile, serde_yaml::Error> =
            serde_yaml::from_str(MAPPING_WRONG_YAML);

        assert_err!(parsed);
    }
}
