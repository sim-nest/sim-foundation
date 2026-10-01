// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A deliberately narrow Markdown block scanner for the selected roadmap.
//!
//! It answers one question per line: is this line ordinary document prose, or
//! is it inside a context that asserts nothing (front matter, a code fence, an
//! HTML comment or block, an indented code block)? Facts are read only from
//! prose lines. It does not render Markdown; anything it cannot classify as
//! prose is refused by the callers, never guessed.

/// One source line: its byte offset, its text without the line terminator, and
/// whether it is ordinary prose.
#[derive(Clone, Copy, Debug)]
pub(super) struct Line<'a> {
    pub(super) start: usize,
    pub(super) text: &'a str,
    pub(super) prose: bool,
}

fn indent(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

fn fence(line: &str) -> Option<(char, usize)> {
    if indent(line) > 3 {
        return None;
    }
    let body = line.trim_start_matches(' ');
    let marker = body.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let run = body.chars().take_while(|c| *c == marker).count();
    (run >= 3).then_some((marker, run))
}

/// Classifies every line of `text`.
pub(super) fn scan(text: &str) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    let mut offset = 0;
    let mut front_matter = false;
    let mut open_fence: Option<(char, usize)> = None;
    let mut in_comment = false;
    let mut in_html = false;
    for (n, raw) in text.split_inclusive('\n').enumerate() {
        let line = raw.trim_end_matches(['\n', '\r']);
        let mut prose = true;
        if n == 0 && line.trim_end() == "---" {
            front_matter = true;
            prose = false;
        } else if front_matter {
            prose = false;
            if matches!(line.trim_end(), "---" | "...") {
                front_matter = false;
            }
        } else if let Some((marker, run)) = open_fence {
            prose = false;
            if let Some((m, r)) = fence(line) {
                let closing = line.trim_start_matches(' ').chars().all(|c| c == m);
                if m == marker && r >= run && closing {
                    open_fence = None;
                }
            }
        } else if in_comment {
            prose = false;
            if line.contains("-->") {
                in_comment = false;
            }
        } else if in_html {
            prose = false;
            if line.trim().is_empty() {
                in_html = false;
            }
        } else if let Some(f) = fence(line) {
            prose = false;
            open_fence = Some(f);
        } else if indent(line) <= 3 && line.trim_start_matches(' ').starts_with("<!--") {
            prose = false;
            in_comment = !line.contains("-->");
        } else if indent(line) <= 3
            && line
                .trim_start_matches(' ')
                .strip_prefix('<')
                .and_then(|r| r.chars().next())
                .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'))
        {
            prose = false;
            in_html = !line.trim().is_empty();
        } else if indent(line) >= 4 || line.starts_with('\t') {
            prose = false;
        }
        out.push(Line {
            start: offset,
            text: line,
            prose,
        });
        offset += raw.len();
    }
    out
}

/// ATX heading content (`### text`, up to three leading spaces, optional
/// closing hashes), or `None` if the line is not an ATX heading.
pub(super) fn atx_heading(line: &str) -> Option<(usize, &str)> {
    if indent(line) > 3 {
        return None;
    }
    let body = line.trim_start_matches(' ').trim_end();
    let level = body.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &body[level..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let rest = rest.trim();
    let rest = rest.trim_end_matches('#').trim_end();
    Some((level, rest))
}

/// Whether a prose line can continue or interrupt a paragraph as another
/// block (list item, quotation, heading, table row, setext underline, fence).
pub(super) fn starts_other_block(line: &str) -> bool {
    let body = line.trim_start_matches(' ');
    body.starts_with('>')
        || body.starts_with('#')
        || body.starts_with('|')
        || body.starts_with("- ")
        || body.starts_with("* ")
        || body.starts_with("+ ")
        || body.starts_with("```")
        || body.starts_with("~~~")
        || body.starts_with('<')
        || (!body.is_empty() && body.chars().all(|c| c == '=' || c == '-'))
        || body.chars().next().is_some_and(|c| c.is_ascii_digit())
}
