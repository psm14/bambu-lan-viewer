use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub enum CommandRequest {
    Pause,
    Resume,
    Stop,
    Light { on: bool },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CommandPayload {
    Pause,
    Resume,
    Stop,
    Light { on: bool },
}

impl From<CommandPayload> for CommandRequest {
    fn from(payload: CommandPayload) -> Self {
        match payload {
            CommandPayload::Pause => CommandRequest::Pause,
            CommandPayload::Resume => CommandRequest::Resume,
            CommandPayload::Stop => CommandRequest::Stop,
            CommandPayload::Light { on } => CommandRequest::Light { on },
        }
    }
}

impl CommandRequest {
    pub fn to_payload(&self, user_id: &str, sequence_id: u64) -> Value {
        let sequence_id = sequence_id.to_string();
        match self {
            CommandRequest::Pause => json!({
                "user_id": user_id,
                "print": {
                    "sequence_id": sequence_id,
                    "command": "pause"
                }
            }),
            CommandRequest::Resume => json!({
                "user_id": user_id,
                "print": {
                    "sequence_id": sequence_id,
                    "command": "resume"
                }
            }),
            CommandRequest::Stop => json!({
                "user_id": user_id,
                "print": {
                    "sequence_id": sequence_id,
                    "command": "stop"
                }
            }),
            CommandRequest::Light { on } => json!({
                "user_id": user_id,
                "system": {
                    "sequence_id": sequence_id,
                    "command": "ledctrl",
                    "led_node": "chamber_light",
                    "led_mode": if *on { "on" } else { "off" },
                    "led_on_time": 500,
                    "led_off_time": 500,
                    "loop_times": 0,
                    "interval_time": 0
                }
            }),
        }
    }
}
