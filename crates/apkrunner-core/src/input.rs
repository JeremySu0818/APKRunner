use serde::{Deserialize, Serialize};

use crate::error::{ApkRunnerError, ApkRunnerResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InputEvent {
    Tap { x: u32, y: u32 },
    Key { key_code: u32 },
    Text { text: String },
}

pub fn escape_adb_input_text(text: &str) -> ApkRunnerResult<String> {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '_' | '@' | '%' | '+' | '-' => {
                escaped.push(character)
            }
            ' ' => escaped.push_str("%s"),
            unsupported => {
                return Err(ApkRunnerError::RuntimeBackendError(format!(
                    "unsupported text input character: {unsupported:?}"
                )));
            }
        }
    }
    Ok(escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adb_input_text_escapes_spaces() {
        assert_eq!(
            escape_adb_input_text("hello world+a@b.com").expect("text should be safe"),
            "hello%sworld+a@b.com"
        );
    }

    #[test]
    fn adb_input_text_rejects_shell_metacharacters() {
        assert!(escape_adb_input_text("hello;rm").is_err());
    }
}
