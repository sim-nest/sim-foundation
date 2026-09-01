use crate::{
    DecodeLimits, WebRecordError, WebRepresentation,
    wire::{cid, field, node, opt_text, read_cid, read_opt_text, read_u32, sym, u32d},
};
use sim_kernel::{ContentId, Datum};

/// A quote anchored to Unicode scalar offsets and optional context/path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceSelector {
    pub representation_id: ContentId,
    pub start: u32,
    pub end: u32,
    pub exact: String,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub structural_path: Option<Vec<String>>,
}
impl EvidenceSelector {
    pub fn checked(
        representation_id: ContentId,
        start: u32,
        end: u32,
        exact: String,
        text: &str,
    ) -> Result<Self, WebRecordError> {
        if start > end || end as usize > text.chars().count() {
            return Err(WebRecordError::InvalidSelector);
        }
        let observed: String = text
            .chars()
            .skip(start as usize)
            .take((end - start) as usize)
            .collect();
        if observed != exact {
            return Err(WebRecordError::InvalidSelector);
        }
        Ok(Self {
            representation_id,
            start,
            end,
            exact,
            prefix: None,
            suffix: None,
            structural_path: None,
        })
    }
    pub fn with_context(
        mut self,
        prefix: Option<String>,
        suffix: Option<String>,
        structural_path: Option<Vec<String>>,
    ) -> Self {
        self.prefix = prefix;
        self.suffix = suffix;
        self.structural_path = structural_path;
        self
    }
    pub fn verify(&self, rep: &WebRepresentation) -> Result<(), WebRecordError> {
        if self.representation_id != rep.content_id {
            return Err(WebRecordError::InvalidSelector);
        }
        Self::checked(
            self.representation_id.clone(),
            self.start,
            self.end,
            self.exact.clone(),
            &rep.text,
        )
        .map(|_| ())
    }
    pub fn to_datum(&self) -> Datum {
        node(
            "selector",
            vec![
                field("representation", cid(&self.representation_id)),
                field("start", u32d(self.start)),
                field("end", u32d(self.end)),
                field("exact", Datum::String(self.exact.clone())),
                field("prefix", opt_text(&self.prefix)),
                field("suffix", opt_text(&self.suffix)),
                field(
                    "path",
                    Datum::Vector(
                        self.structural_path
                            .clone()
                            .unwrap_or_default()
                            .into_iter()
                            .map(Datum::String)
                            .collect(),
                    ),
                ),
            ],
        )
    }
    pub fn from_datum(
        value: &Datum,
        rep: &WebRepresentation,
        limits: DecodeLimits,
    ) -> Result<Self, WebRecordError> {
        let Datum::Node { tag, fields } = value else {
            return Err(WebRecordError::InvalidRecord("selector"));
        };
        if tag != &sym("selector") || fields.len() != 7 {
            return Err(WebRecordError::InvalidRecord("selector"));
        }
        let get = |i: usize, name: &str| {
            if fields[i].0 == sym(name) {
                Ok(&fields[i].1)
            } else {
                Err(WebRecordError::InvalidRecord("selector ordering"))
            }
        };
        let representation_id = read_cid(get(0, "representation")?)?;
        let start = read_u32(get(1, "start")?)?;
        let end = read_u32(get(2, "end")?)?;
        let Datum::String(exact) = get(3, "exact")? else {
            return Err(WebRecordError::InvalidRecord("exact"));
        };
        if exact.len() > limits.max_text_bytes {
            return Err(WebRecordError::BoundExceeded("exact"));
        }
        let mut selector = Self::checked(representation_id, start, end, exact.clone(), &rep.text)?;
        selector.prefix = read_opt_text(get(4, "prefix")?, limits)?;
        selector.suffix = read_opt_text(get(5, "suffix")?, limits)?;
        let Datum::Vector(path) = get(6, "path")? else {
            return Err(WebRecordError::InvalidRecord("path"));
        };
        if path.len() > limits.max_items {
            return Err(WebRecordError::BoundExceeded("path"));
        }
        selector.structural_path = if path.is_empty() {
            None
        } else {
            Some(
                path.iter()
                    .map(|v| match v {
                        Datum::String(s) => Ok(s.clone()),
                        _ => Err(WebRecordError::InvalidRecord("path")),
                    })
                    .collect::<Result<_, _>>()?,
            )
        };
        selector.verify(rep)?;
        Ok(selector)
    }
}
