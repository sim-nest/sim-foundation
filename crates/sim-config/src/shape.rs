//! Shape-backed configuration defaults and validation.

use std::sync::Arc;

use sim_kernel::{
    Cx, Datum, Diagnostic, MatchScore, Ref, Shape, ShapeMatch, Symbol,
    datum_store::DatumStore,
    shape_report::{ShapeReport, shape_report_from_match},
};

use crate::{ConfigDir, ConfigLayer, ConfigSource, ConfigTable, merge_layers};

/// Result returned by config shape validation.
pub type ConfigShapeResult<T> = Result<T, Box<ShapeReport>>;

/// Shape-backed contract for one library's configuration table.
#[derive(Clone)]
pub struct ConfigShape {
    /// Library id whose configuration this shape validates.
    pub lib: Symbol,
    /// Shape applied to the effective table after built-in defaults are bound.
    pub shape: Arc<dyn Shape>,
    /// Built-in defaults for absent fields.
    pub defaults: ConfigTable,
    /// Top-level keys whose values are secret-bearing.
    pub secret_keys: Vec<String>,
}

impl ConfigShape {
    /// Creates a config shape with no secret keys.
    pub fn new(lib: Symbol, shape: Arc<dyn Shape>, defaults: ConfigTable) -> Self {
        Self {
            lib,
            shape,
            defaults,
            secret_keys: Vec::new(),
        }
    }

    /// Adds secret keys, deduplicating them at the shape boundary.
    pub fn with_secret_keys(mut self, keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.secret_keys = dedupe_keys(keys);
        self
    }

    /// Validates `table` against this shape and returns the default-bound table.
    ///
    /// Built-in defaults are the lowest-precedence layer. The supplied table
    /// overlays them, then the configured shape checks the effective table. A
    /// closed table shape therefore reports unknown fields while defaults may
    /// satisfy required fields.
    pub fn validate(&self, cx: &mut Cx, table: &ConfigTable) -> ConfigShapeResult<ConfigTable> {
        if table.lib != self.lib {
            return Err(Box::new(self.reject_report(
                cx,
                table,
                format!(
                    "config table for `{}` cannot validate as `{}`",
                    table.lib.as_qualified_str(),
                    self.lib.as_qualified_str()
                ),
            )));
        }
        if self.defaults.lib != self.lib {
            return Err(Box::new(self.reject_report(
                cx,
                table,
                format!(
                    "config defaults for `{}` do not match shape lib `{}`",
                    self.defaults.lib.as_qualified_str(),
                    self.lib.as_qualified_str()
                ),
            )));
        }

        let effective = self.bind_defaults(table);
        let matched = self
            .shape
            .check_expr(cx, &effective.table)
            .unwrap_or_else(|error| {
                ShapeMatch::reject(format!("config shape check failed: {error}"))
            });
        if matched.accepted {
            Ok(effective)
        } else {
            Err(Box::new(self.report_from_match(cx, &effective, matched)))
        }
    }

    /// Returns this shape's built-in defaults as a merge layer.
    pub fn defaults_layer(&self) -> ConfigLayer {
        ConfigLayer::new(
            ConfigSource::BuiltIn {
                lib: self.lib.clone(),
            },
            ConfigDir {
                entries: vec![self.defaults.clone()],
            },
        )
    }

    /// Returns true when `key` is marked as secret-bearing by this shape.
    pub fn marks_secret(&self, key: &str) -> bool {
        self.secret_keys.iter().any(|secret| secret == key)
    }

    /// Returns secret field records for report and redaction code.
    pub fn secret_fields(&self) -> Vec<ConfigSecretField> {
        dedupe_keys(self.secret_keys.iter().cloned())
            .into_iter()
            .map(|key| ConfigSecretField {
                lib: self.lib.clone(),
                key,
            })
            .collect()
    }

    fn bind_defaults(&self, table: &ConfigTable) -> ConfigTable {
        let explicit = ConfigLayer::new(
            ConfigSource::Explicit {
                label: "shape-validated-input".to_owned(),
            },
            ConfigDir {
                entries: vec![table.clone()],
            },
        );
        let effective = merge_layers(&[self.defaults_layer(), explicit]);
        effective
            .dir
            .table(&self.lib)
            .cloned()
            .unwrap_or_else(|| table.clone())
    }

    fn report_from_match(
        &self,
        cx: &mut Cx,
        table: &ConfigTable,
        matched: ShapeMatch,
    ) -> ShapeReport {
        let shape_ref = self.shape_ref();
        let target_ref = self.target_ref(cx, table);
        shape_report_from_match(cx, shape_ref.clone(), target_ref.clone(), matched.clone())
            .unwrap_or_else(|error| {
                fallback_report(
                    shape_ref,
                    target_ref,
                    format!("config shape report failed: {error}"),
                )
            })
    }

    fn reject_report(
        &self,
        cx: &mut Cx,
        table: &ConfigTable,
        message: impl Into<String>,
    ) -> ShapeReport {
        self.report_from_match(cx, table, ShapeMatch::reject(message))
    }

    fn shape_ref(&self) -> Ref {
        self.shape.symbol().map(Ref::Symbol).unwrap_or_else(|| {
            Ref::Symbol(Symbol::qualified(
                "config-shape",
                self.lib.as_qualified_str(),
            ))
        })
    }

    fn target_ref(&self, cx: &mut Cx, table: &ConfigTable) -> Ref {
        Datum::try_from(table.table.clone())
            .and_then(|datum| cx.datum_store_mut().intern(datum).map(Ref::Content))
            .unwrap_or_else(|_| {
                Ref::Symbol(Symbol::qualified(
                    "config-target",
                    table.lib.as_qualified_str(),
                ))
            })
    }
}

/// Secret-bearing config field metadata exported by a [`ConfigShape`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSecretField {
    /// Library whose table contains the secret field.
    pub lib: Symbol,
    /// Top-level field key whose value must be redacted by report surfaces.
    pub key: String,
}

/// Default configuration contract exposed by a loadable library.
pub trait LibConfigDefaults {
    /// Returns the shape-backed config contract for this library.
    fn config_shape(&self) -> ConfigShape;

    /// Returns the built-in defaults layer published by this library.
    fn built_in_config_layer(&self) -> ConfigLayer {
        self.config_shape().defaults_layer()
    }
}

fn dedupe_keys(keys: impl IntoIterator<Item = impl Into<String>>) -> Vec<String> {
    let mut deduped = Vec::new();
    for key in keys {
        let key = key.into();
        if !deduped.contains(&key) {
            deduped.push(key);
        }
    }
    deduped
}

fn fallback_report(shape: Ref, target: Ref, message: impl Into<String>) -> ShapeReport {
    ShapeReport {
        id: Ref::Symbol(Symbol::qualified("config-shape", "fallback-report")),
        shape,
        target,
        accepted: false,
        score: MatchScore::reject(),
        captures: Ref::Symbol(Symbol::qualified("config-shape", "empty-captures")),
        diagnostics: vec![Diagnostic::error(message)],
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sim_kernel::{
        Cx, DefaultFactory, Expr, ExprKind, NoopEvalPolicy, Result, ShapeDoc, Value,
        shape::{MatchScore, Shape},
    };
    use sim_value::{access::field, build::map, build::text};

    use super::*;

    fn cx() -> Cx {
        Cx::new(Arc::new(NoopEvalPolicy), Arc::new(DefaultFactory))
    }

    fn lib() -> Symbol {
        Symbol::qualified("model", "defaults")
    }

    fn shape() -> Arc<dyn Shape> {
        Arc::new(ClosedTableShape::new(vec![
            FieldSpec::required("provider", ExprKind::String),
            FieldSpec::required("enabled", ExprKind::Bool),
            FieldSpec::optional("api_key", ExprKind::String),
        ]))
    }

    fn config_shape() -> ConfigShape {
        let lib = lib();
        let defaults = ConfigTable::new(
            lib.clone(),
            map(vec![
                ("provider", text("modeled")),
                ("enabled", Expr::Bool(true)),
            ]),
        )
        .unwrap();
        ConfigShape::new(lib, shape(), defaults).with_secret_keys(["api_key", "api_key"])
    }

    #[test]
    fn validate_binds_built_in_defaults() {
        let mut cx = cx();
        let table = ConfigTable::new(lib(), map(vec![("provider", text("openai"))])).unwrap();

        let effective = config_shape().validate(&mut cx, &table).unwrap();

        assert_eq!(
            field(&effective.table, "provider"),
            Some(&Expr::String("openai".to_owned()))
        );
        assert_eq!(field(&effective.table, "enabled"), Some(&Expr::Bool(true)));
    }

    #[test]
    fn validate_rejects_unknown_fields_with_shape_report() {
        let mut cx = cx();
        let table = ConfigTable::new(
            lib(),
            map(vec![("provider", text("openai")), ("typo", text("bad"))]),
        )
        .unwrap();

        let report = config_shape().validate(&mut cx, &table).unwrap_err();

        assert!(!report.accepted);
        assert!(report.diagnostics[0].message.contains("extra key typo"));
    }

    #[test]
    fn secret_fields_are_shape_metadata_and_deduped() {
        let shape = config_shape().with_secret_keys(["api_key", "token", "api_key"]);

        assert!(shape.marks_secret("api_key"));
        assert_eq!(
            shape.secret_fields(),
            vec![
                ConfigSecretField {
                    lib: lib(),
                    key: "api_key".to_owned()
                },
                ConfigSecretField {
                    lib: lib(),
                    key: "token".to_owned()
                }
            ]
        );
    }

    #[test]
    fn scalar_kind_errors_are_shape_reports() {
        let mut cx = cx();
        let table = ConfigTable::new(
            lib(),
            map(vec![("provider", text("openai")), ("enabled", text("yes"))]),
        )
        .unwrap();

        let report = config_shape().validate(&mut cx, &table).unwrap_err();

        assert!(!report.accepted);
        assert!(
            report.diagnostics[0]
                .message
                .contains("enabled expected bool")
        );
    }

    #[test]
    fn loadable_lib_defaults_publish_built_in_layer() {
        struct DemoLib(ConfigShape);
        impl LibConfigDefaults for DemoLib {
            fn config_shape(&self) -> ConfigShape {
                self.0.clone()
            }
        }

        let layer = DemoLib(config_shape()).built_in_config_layer();

        assert_eq!(layer.source, ConfigSource::BuiltIn { lib: lib() });
        assert!(layer.dir.table(&lib()).is_some());
    }

    #[derive(Clone)]
    struct FieldSpec {
        name: &'static str,
        kind: ExprKind,
        required: bool,
    }

    impl FieldSpec {
        fn required(name: &'static str, kind: ExprKind) -> Self {
            Self {
                name,
                kind,
                required: true,
            }
        }

        fn optional(name: &'static str, kind: ExprKind) -> Self {
            Self {
                name,
                kind,
                required: false,
            }
        }
    }

    struct ClosedTableShape {
        fields: Vec<FieldSpec>,
    }

    impl ClosedTableShape {
        fn new(fields: Vec<FieldSpec>) -> Self {
            Self { fields }
        }

        fn field(&self, name: &str) -> Option<&FieldSpec> {
            self.fields.iter().find(|field| field.name == name)
        }
    }

    impl Shape for ClosedTableShape {
        fn symbol(&self) -> Option<Symbol> {
            Some(Symbol::qualified("test", "closed-config-table"))
        }

        fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
            let expr = value.object().as_expr(cx)?;
            self.check_expr(cx, &expr)
        }

        fn check_expr(&self, _cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
            let Expr::Map(entries) = expr else {
                return Ok(ShapeMatch::reject("config table expected map"));
            };

            for (key, value) in entries {
                let Some(key) = key_name(key) else {
                    return Ok(ShapeMatch::reject("config table key expected symbol"));
                };
                let Some(field) = self.field(key) else {
                    return Ok(ShapeMatch::reject(format!("shape-table: extra key {key}")));
                };
                if !field.kind.matches(value) {
                    return Ok(ShapeMatch::reject(format!(
                        "{key} expected {}",
                        field.kind.name()
                    )));
                }
            }

            for field in self.fields.iter().filter(|field| field.required) {
                if !entries
                    .iter()
                    .any(|(key, _)| key_name(key).is_some_and(|key| key == field.name))
                {
                    return Ok(ShapeMatch::reject(format!(
                        "missing required config key {}",
                        field.name
                    )));
                }
            }

            Ok(ShapeMatch::accept(MatchScore::exact(entries.len() as i32)))
        }

        fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
            Ok(ShapeDoc::new("closed config table"))
        }
    }

    fn key_name(key: &Expr) -> Option<&str> {
        match key {
            Expr::Symbol(symbol) if symbol.namespace.is_none() => Some(symbol.name.as_ref()),
            Expr::String(text) => Some(text),
            _ => None,
        }
    }
}
