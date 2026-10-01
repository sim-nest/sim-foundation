// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A deliberately narrow Markdown block scanner for the selected roadmap.
//!
//! It answers one question per line: is this line ordinary top-level document
//! prose, or is it inside a context that asserts nothing (front matter, a code
//! fence, an HTML comment or block, a raw-text HTML element, an indented code
//! block, a list item)? Facts are read only from prose lines. It does not
//! render Markdown; anything it cannot classify as prose is refused by the
//! callers, never guessed.

/// One source line: its byte offset, its text without the line terminator, and
/// whether it is ordinary top-level prose.
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

/// Width of a list marker plus the following spaces (content indent), if the
/// line starts a list item: `-`/`*`/`+` or 1-9 digits then `.`/`)`, followed by
/// whitespace or the end of the line.
fn list_item(line: &str) -> Option<usize> {
    let lead = indent(line);
    if lead > 3 {
        return None;
    }
    let body = &line[lead..];
    let marker = if let Some(c) = body.chars().next().filter(|c| matches!(c, '-' | '*' | '+')) {
        c.len_utf8()
    } else {
        let digits = body.chars().take_while(char::is_ascii_digit).count();
        let after = body[digits..].chars().next();
        if (1..=9).contains(&digits) && matches!(after, Some('.' | ')')) {
            digits + 1
        } else {
            return None;
        }
    };
    let rest = &body[marker..];
    if !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')) {
        return None;
    }
    let spaces = rest.chars().take_while(|c| *c == ' ').count().clamp(1, 4);
    Some(lead + marker + spaces)
}

fn raw_text_tag(line: &str) -> Option<&'static str> {
    let body = line
        .trim_start_matches(' ')
        .strip_prefix('<')?
        .to_ascii_lowercase();
    ["script", "pre", "style", "textarea"]
        .into_iter()
        .find(|tag| {
            body.strip_prefix(tag)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(['>', ' ', '\t']))
        })
}

/// Classifies every line of `text`.
pub(super) fn scan(text: &str) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    let mut offset = 0;
    let mut front_matter = false;
    let mut open_fence: Option<(char, usize)> = None;
    let mut in_comment = false;
    let mut in_html = false;
    let mut raw_end: Option<String> = None;
    let mut list_indent: Option<usize> = None;
    let mut prev_blank = true;
    let mut prev_prose_text = false;
    for (n, raw) in text.split_inclusive('\n').enumerate() {
        let line = raw.trim_end_matches(['\n', '\r']);
        let blank = line.trim().is_empty();
        let mut prose = true;
        let mut handled = true;
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
        } else if let Some(end) = raw_end.clone() {
            prose = false;
            if line.to_ascii_lowercase().contains(&end) {
                raw_end = None;
            }
        } else if in_html {
            prose = false;
            if blank {
                in_html = false;
            }
        } else if let Some(width) = list_indent {
            // Inside a list item: indented or blank lines belong to it, and a
            // non-indented line directly after a text line is a lazy
            // continuation. Anything else leaves the list.
            let continues =
                blank || indent(line) >= width || (!prev_blank && !starts_other_block(line));
            if continues {
                prose = false;
            } else {
                list_indent = None;
                handled = false;
            }
        } else {
            handled = false;
        }
        if !handled {
            if let Some(f) = fence(line) {
                prose = false;
                open_fence = Some(f);
            } else if indent(line) <= 3 && line.trim_start_matches(' ').starts_with("<!--") {
                prose = false;
                in_comment = !line.contains("-->");
            } else if let Some(tag) = raw_text_tag(line) {
                prose = false;
                let end = format!("</{tag}");
                if !line.to_ascii_lowercase().contains(&end) {
                    raw_end = Some(end);
                }
            } else if indent(line) <= 3
                && line
                    .trim_start_matches(' ')
                    .strip_prefix('<')
                    .and_then(|r| r.chars().next())
                    .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'))
            {
                prose = false;
                in_html = !blank;
            } else if let Some(width) = list_item(line) {
                prose = false;
                list_indent = Some(width);
            } else if (indent(line) >= 4 || line.starts_with('\t')) && !prev_prose_text {
                // An indented line starts a code block only when no paragraph
                // is in progress; otherwise it continues that paragraph.
                prose = false;
            }
        }
        prev_prose_text = prose && !blank;
        prev_blank = blank;
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
    let setext = body.trim_end();
    body.starts_with('>')
        || body.starts_with('#')
        || body.starts_with('|')
        || body.starts_with("```")
        || body.starts_with("~~~")
        || body.starts_with('<')
        || list_item(line).is_some()
        || (!setext.is_empty() && setext.chars().all(|c| c == '=' || c == '-'))
}
