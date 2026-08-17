//! Stable, framework-neutral data types for the first Cakify benchmark round.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: &str = "cakify.bench.v1";
pub const FIXTURE_ID: &str = "chat-10k-v1";
pub const FIXTURE_HASH: &str = "chat-10k-v1:deterministic-20260816";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FixtureManifest {
    pub protocol_version: String,
    pub fixture_id: String,
    pub fixture_hash: String,
    pub message_count: u32,
    pub page_size: u32,
    pub stream_event_count: u32,
    pub stream_interval_ms: u64,
    pub tool_event_count: u32,
    pub image_asset: String,
    pub visual_spec_version: String,
}

impl Default for FixtureManifest {
    fn default() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION.to_owned(),
            fixture_id: FIXTURE_ID.to_owned(),
            fixture_hash: FIXTURE_HASH.to_owned(),
            message_count: 10_000,
            page_size: 200,
            stream_event_count: 30,
            stream_interval_ms: 1_000,
            tool_event_count: 8,
            image_asset: "bench/assets/attachment-sample.svg".to_owned(),
            visual_spec_version: "visual.v1".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageRecord {
    pub id: String,
    pub index: u32,
    pub role: String,
    pub markdown: String,
    pub has_image: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessagePage {
    pub fixture_hash: String,
    pub offset: u32,
    pub total: u32,
    pub messages: Vec<MessageRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub ok: bool,
    pub protocol_version: String,
    pub fixture_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadyResponse {
    pub port: u16,
    pub protocol_version: String,
    pub fixture_hash: String,
    pub session_token: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunRequest {
    pub run_id: String,
    pub scenario: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancelRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancelResponse {
    pub accepted: bool,
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BenchEvent {
    Ready {
        run_id: String,
        fixture_hash: String,
    },
    Tool {
        run_id: String,
        sequence: u32,
        tool: String,
        stage: String,
        detail: String,
    },
    StreamDelta {
        run_id: String,
        sequence: u32,
        text: String,
        done: bool,
    },
    Completed {
        run_id: String,
        elapsed_ms: u64,
    },
    Cancelled {
        run_id: String,
        reason: String,
    },
}
