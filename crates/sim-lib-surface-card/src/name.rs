//! External-name policies for projecting kernel symbols onto foreign surfaces.

use sim_kernel::Symbol;

/// Policy selecting how a kernel [`Symbol`] is mangled into a surface-specific
/// external name.
///
/// Each variant fails closed toward the character set its surface accepts; none
/// of them is reversible, so they are used for naming only, never for routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalNamePolicy {
    /// MCP tool name: keep `[A-Za-z0-9_-]`, replace everything else (including
    /// `/` and `.`) with a single `_` per character. No run-collapsing, no trim.
    McpTool,
    /// OpenAI tool name: keep ascii-alphanumeric characters; collapse any run of
    /// non-alphanumeric characters to a single `_`; then trim leading/trailing
    /// `_`. May return an empty string (callers supply their own fallback).
    OpenAiTool,
    /// File-safe name: keep ascii-alphanumeric and `-`, `_`, `.`; replace every
    /// other character with `_` per character. No run-collapsing, no trim.
    FileSafe,
    /// Human-readable name: the qualified symbol string unchanged.
    HumanReadable,
}

/// Project `symbol` onto an external name according to `policy`.
pub fn external_name(symbol: &Symbol, policy: ExternalNamePolicy) -> String {
    let qualified = symbol.as_qualified_str();
    match policy {
        ExternalNamePolicy::OpenAiTool => openai_tool(&qualified),
        ExternalNamePolicy::FileSafe => map_chars(&qualified, |ch| matches!(ch, '-' | '_' | '.')),
        ExternalNamePolicy::McpTool => map_chars(&qualified, |ch| matches!(ch, '-' | '_')),
        ExternalNamePolicy::HumanReadable => qualified,
    }
}

/// Builds the OpenAI tool name for a qualified symbol, minus the empty-string
/// fallback (the caller keeps that).
fn openai_tool(qualified: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = false;
    for ch in qualified.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_separator = false;
        } else if !last_was_separator {
            out.push('_');
            last_was_separator = true;
        }
    }
    out.trim_matches('_').to_owned()
}

/// Per-character map: keep ascii-alphanumeric and any character `keep_extra`
/// accepts; replace everything else with a single `_`.
fn map_chars(qualified: &str, keep_extra: impl Fn(char) -> bool) -> String {
    qualified
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || keep_extra(ch) {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_on_qualified_symbol() {
        let symbol = Symbol::qualified("skill", "do_thing/v2");
        // as_qualified_str() == "skill/do_thing/v2"
        assert_eq!(
            external_name(&symbol, ExternalNamePolicy::OpenAiTool),
            "skill_do_thing_v2"
        );
        assert_eq!(
            external_name(&symbol, ExternalNamePolicy::FileSafe),
            "skill_do_thing_v2"
        );
        assert_eq!(
            external_name(&symbol, ExternalNamePolicy::McpTool),
            "skill_do_thing_v2"
        );
        assert_eq!(
            external_name(&symbol, ExternalNamePolicy::HumanReadable),
            "skill/do_thing/v2"
        );
    }

    #[test]
    fn openai_tool_empty_case() {
        // A symbol whose qualified string contains no ascii-alphanumeric
        // characters mangles to the empty string under OpenAiTool.
        let symbol = Symbol::qualified("--", "//");
        assert_eq!(external_name(&symbol, ExternalNamePolicy::OpenAiTool), "");
    }
}
