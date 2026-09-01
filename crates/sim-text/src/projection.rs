//! Runtime, codec, browse, Shape, and read-construction projections.

use std::any::Any;

use sim_kernel::{
    Cx, Error, Expr, MatchScore, Object, ObjectCompat, ObjectEncode, ObjectEncoding,
    ReadConstructor, Result, Shape, ShapeDoc, ShapeMatch, ShapeRef, Symbol, Value,
};

use crate::{CodeUnitString, CodeUnitStringError};

/// Stable class/tag symbol used by the standard read-construct representation.
pub const CODE_UNIT_STRING_SYMBOL: &str = "text/CodeUnitString";

fn symbol() -> Symbol {
    Symbol::qualified("text", "CodeUnitString")
}

/// Project exact code units into the codec-neutral expression graph.
///
/// The payload is big-endian bytes, making the representation independent of
/// host byte order and capable of carrying lone surrogates without coercion.
pub fn code_unit_string_to_expr(text: &CodeUnitString) -> Expr {
    let mut bytes = Vec::with_capacity(text.len().saturating_mul(2));
    for unit in text.code_units() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    Expr::Extension {
        tag: symbol(),
        payload: Box::new(Expr::Bytes(bytes)),
    }
}

/// Recover exact code units from the tagged codec-neutral representation.
pub fn code_unit_string_from_expr(expr: &Expr) -> Result<CodeUnitString> {
    let Expr::Extension { tag, payload } = expr else {
        return Err(Error::Eval("expected tagged exact-unit string".to_owned()));
    };
    if tag != &symbol() {
        return Err(Error::Eval(format!(
            "expected tag {}, found {tag}",
            symbol()
        )));
    }
    let Expr::Bytes(bytes) = payload.as_ref() else {
        return Err(Error::Eval(
            "exact-unit string payload must be bytes".to_owned(),
        ));
    };
    if bytes.len() % 2 != 0 {
        return Err(Error::Eval(format!(
            "exact-unit string payload has odd byte length {}",
            bytes.len()
        )));
    }
    Ok(CodeUnitString::from_code_units(
        bytes
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect(),
    ))
}

/// Browse table for an exact-unit string.
///
/// `code-unit-length` and `scalar-text` are deliberately separate: invalid
/// UTF-16 has no scalar-text member, so browsing never relabels it as text.
pub fn code_unit_string_browse(cx: &mut Cx, text: &CodeUnitString) -> Result<Value> {
    let mut entries = vec![
        (
            Symbol::new("kind"),
            cx.factory().string(CODE_UNIT_STRING_SYMBOL.to_owned())?,
        ),
        (
            Symbol::new("code-unit-length"),
            cx.factory().string(text.len().to_string())?,
        ),
        (
            Symbol::new("code-units-be"),
            cx.factory().bytes(match code_unit_string_to_expr(text) {
                Expr::Extension { payload, .. } => match *payload {
                    Expr::Bytes(bytes) => bytes,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            })?,
        ),
    ];
    if let Ok(scalar) = text.to_scalar() {
        entries.push((Symbol::new("scalar-text"), cx.factory().string(scalar)?));
    }
    cx.factory().table(entries)
}

impl Object for CodeUnitString {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        let body = self
            .code_units()
            .map(|unit| format!("{unit:04x}"))
            .collect::<Vec<_>>()
            .join(" ");
        Ok(format!("#<{} [{body}]>", CODE_UNIT_STRING_SYMBOL))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ObjectCompat for CodeUnitString {
    fn as_expr(&self, _cx: &mut Cx) -> Result<Expr> {
        Ok(code_unit_string_to_expr(self))
    }

    fn as_table(&self, cx: &mut Cx) -> Result<Value> {
        code_unit_string_browse(cx, self)
    }

    fn as_object_encoder(&self) -> Option<&dyn ObjectEncode> {
        Some(self)
    }
}

impl ObjectEncode for CodeUnitString {
    fn object_encoding(&self, _cx: &mut Cx) -> Result<ObjectEncoding> {
        let Expr::Extension { payload, .. } = code_unit_string_to_expr(self) else {
            unreachable!()
        };
        Ok(ObjectEncoding::Constructor {
            class: symbol(),
            args: vec![*payload],
        })
    }
}

/// Shape that accepts exact-unit strings, never ordinary scalar strings.
#[derive(Clone, Copy, Debug, Default)]
pub struct CodeUnitStringShape;

impl Shape for CodeUnitStringShape {
    fn symbol(&self) -> Option<Symbol> {
        Some(symbol())
    }

    fn check_value(&self, _cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        Ok(
            if value.object().downcast_ref::<CodeUnitString>().is_some() {
                ShapeMatch::accept(MatchScore::exact(1))
            } else {
                ShapeMatch::reject("expected exact-unit string")
            },
        )
    }

    fn check_expr(&self, _cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        Ok(if code_unit_string_from_expr(expr).is_ok() {
            ShapeMatch::accept(MatchScore::exact(1))
        } else {
            ShapeMatch::reject("expected tagged exact-unit string")
        })
    }

    fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
        Ok(ShapeDoc::new("exact UTF-16 code-unit string")
            .with_detail("distinct from scalar Unicode text"))
    }
}

/// Standard read constructor for [`CodeUnitString`].
#[derive(Clone, Copy, Debug, Default)]
pub struct CodeUnitStringReadConstructor;

impl Object for CodeUnitStringReadConstructor {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok(format!("#<read-constructor {}>", CODE_UNIT_STRING_SYMBOL))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ObjectCompat for CodeUnitStringReadConstructor {
    fn as_read_constructor(&self) -> Option<&dyn ReadConstructor> {
        Some(self)
    }
}

impl ReadConstructor for CodeUnitStringReadConstructor {
    fn symbol(&self) -> Symbol {
        symbol()
    }

    fn args_shape(&self, cx: &mut Cx) -> Result<ShapeRef> {
        cx.factory().nil()
    }

    fn construct_read(&self, cx: &mut Cx, args: Vec<Value>) -> Result<Value> {
        let [arg] = args.as_slice() else {
            return Err(Error::Eval(
                "exact-unit string read constructor expects one byte string".to_owned(),
            ));
        };
        let expr = arg.object().as_expr(cx)?;
        let Expr::Bytes(bytes) = expr else {
            return Err(Error::Eval(
                "exact-unit string read constructor expects bytes".to_owned(),
            ));
        };
        let tagged = Expr::Extension {
            tag: symbol(),
            payload: Box::new(Expr::Bytes(bytes)),
        };
        cx.factory()
            .opaque(std::sync::Arc::new(code_unit_string_from_expr(&tagged)?))
    }
}

/// Convert for a scalar-text-only codec, refusing at the exact bad unit.
pub fn scalar_text(text: &CodeUnitString) -> core::result::Result<String, CodeUnitStringError> {
    text.to_scalar()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sim_kernel::{DefaultFactory, HandleSeed, NoopEvalPolicy};

    use crate::{CodeUnitOffset, InvalidSurrogate};

    use super::*;

    #[test]
    fn lone_surrogate_round_trips_tagged_bytes_and_scalar_codec_refuses_located() {
        let exact = CodeUnitString::from_code_units(vec![0x0061, 0xd800, 0x0062]);
        let encoded = code_unit_string_to_expr(&exact);
        assert_eq!(code_unit_string_from_expr(&encoded).unwrap(), exact);
        assert_eq!(
            scalar_text(&exact),
            Err(CodeUnitStringError::LoneSurrogate(InvalidSurrogate {
                offset: CodeUnitOffset::new(1),
                unit: 0xd800,
            }))
        );
    }

    #[test]
    fn shape_and_browse_keep_exact_units_distinct_from_scalar_text() {
        let mut cx = Cx::new(
            Arc::new(NoopEvalPolicy),
            Arc::new(DefaultFactory),
            HandleSeed::new(0),
        );
        let exact = CodeUnitString::from_code_units(vec![0xd800]);
        let value = cx.factory().opaque(Arc::new(exact.clone())).unwrap();
        assert!(
            CodeUnitStringShape
                .check_value(&mut cx, value)
                .unwrap()
                .accepted
        );
        assert!(
            !CodeUnitStringShape
                .check_expr(&mut cx, &Expr::String("text".to_owned()))
                .unwrap()
                .accepted
        );
        let table = code_unit_string_browse(&mut cx, &exact).unwrap();
        let entries = table
            .object()
            .as_table_impl()
            .unwrap()
            .entries(&mut cx)
            .unwrap();
        assert!(
            !entries
                .iter()
                .any(|(key, _)| key == &Symbol::new("scalar-text"))
        );
    }
}
