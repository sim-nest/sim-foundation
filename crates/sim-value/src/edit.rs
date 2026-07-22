//! Side-effect-free text edit primitives.

use sim_kernel::{Error, Result};

/// Replaces `old` with `new` in `text`.
///
/// The pattern must be present. When `replace_all` is false, the pattern must
/// occur exactly once so a caller cannot patch an unintended occurrence.
pub fn edit(text: &str, old: &str, new: &str, replace_all: bool) -> Result<String> {
    if old.is_empty() {
        return Err(Error::Eval("edit: old pattern is empty".to_owned()));
    }
    let matches = text.matches(old).count();
    match matches {
        0 => Err(Error::Eval(format!("edit: pattern not found: {old:?}"))),
        n if n > 1 && !replace_all => Err(Error::Eval(format!(
            "edit: pattern is not unique ({n} matches); pass replace_all"
        ))),
        _ if replace_all => Ok(text.replace(old, new)),
        _ => Ok(text.replacen(old, new, 1)),
    }
}

/// Replaces a 1-based inclusive line range with `new`.
///
/// `new` is inserted exactly as provided. Callers that want the replacement to
/// end in a newline include it in `new`.
pub fn edit_lines(text: &str, start: usize, end: usize, new: &str) -> Result<String> {
    if start == 0 {
        return Err(Error::Eval(
            "edit-lines: start must be at least 1".to_owned(),
        ));
    }
    if end < start {
        return Err(Error::Eval(
            "edit-lines: end must be greater than or equal to start".to_owned(),
        ));
    }

    let lines = text.split_inclusive('\n').collect::<Vec<_>>();
    if end > lines.len() {
        return Err(Error::Eval(format!(
            "edit-lines: range {start}..{end} exceeds {} line(s)",
            lines.len()
        )));
    }

    let mut edited = String::new();
    for line in &lines[..start - 1] {
        edited.push_str(line);
    }
    edited.push_str(new);
    for line in &lines[end..] {
        edited.push_str(line);
    }
    Ok(edited)
}
