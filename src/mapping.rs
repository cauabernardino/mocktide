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

#[derive(Debug)]
pub(crate) struct MappingGuard {
    mapping: Mapping,
}

#[derive(Debug, Clone)]
pub(crate) struct Mapping {
    pub state: Arc<MappingState>,
}

#[derive(Debug)]
pub(crate) struct MappingState {
    pub name_to_message: HashMap<String, Bytes>,
    // TODO: Check necessity of having this
    pub message_to_name: HashMap<Bytes, String>,
    pub message_actions: VecDeque<MessageAction>,
}

#[derive(Debug)]
struct MappingFile {
    messages: HashMap<String, Bytes>,
    actions: VecDeque<MessageAction>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct MessageAction {
    pub message: String,
    pub action: Action,
    //TODO: Transform into sleep
    // #[serde(default)]
    // pub wait_for: String,
}

#[derive(Deserialize, Debug)]
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
        let state = Arc::new(MappingState::from_file(config).unwrap());

        Mapping { state }
    }
}

impl MappingState {
    pub fn from_file(config_path: &Path) -> crate::Result<MappingState> {
        let file_content = fs::read_to_string(config_path).unwrap();

        let parsed: MappingFile = serde_yaml::from_str(file_content.as_str())
            .with_context(|| "error parsing the mapping file")?;

        let mut name_to_message: HashMap<String, Bytes> = HashMap::new();
        let mut message_to_name: HashMap<Bytes, String> = HashMap::new();

        for (msg_name, msg_value) in &parsed.messages {
            debug!("{:?}", msg_value);

            name_to_message.insert(msg_name.clone(), msg_value.clone());
            message_to_name.insert(msg_value.clone(), msg_name.clone());
        }

        debug!("parsed file: {:?}", parsed);
        Ok(MappingState {
            name_to_message,
            message_to_name,
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
