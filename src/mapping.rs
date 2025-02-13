use anyhow::Context;
// use base64::{engine::general_purpose::STANDARD, Engine};
use bytes::Bytes;
use log::debug;
use serde::{Deserialize, Deserializer};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::Path,
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
    pub name_to_message: HashMap<String, Bytes>,
    pub message_actions: VecDeque<MessageAction>,
}

#[derive(Debug)]
pub struct MappingFile {
    messages: HashMap<String, Bytes>,
    actions: VecDeque<MessageAction>,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct MessageAction {
    pub message: String,
    pub action: Action,
    //TODO: Transform into sleep
    // #[serde(default)]
    // pub wait_for: String,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) enum Action {
    Send,
    Recv,
    Unknown,
}

impl MappingGuard {
    pub(crate) fn new(config: &Path) -> MappingGuard {
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
    pub(crate) fn new(config: &Path) -> Mapping {
        let state = Arc::new(RwLock::new(MappingState::from_file(config).unwrap()));

        Mapping { state }
    }
}

impl MappingState {
    pub fn from_file(config_path: &Path) -> crate::Result<MappingState> {
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
            messages: HashMap<String, String>,
            actions: VecDeque<MessageAction>,
        }

        let helper = Helper::deserialize(deserializer)?;

        let mut messages = HashMap::new();
        for (k, v) in helper.messages {
            // TODO: Access the need of using base64 encoding
            // let bytes = STANDARD.decode(&v).map_err(de::Error::custom)?;
            messages.insert(k, Bytes::from(v));
        }

        Ok(MappingFile {
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
                | (Action::Unknown, Action::Unknown),
        )
    }
}

impl PartialEq for MessageAction {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message && self.action == other.action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static MAPPING_YAML: &str = r#"
        messages:
            msg1: "\x01\x02\x03"
            msg2: "\x04\x05\x06"

        actions:
            - message: msg1
              action: Send
            - message: msg2
              action: Recv
    "#;

    #[test]
    fn yaml_file_is_correctly_deserialized() {
        let test_parsed: MappingFile = serde_yaml::from_str(MAPPING_YAML).unwrap();

        assert_eq!(test_parsed.messages["msg1"], Bytes::from("\x01\x02\x03"));
        assert_eq!(test_parsed.messages["msg2"], Bytes::from("\x04\x05\x06"));
        assert_eq!(
            test_parsed.actions[0],
            MessageAction {
                message: "msg1".to_string(),
                action: Action::Send
            }
        );
        assert_eq!(
            test_parsed.actions[1],
            MessageAction {
                message: "msg2".to_string(),
                action: Action::Recv
            }
        );
    }
}
