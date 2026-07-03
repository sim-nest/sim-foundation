//! One value-addressing primitive over `Expr`.
//!
//! A path is a sequence of segments: a map key or a sequence index. The wire
//! form of a segment is `Vector([sym("k"), key])` for a map key or
//! `Vector([sym("i"), text(index)])` for a sequence index -- exactly the form
//! the scene differ and the universal editor already emit, so this is a drop-in
//! for all three previous copies. `set_at`/`remove_at` are immutable.

use sim_kernel::Expr;

use crate::build::sym;

/// One step of a path: into a map by key, or into a sequence by index.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    /// A map key.
    Key(Expr),
    /// A sequence index.
    Index(usize),
}

/// A path from a root value to a nested value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path(pub Vec<Segment>);

/// A failure navigating or editing along a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    /// The path value was not a list of segments.
    NotAList,
    /// A path segment was malformed.
    BadSegment,
    /// A key segment targeted a non-map value.
    NotAMap,
    /// An index segment targeted a non-sequence value.
    NotASequence,
    /// A key segment referenced a missing key (mid-path).
    MissingKey,
    /// An index segment was out of bounds.
    IndexOutOfBounds,
    /// `remove_at` on a sequence index, which is not supported.
    RemoveIndexUnsupported,
    /// `remove_at` with an empty path.
    EmptyRemove,
}

impl Path {
    /// An empty path (addresses the root).
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Extend the path with a map-key step.
    pub fn key(mut self, key: Expr) -> Self {
        self.0.push(Segment::Key(key));
        self
    }

    /// Extend the path with a sequence-index step.
    pub fn index(mut self, index: usize) -> Self {
        self.0.push(Segment::Index(index));
        self
    }

    /// Encode the path to its `k`/`i` wire form.
    pub fn to_expr(&self) -> Expr {
        Expr::List(
            self.0
                .iter()
                .map(|segment| match segment {
                    Segment::Key(key) => Expr::Vector(vec![sym("k"), key.clone()]),
                    Segment::Index(index) => {
                        Expr::Vector(vec![sym("i"), Expr::String(index.to_string())])
                    }
                })
                .collect(),
        )
    }

    /// Parse a path from its `k`/`i` wire form.
    pub fn from_expr(expr: &Expr) -> Result<Self, PathError> {
        let Expr::List(segments) = expr else {
            return Err(PathError::NotAList);
        };
        let mut out = Vec::with_capacity(segments.len());
        for segment in segments {
            out.push(parse_segment(segment)?);
        }
        Ok(Path(out))
    }
}

fn parse_segment(segment: &Expr) -> Result<Segment, PathError> {
    let Expr::Vector(parts) = segment else {
        return Err(PathError::BadSegment);
    };
    match parts.as_slice() {
        [Expr::Symbol(tag), key] if &*tag.name == "k" => Ok(Segment::Key(key.clone())),
        [Expr::Symbol(tag), Expr::String(index)] if &*tag.name == "i" => index
            .parse::<usize>()
            .map(Segment::Index)
            .map_err(|_| PathError::BadSegment),
        _ => Err(PathError::BadSegment),
    }
}

fn seq_items(value: &Expr) -> Option<&Vec<Expr>> {
    match value {
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => Some(items),
        _ => None,
    }
}

fn rewrap(original: &Expr, items: Vec<Expr>) -> Expr {
    match original {
        Expr::Vector(_) => Expr::Vector(items),
        Expr::Set(_) => Expr::Set(items),
        _ => Expr::List(items),
    }
}

/// Borrow the value at `path`, if present.
pub fn get<'a>(root: &'a Expr, path: &Path) -> Option<&'a Expr> {
    let mut current = root;
    for segment in &path.0 {
        current = match segment {
            Segment::Key(key) => {
                let Expr::Map(entries) = current else {
                    return None;
                };
                entries
                    .iter()
                    .find_map(|(entry_key, value)| (entry_key == key).then_some(value))?
            }
            Segment::Index(index) => seq_items(current)?.get(*index)?,
        };
    }
    Some(current)
}

/// Set the value at `path`, preserving every sibling, in a new root value. A
/// missing intermediate key or out-of-range index is an error.
pub fn set_at(root: &Expr, path: &Path, value: Expr) -> Result<Expr, PathError> {
    set_rec(root, &path.0, value)
}

fn set_rec(node: &Expr, segments: &[Segment], value: Expr) -> Result<Expr, PathError> {
    let Some((first, rest)) = segments.split_first() else {
        return Ok(value);
    };
    match first {
        Segment::Key(key) => {
            let Expr::Map(entries) = node else {
                return Err(PathError::NotAMap);
            };
            let mut entries = entries.clone();
            match entries.iter_mut().find(|(entry_key, _)| entry_key == key) {
                Some(slot) => slot.1 = set_rec(&slot.1.clone(), rest, value)?,
                None if rest.is_empty() => entries.push((key.clone(), value)),
                None => return Err(PathError::MissingKey),
            }
            Ok(Expr::Map(entries))
        }
        Segment::Index(index) => {
            let mut items = seq_items(node).ok_or(PathError::NotASequence)?.clone();
            let slot = items.get(*index).ok_or(PathError::IndexOutOfBounds)?;
            let replaced = set_rec(&slot.clone(), rest, value)?;
            items[*index] = replaced;
            Ok(rewrap(node, items))
        }
    }
}

/// Remove the value at `path` (the final segment must be a map key), in a new
/// root value.
pub fn remove_at(root: &Expr, path: &Path) -> Result<Expr, PathError> {
    let Some((last, parents)) = path.0.split_last() else {
        return Err(PathError::EmptyRemove);
    };
    let Segment::Key(key) = last else {
        return Err(PathError::RemoveIndexUnsupported);
    };
    remove_rec(root, parents, key)
}

fn remove_rec(node: &Expr, parents: &[Segment], key: &Expr) -> Result<Expr, PathError> {
    let Some((first, rest)) = parents.split_first() else {
        let Expr::Map(entries) = node else {
            return Err(PathError::NotAMap);
        };
        return Ok(Expr::Map(
            entries
                .iter()
                .filter(|(entry_key, _)| entry_key != key)
                .cloned()
                .collect(),
        ));
    };
    match first {
        Segment::Key(parent_key) => {
            let Expr::Map(entries) = node else {
                return Err(PathError::NotAMap);
            };
            let mut entries = entries.clone();
            let slot = entries
                .iter_mut()
                .find(|(entry_key, _)| entry_key == parent_key)
                .ok_or(PathError::MissingKey)?;
            slot.1 = remove_rec(&slot.1.clone(), rest, key)?;
            Ok(Expr::Map(entries))
        }
        Segment::Index(index) => {
            let mut items = seq_items(node).ok_or(PathError::NotASequence)?.clone();
            let slot = items.get(*index).ok_or(PathError::IndexOutOfBounds)?;
            items[*index] = remove_rec(&slot.clone(), rest, key)?;
            Ok(rewrap(node, items))
        }
    }
}
