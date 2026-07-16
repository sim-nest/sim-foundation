//! Shape-backed config contracts for cookbook-facing runtime configuration.

use std::sync::Arc;

use sim_config::{ConfigShape, ConfigTable, config_field_name, same_config_field};
use sim_kernel::{
    Cx, Expr, ExprKind, MatchScore, Result, Shape, ShapeDoc, ShapeMatch, Symbol, Value,
};
use sim_value::build::{int, list, map, text};

/// Returns the stable config library id for cookbook defaults.
pub fn cookbook_lib_symbol() -> Symbol {
    Symbol::qualified("sim", "cookbook")
}

/// Returns the stable config library id for stream-host defaults.
pub fn stream_host_lib_symbol() -> Symbol {
    Symbol::qualified("stream", "host")
}

/// Returns the stable config library id for model defaults.
pub fn model_defaults_lib_symbol() -> Symbol {
    Symbol::qualified("model", "defaults")
}

/// Shape-backed contract for the `sim/cookbook` config table.
///
/// The table carries `minimum_loaded`, `hide`, and `order` string lists plus
/// repeated `loadable_lib` rows. Each row has an `id` shown by the cookbook and
/// a host-owned `source` key resolved later by the runtime host.
pub fn cookbook_config_shape() -> ConfigShape {
    let lib = cookbook_lib_symbol();
    ConfigShape::new(
        lib.clone(),
        Arc::new(TableContract::new(
            Symbol::qualified("config-shape", "sim/cookbook"),
            vec![
                FieldSpec::optional("minimum_loaded", FieldShape::StringList),
                FieldSpec::optional("hide", FieldShape::StringList),
                FieldSpec::optional("order", FieldShape::StringList),
                FieldSpec::optional(
                    "loadable_lib",
                    FieldShape::TableList(vec![
                        FieldSpec::required("id", FieldShape::Kind(ExprKind::String)),
                        FieldSpec::required("source", FieldShape::Kind(ExprKind::String)),
                    ]),
                ),
            ],
        )),
        ConfigTable::new(
            lib,
            map(vec![
                ("minimum_loaded", string_list(["codec/lisp"])),
                ("loadable_lib", list(Vec::new())),
            ]),
        )
        .expect("cookbook defaults are a config table"),
    )
}

/// Shape-backed contract for the `stream/host` config table.
pub fn stream_host_config_shape() -> ConfigShape {
    let lib = stream_host_lib_symbol();
    ConfigShape::new(
        lib.clone(),
        Arc::new(TableContract::new(
            Symbol::qualified("config-shape", "stream/host"),
            vec![
                FieldSpec::optional("audio_backend_candidates", FieldShape::StringList),
                FieldSpec::optional("midi_backend_candidates", FieldShape::StringList),
                FieldSpec::optional("audio_backend_regex", FieldShape::Kind(ExprKind::String)),
                FieldSpec::optional("midi_backend_regex", FieldShape::Kind(ExprKind::String)),
                FieldSpec::optional("sample_rate_hz", FieldShape::Kind(ExprKind::Number)),
                FieldSpec::optional("max_block_frames", FieldShape::Kind(ExprKind::Number)),
            ],
        )),
        ConfigTable::new(
            lib,
            map(vec![
                ("audio_backend_candidates", string_list(["modeled"])),
                ("midi_backend_candidates", string_list(["modeled"])),
                ("audio_backend_regex", text("^(?:modeled)$")),
                ("midi_backend_regex", text("^(?:modeled)$")),
                ("sample_rate_hz", int(48_000)),
                ("max_block_frames", int(512)),
            ]),
        )
        .expect("stream-host defaults are a config table"),
    )
}

/// Shape-backed contract for the `model/defaults` config table.
pub fn model_defaults_config_shape() -> ConfigShape {
    let lib = model_defaults_lib_symbol();
    ConfigShape::new(
        lib.clone(),
        Arc::new(TableContract::new(
            Symbol::qualified("config-shape", "model/defaults"),
            vec![
                FieldSpec::optional("model_regex", FieldShape::Kind(ExprKind::String)),
                FieldSpec::optional("provider_regex", FieldShape::Kind(ExprKind::String)),
                FieldSpec::optional("prefer_local", FieldShape::Kind(ExprKind::Bool)),
                FieldSpec::optional("default_model", FieldShape::Kind(ExprKind::String)),
                FieldSpec::optional("openai_key_present", FieldShape::Kind(ExprKind::Bool)),
                FieldSpec::optional("openai_base_present", FieldShape::Kind(ExprKind::Bool)),
                FieldSpec::optional("ollama_host_present", FieldShape::Kind(ExprKind::Bool)),
            ],
        )),
        ConfigTable::new(
            lib,
            map(vec![
                ("model_regex", text(r"^(?:fixture/|sim/|gpt-4\.1|o[34]).*")),
                ("provider_regex", text("^(?:modeled)$")),
                ("prefer_local", Expr::Bool(true)),
                ("default_model", text("fixture/echo")),
                ("openai_key_present", Expr::Bool(false)),
                ("openai_base_present", Expr::Bool(false)),
                ("ollama_host_present", Expr::Bool(false)),
            ]),
        )
        .expect("model defaults are a config table"),
    )
}

/// Representative config contracts exported by the cookbook foundation crate.
pub fn representative_config_shapes() -> Vec<ConfigShape> {
    vec![
        cookbook_config_shape(),
        stream_host_config_shape(),
        model_defaults_config_shape(),
    ]
}

#[derive(Clone)]
struct FieldSpec {
    name: &'static str,
    shape: FieldShape,
    required: bool,
}

impl FieldSpec {
    fn required(name: &'static str, shape: FieldShape) -> Self {
        Self {
            name,
            shape,
            required: true,
        }
    }

    fn optional(name: &'static str, shape: FieldShape) -> Self {
        Self {
            name,
            shape,
            required: false,
        }
    }
}

#[derive(Clone)]
enum FieldShape {
    Kind(ExprKind),
    StringList,
    TableList(Vec<FieldSpec>),
}

impl FieldShape {
    fn check(&self, name: &str, value: &Expr) -> std::result::Result<(), String> {
        match self {
            Self::Kind(kind) => {
                if kind.matches(value) {
                    Ok(())
                } else {
                    Err(format!("{name} expected {}", kind.name()))
                }
            }
            Self::StringList => check_string_list(name, value),
            Self::TableList(fields) => check_table_list(name, value, fields),
        }
    }
}

struct TableContract {
    symbol: Symbol,
    fields: Vec<FieldSpec>,
}

impl TableContract {
    fn new(symbol: Symbol, fields: Vec<FieldSpec>) -> Self {
        Self { symbol, fields }
    }
}

impl Shape for TableContract {
    fn symbol(&self) -> Option<Symbol> {
        Some(self.symbol.clone())
    }

    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        let expr = value.object().as_expr(cx)?;
        self.check_expr(cx, &expr)
    }

    fn check_expr(&self, _cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        match check_table(expr, &self.fields, "config table") {
            Ok(()) => Ok(ShapeMatch::accept(MatchScore::exact(
                self.fields.len() as i32
            ))),
            Err(message) => Ok(ShapeMatch::reject(message)),
        }
    }

    fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
        Ok(ShapeDoc::new(self.symbol.as_qualified_str()))
    }
}

fn check_table(
    expr: &Expr,
    fields: &[FieldSpec],
    context: &str,
) -> std::result::Result<(), String> {
    let Expr::Map(entries) = expr else {
        return Err(format!("{context} expected map"));
    };
    for (index, (key, value)) in entries.iter().enumerate() {
        if entries[..index]
            .iter()
            .any(|(prior_key, _)| same_config_field(prior_key, key))
        {
            return Err(format!(
                "{context}: duplicate key {}",
                config_field_name(key).unwrap_or("<opaque>")
            ));
        }
        let Some(name) = config_field_name(key) else {
            return Err(format!("{context} key expected symbol or string"));
        };
        let Some(field) = fields.iter().find(|field| field.name == name) else {
            return Err(format!("{context}: extra key {name}"));
        };
        field.shape.check(name, value)?;
    }
    for field in fields.iter().filter(|field| field.required) {
        if !entries
            .iter()
            .any(|(key, _)| config_field_name(key).is_some_and(|name| name == field.name))
        {
            return Err(format!("{context}: missing required key {}", field.name));
        }
    }
    Ok(())
}

fn check_string_list(name: &str, value: &Expr) -> std::result::Result<(), String> {
    let Expr::List(items) = value else {
        return Err(format!("{name} expected list"));
    };
    for (index, item) in items.iter().enumerate() {
        if !matches!(item, Expr::String(_)) {
            return Err(format!("{name}[{index}] expected string"));
        }
    }
    Ok(())
}

fn check_table_list(
    name: &str,
    value: &Expr,
    fields: &[FieldSpec],
) -> std::result::Result<(), String> {
    let Expr::List(items) = value else {
        return Err(format!("{name} expected repeated table list"));
    };
    for (index, item) in items.iter().enumerate() {
        check_table(item, fields, &format!("{name}[{index}]"))?;
    }
    Ok(())
}

fn string_list(items: impl IntoIterator<Item = impl Into<String>>) -> Expr {
    list(items.into_iter().map(text).collect())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sim_kernel::{Cx, DefaultFactory, NoopEvalPolicy};

    use super::*;

    fn cx() -> Cx {
        Cx::new(Arc::new(NoopEvalPolicy), Arc::new(DefaultFactory))
    }

    #[test]
    fn cookbook_shape_accepts_provider_fields_together() {
        use sim_config::ConfigView;

        let mut cx = cx();
        let shape = cookbook_config_shape();
        let table = ConfigTable::new(
            cookbook_lib_symbol(),
            map(vec![
                ("minimum_loaded", string_list(["codec/lisp"])),
                ("hide", string_list(["demo/hidden"])),
                ("order", string_list(["numbers/cas", "codec/algol"])),
                (
                    "loadable_lib",
                    list(vec![
                        map(vec![
                            ("id", text("numbers/cas")),
                            ("source", text("symbol:numbers/cas")),
                        ]),
                        map(vec![
                            ("id", text("codec/algol")),
                            ("source", text("symbol:codec/algol")),
                        ]),
                    ]),
                ),
            ]),
        )
        .unwrap();

        let effective = shape.validate(&mut cx, &table).unwrap();
        let view = ConfigView::new(&effective);

        assert!(matches!(effective.table, Expr::Map(_)));
        assert_eq!(view.string_array("minimum_loaded").unwrap(), ["codec/lisp"]);
        assert_eq!(view.string_array("hide").unwrap(), ["demo/hidden"]);
        assert_eq!(
            view.string_array("order").unwrap(),
            ["numbers/cas", "codec/algol"]
        );
    }

    #[test]
    fn cookbook_shape_accepts_string_keyed_config_fields() {
        let mut cx = cx();
        let shape = cookbook_config_shape();
        let table = ConfigTable::new(
            cookbook_lib_symbol(),
            Expr::Map(vec![
                (
                    Expr::String("minimum_loaded".to_owned()),
                    string_list(["codec/lisp"]),
                ),
                (
                    Expr::String("loadable_lib".to_owned()),
                    list(vec![Expr::Map(vec![
                        (Expr::String("id".to_owned()), text("numbers/cas")),
                        (
                            Expr::String("source".to_owned()),
                            text("symbol:numbers/cas"),
                        ),
                    ])]),
                ),
            ]),
        )
        .unwrap();

        let effective = shape.validate(&mut cx, &table).unwrap();

        assert!(matches!(effective.table, Expr::Map(_)));
    }

    #[test]
    fn cookbook_shape_rejects_malformed_loadable_rows() {
        let mut cx = cx();
        let shape = cookbook_config_shape();
        let table = ConfigTable::new(
            cookbook_lib_symbol(),
            map(vec![(
                "loadable_lib",
                list(vec![map(vec![("id", text("numbers/cas"))])]),
            )]),
        )
        .unwrap();

        let report = shape.validate(&mut cx, &table).unwrap_err();

        assert!(!report.accepted);
        assert!(
            report.diagnostics[0]
                .message
                .contains("missing required key source")
        );
    }

    #[test]
    fn representative_shapes_cover_runtime_config_libs() {
        let libs = representative_config_shapes()
            .into_iter()
            .map(|shape| shape.lib)
            .collect::<Vec<_>>();

        assert_eq!(
            libs,
            vec![
                cookbook_lib_symbol(),
                stream_host_lib_symbol(),
                model_defaults_lib_symbol()
            ]
        );
    }

    #[test]
    fn stream_and_model_shapes_validate_probe_style_tables() {
        let mut cx = cx();
        let stream = ConfigTable::new(
            stream_host_lib_symbol(),
            map(vec![
                ("audio_backend_candidates", string_list(["modeled"])),
                ("midi_backend_candidates", string_list(["modeled"])),
                ("sample_rate_hz", int(48_000)),
            ]),
        )
        .unwrap();
        let model = ConfigTable::new(
            model_defaults_lib_symbol(),
            map(vec![
                ("model_regex", text(r"^(?:fixture/|sim/).*")),
                ("provider_regex", text("^(?:modeled)$")),
                ("prefer_local", Expr::Bool(true)),
            ]),
        )
        .unwrap();

        stream_host_config_shape()
            .validate(&mut cx, &stream)
            .unwrap();
        model_defaults_config_shape()
            .validate(&mut cx, &model)
            .unwrap();
    }
}
