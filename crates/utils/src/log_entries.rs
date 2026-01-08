use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LogEntryChannel {
    Raw,
    Normalized,
}

impl LogEntryChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogEntryChannel::Raw => "raw",
            LogEntryChannel::Normalized => "normalized",
        }
    }
}

impl FromStr for LogEntryChannel {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "raw" => Ok(LogEntryChannel::Raw),
            "normalized" => Ok(LogEntryChannel::Normalized),
            _ => Err(format!("Unknown log entry channel: {value}")),
        }
    }
}

impl std::fmt::Display for LogEntryChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
