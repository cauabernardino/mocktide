use anyhow::Context;
use bytes::Bytes;
use log::debug;
use serde::{de, Deserialize, Deserializer};
use std::{collections::HashMap, fs};

/// A parsed mapping file: the messages and the actions every connection runs through.
#[derive(Debug)]
pub(crate) struct Mapping {
    pub name: String,
    pub messages: HashMap<String, Bytes>,
    pub actions: Vec<MessageAction>,
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

    /// How long a `Recv` waits for its message, in seconds.
    #[serde(default = "default_recv_timeout")]
    pub timeout: u64,
}

fn default_recv_timeout() -> u64 {
    30
}

/// Defines actions the server can perform
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    /// Send a mapped message
    Send,
    /// Receive a mapped message
    Recv,
    /// Shutdown the server, closing all connections
    Shutdown,
}

impl Mapping {
    pub fn from_file(path: &str) -> crate::Result<Mapping> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("could not read mapping file '{path}'"))?;

        let mapping: Mapping = serde_yaml_ng::from_str(&content)
            .with_context(|| format!("error parsing mapping file '{path}'"))?;

        debug!("parsed mapping: {:#?}", mapping);
        Ok(mapping)
    }
}

impl<'de> Deserialize<'de> for Mapping {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            name: String,
            messages: HashMap<String, String>,
            actions: Vec<MessageAction>,
        }

        let helper = Helper::deserialize(deserializer)?;

        for action in &helper.actions {
            if action.execute == Action::Shutdown {
                continue;
            }
            if action.message.is_empty() {
                return Err(de::Error::custom(format!(
                    "Action {:?} requires a mapped message",
                    action.execute
                )));
            }
            if !helper.messages.contains_key(&action.message) {
                return Err(de::Error::custom(format!(
                    "Action {:?} references unknown message '{}'",
                    action.execute, action.message
                )));
            }
        }

        let mut messages = HashMap::new();
        for (name, value) in helper.messages {
            let bytes = to_bytes(&value).map_err(|c| {
                de::Error::custom(format!(
                    "message '{name}' contains '{c}' (U+{:04X}), which does not fit in a byte; \
                     write binary data as \\xNN escapes",
                    c as u32
                ))
            })?;
            messages.insert(name, bytes);
        }
        // To not complicate even more the flow of ConnHandler,
        // a null byte mapping for shutdown action
        messages.insert("".to_string(), Bytes::from("\x00"));

        Ok(Mapping {
            name: helper.name,
            messages,
            actions: helper.actions,
        })
    }
}

/// Maps each character to the byte with the same value, so a YAML `"\x80"` is the
/// byte 0x80 rather than its UTF-8 encoding. Returns the first character above U+00FF.
fn to_bytes(value: &str) -> Result<Bytes, char> {
    value
        .chars()
        .map(|c| u8::try_from(c).map_err(|_| c))
        .collect::<Result<Vec<u8>, char>>()
        .map(Bytes::from)
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
    fn test_escapes_above_ascii_map_to_single_bytes() {
        let parsed: Mapping = serde_yaml_ng::from_str(
            r#"
            name: bytes
            messages:
                binary: "\x00\x7F\x80\xFF"
                text: "+hello"
            actions:
                - message: binary
                  execute: Send
            "#,
        )
        .unwrap();

        assert_eq!(parsed.messages["binary"][..], [0x00, 0x7F, 0x80, 0xFF]);
        assert_eq!(parsed.messages["text"][..], *b"+hello");
    }

    #[test]
    fn test_characters_that_do_not_fit_a_byte_are_rejected() {
        let parsed: Result<Mapping, serde_yaml_ng::Error> = serde_yaml_ng::from_str(
            r#"
            name: euro
            messages:
                price: "10\u20AC"
            actions:
                - message: price
                  execute: Send
            "#,
        );

        let err = parsed.unwrap_err().to_string();
        assert!(err.contains("price"), "{err}");
    }

    #[test]
    fn test_yaml_file_is_correctly_deserialized() {
        let parsed: Result<Mapping, serde_yaml_ng::Error> = serde_yaml_ng::from_str(MAPPING_YAML);

        assert_ok!(parsed);
    }

    static MAPPING_UNKNOWN_MSG_YAML: &str = r#"
        name: unknown message

        messages:
            msg1: "\x01\x02\x03"

        actions:
            - message: msg2
              execute: Recv
    "#;

    #[test]
    fn test_yaml_file_fails_to_deserialize_action_with_unknown_msg() {
        let parsed: Result<Mapping, serde_yaml_ng::Error> =
            serde_yaml_ng::from_str(MAPPING_UNKNOWN_MSG_YAML);

        assert_err!(parsed);
    }

    #[test]
    fn test_yaml_file_fails_to_serialize_action_without_a_mapped_msg() {
        let parsed: Result<Mapping, serde_yaml_ng::Error> =
            serde_yaml_ng::from_str(MAPPING_WRONG_YAML);

        assert_err!(parsed);
    }
}
