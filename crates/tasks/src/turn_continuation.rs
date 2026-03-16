use db::types::VkNextAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VkNextParseResult {
    Parsed(VkNextAction),
    Missing,
    Invalid { raw: String },
}

const VK_NEXT_PREFIX: &str = "vk_next:";

pub fn parse_vk_next_action(text: &str) -> Option<VkNextAction> {
    match parse_vk_next_action_detailed(text) {
        VkNextParseResult::Parsed(action) => Some(action),
        _ => None,
    }
}

pub fn parse_vk_next_action_detailed(text: &str) -> VkNextParseResult {
    for line in text.lines() {
        let trimmed = line.trim();
        let trimmed = trimmed
            .strip_prefix('-')
            .or_else(|| trimmed.strip_prefix('*'))
            .map(|rest| rest.trim_start())
            .unwrap_or(trimmed);

        // Fast path: avoid allocating for most lines.
        let Some(head) = trimmed.get(..VK_NEXT_PREFIX.len()) else {
            continue;
        };
        if !head.eq_ignore_ascii_case(VK_NEXT_PREFIX) {
            continue;
        }

        let rest = trimmed[VK_NEXT_PREFIX.len()..].trim();

        let value = rest.split_whitespace().next().unwrap_or_default();
        return match value {
            "continue" => VkNextParseResult::Parsed(VkNextAction::Continue),
            "review" => VkNextParseResult::Parsed(VkNextAction::Review),
            other => VkNextParseResult::Invalid {
                raw: other.to_string(),
            },
        };
    }

    VkNextParseResult::Missing
}

pub fn strip_vk_next_lines(text: &str) -> String {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let trimmed = trimmed
            .strip_prefix('-')
            .or_else(|| trimmed.strip_prefix('*'))
            .map(|rest| rest.trim_start())
            .unwrap_or(trimmed);
        let Some(head) = trimmed.get(..VK_NEXT_PREFIX.len()) else {
            out.push(line);
            continue;
        };
        if !head.eq_ignore_ascii_case(VK_NEXT_PREFIX) {
            out.push(line);
        }
    }
    out.join("\n").trim().to_string()
}

pub fn build_turn_continuation_prompt(
    previous_summary: Option<&str>,
    remaining_turns: i32,
) -> String {
    let remaining_turns = remaining_turns.max(0);
    let summary = previous_summary
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("No summary available from the previous turn.");

    format!(
        "This is an automated continuation turn for the same Vibe Kanban task session.\n\
Remaining continuation turns after this one: {remaining_turns}\n\n\
Previous turn summary:\n{summary}\n\n\
Instructions:\n\
1. Continue from the current workspace state. Do not restate the original task prompt.\n\
2. Focus only on the remaining work needed to reach a review-ready or terminal state.\n\
3. Do the smallest convincing validation for your new changes.\n\
4. In your final assistant message, include exactly one line:\n\
   VK_NEXT: continue\n\
   or\n\
   VK_NEXT: review\n\
   Put the VK_NEXT line near the top of your final message so it is easy to parse."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vk_next_action_parses_simple_line() {
        assert_eq!(
            parse_vk_next_action("VK_NEXT: continue"),
            Some(VkNextAction::Continue)
        );
        assert_eq!(
            parse_vk_next_action("VK_NEXT: review"),
            Some(VkNextAction::Review)
        );
    }

    #[test]
    fn parse_vk_next_action_parses_bullet_line() {
        assert_eq!(
            parse_vk_next_action("- VK_NEXT: continue"),
            Some(VkNextAction::Continue)
        );
        assert_eq!(
            parse_vk_next_action("* VK_NEXT: review"),
            Some(VkNextAction::Review)
        );
    }

    #[test]
    fn parse_vk_next_action_returns_none_when_missing() {
        assert_eq!(parse_vk_next_action("done"), None);
    }

    #[test]
    fn parse_vk_next_action_returns_none_when_invalid_value() {
        assert_eq!(parse_vk_next_action("VK_NEXT: maybe"), None);
    }

    #[test]
    fn parse_vk_next_action_detailed_distinguishes_missing_vs_invalid() {
        assert_eq!(
            parse_vk_next_action_detailed("done"),
            VkNextParseResult::Missing
        );
        assert_eq!(
            parse_vk_next_action_detailed("VK_NEXT: maybe"),
            VkNextParseResult::Invalid {
                raw: "maybe".to_string()
            }
        );
    }

    #[test]
    fn strip_vk_next_lines_removes_marker_lines() {
        let text = "Summary line\nVK_NEXT: continue\nMore details\n- VK_NEXT: review\n";
        assert_eq!(strip_vk_next_lines(text), "Summary line\nMore details");
    }

    #[test]
    fn parse_vk_next_action_parses_case_insensitive_prefix() {
        assert_eq!(
            parse_vk_next_action("vk_next: continue"),
            Some(VkNextAction::Continue)
        );
        assert_eq!(
            parse_vk_next_action("Vk_NeXt: review"),
            Some(VkNextAction::Review)
        );
        assert_eq!(strip_vk_next_lines("vk_next: continue"), "");
    }

    #[test]
    fn vk_next_parsing_is_utf8_safe_for_multilingual_text() {
        let samples = [
            "我用的是 `openspec-explore`（探索模式：只调研/设计，不落地实现）。当前没有活动中的 OpenSpec change（`openspec list --json` 返回空）。",
            "こんにちは世界",
            "안녕하세요 세계",
            "مرحبا بالعالم",
            "שלום עולם",
            "Здравствуйте мир",
            "हैलो वर्ल्ड",
            "😀😃😄😁",
            "• VK_NEXT: continue (not supported marker style)",
            "aéééééééééé", // UTF-8 boundary around 8 bytes is inside 'é'
            "前缀 VK_NEXT: continue 后缀", // marker in the middle of the line
        ];

        for text in samples {
            assert_eq!(parse_vk_next_action(text), None);
            assert_eq!(strip_vk_next_lines(text), text.to_string());
        }
    }
}
