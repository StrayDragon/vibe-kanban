use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(use_ts_enum)]
pub enum BaseCodingAgent {
    ClaudeCode,
    Amp,
    Gemini,
    Codex,
    FakeAgent,
    Opencode,
    CursorAgent,
    QwenCode,
    Copilot,
    Droid,
}

impl BaseCodingAgent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "CLAUDE_CODE",
            Self::Amp => "AMP",
            Self::Gemini => "GEMINI",
            Self::Codex => "CODEX",
            Self::FakeAgent => "FAKE_AGENT",
            Self::Opencode => "OPENCODE",
            Self::CursorAgent => "CURSOR_AGENT",
            Self::QwenCode => "QWEN_CODE",
            Self::Copilot => "COPILOT",
            Self::Droid => "DROID",
        }
    }
}

impl fmt::Display for BaseCodingAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct BaseCodingAgentParseError;

impl fmt::Display for BaseCodingAgentParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown base coding agent")
    }
}

impl std::error::Error for BaseCodingAgentParseError {}

impl FromStr for BaseCodingAgent {
    type Err = BaseCodingAgentParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "CLAUDE_CODE" => Ok(Self::ClaudeCode),
            "AMP" => Ok(Self::Amp),
            "GEMINI" => Ok(Self::Gemini),
            "CODEX" => Ok(Self::Codex),
            "FAKE_AGENT" => Ok(Self::FakeAgent),
            "OPENCODE" => Ok(Self::Opencode),
            "CURSOR_AGENT" => Ok(Self::CursorAgent),
            "QWEN_CODE" => Ok(Self::QwenCode),
            "COPILOT" => Ok(Self::Copilot),
            "DROID" => Ok(Self::Droid),
            _ => Err(BaseCodingAgentParseError),
        }
    }
}
