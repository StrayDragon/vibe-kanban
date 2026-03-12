use db::types::VkNextAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VkNextParseResult {
    Parsed(VkNextAction),
    Missing,
    Invalid { raw: String },
}

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
        if trimmed.len() < "vk_next:".len() {
            continue;
        }
        if !trimmed[.."vk_next:".len()].eq_ignore_ascii_case("vk_next:") {
            continue;
        }

        let rest = trimmed["vk_next:".len()..].trim();

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
        if trimmed.len() >= "vk_next:".len()
            && trimmed[.."vk_next:".len()].eq_ignore_ascii_case("vk_next:")
        {
            continue;
        }
        out.push(line);
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
}
