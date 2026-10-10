use serde::Deserialize;
use std::path::PathBuf;

/// A session on disk
pub struct SessionFile {
    pub path: PathBuf,
    pub id: String,
    pub dir: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
}

/// One line from the JSONL stream
#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum Entry {
    #[serde(rename = "session")]
    Session {
        id: String,
        timestamp: String,
        cwd: Option<String>,
    },
    #[serde(rename = "model_change")]
    ModelChange {
        #[serde(rename = "modelId")]
        model_id: String,
        #[serde(default)]
        provider: String,
        #[allow(dead_code)]
        timestamp: String,
    },
    #[serde(rename = "thinking_level_change")]
    ThinkingLevelChange {
        #[allow(dead_code)]
        #[serde(rename = "thinkingLevel")]
        level: String,
        #[allow(dead_code)]
        timestamp: String,
    },
    #[serde(rename = "message")]
    Message {
        id: String,
        timestamp: String,
        message: MessageBody,
    },
    #[serde(rename = "custom")]
    Custom {
        #[serde(rename = "customType")]
        custom_type: String,
        #[serde(default)]
        data: Option<serde_json::Value>,
        #[allow(dead_code)]
        timestamp: String,
    },
    #[serde(rename = "compaction")]
    Compaction {
        id: String,
        summary: String,
        timestamp: String,
    },
}

#[derive(Deserialize)]
pub struct MessageBody {
    pub role: String,
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    #[serde(rename = "toolCallId")]
    pub tool_call_id: Option<String>,
    #[serde(rename = "toolName")]
    pub tool_name: Option<String>,
    pub timestamp: Option<i64>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking {
        #[allow(dead_code)]
        thinking: String,
    },
    #[serde(rename = "toolCall")]
    ToolCall {
        id: String,
        name: String,
        #[serde(default)]
        arguments: Option<serde_json::Value>,
    },
    #[serde(other)]
    Other,
}

/// Extracted info for output
#[derive(Clone)]
pub struct MessageInfo {
    pub session_id: String,
    pub entry_id: String,
    pub timestamp: String,
    pub role: String,
    pub text: String,
    pub tool_name: Option<String>,
    pub tool_args: Option<serde_json::Value>,
}
