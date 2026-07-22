//! Layer merging and field-level provenance.

use sim_kernel::{Expr, Symbol};

use crate::{ConfigDir, ConfigSource, ConfigTable, config_field_name, same_config_field};

/// One configuration layer and the source that produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigLayer {
    /// Source metadata for this layer.
    pub source: ConfigSource,
    /// Dir data carried by this layer.
    pub dir: ConfigDir,
}

impl ConfigLayer {
    /// Creates a new layer from source metadata and Dir data.
    pub fn new(source: ConfigSource, dir: ConfigDir) -> Self {
        Self { source, dir }
    }
}

/// Provenance for one effective top-level field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeTrace {
    /// Library table that contains the field.
    pub lib: Symbol,
    /// Top-level field key.
    pub key: String,
    /// Source that supplied the effective value.
    pub source: ConfigSource,
}

/// Result of merging config layers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffectiveConfig {
    /// Effective Dir after layer precedence is applied.
    pub dir: ConfigDir,
    /// Field-level provenance for effective top-level fields.
    pub trace: Vec<MergeTrace>,
}

impl EffectiveConfig {
    fn record_trace(&mut self, lib: &Symbol, key: String, source: &ConfigSource) {
        if let Some(trace) = self
            .trace
            .iter_mut()
            .find(|trace| &trace.lib == lib && trace.key == key)
        {
            trace.source = source.clone();
        } else {
            self.trace.push(MergeTrace {
                lib: lib.clone(),
                key,
                source: source.clone(),
            });
        }
    }
}

/// Merges config layers from lowest to highest precedence.
pub fn merge_layers(layers: &[ConfigLayer]) -> EffectiveConfig {
    let mut effective = EffectiveConfig::default();
    for layer in layers {
        for table in &layer.dir.entries {
            overlay_table(&mut effective, table, &layer.source);
        }
    }
    effective
}

fn overlay_table(effective: &mut EffectiveConfig, table: &ConfigTable, source: &ConfigSource) {
    let Some(index) = effective
        .dir
        .entries
        .iter()
        .position(|entry| entry.lib == table.lib)
    else {
        effective.dir.entries.push(table.clone());
        for key in top_level_keys(&table.table) {
            effective.record_trace(&table.lib, key, source);
        }
        return;
    };

    let changed_keys = merge_table_expr(&mut effective.dir.entries[index].table, &table.table);
    for key in changed_keys {
        effective.record_trace(&table.lib, key, source);
    }
}

fn merge_table_expr(base: &mut Expr, overlay: &Expr) -> Vec<String> {
    match (base, overlay) {
        (Expr::Map(base_entries), Expr::Map(overlay_entries)) => {
            let mut changed = Vec::new();
            for (overlay_key, overlay_value) in overlay_entries {
                let key = key_label(overlay_key);
                if let Some((_, base_value)) = base_entries
                    .iter_mut()
                    .find(|(base_key, _)| same_config_field(base_key, overlay_key))
                {
                    *base_value = merge_value(base_value, overlay_value);
                } else {
                    base_entries.push((overlay_key.clone(), overlay_value.clone()));
                }
                changed.push(key);
            }
            changed
        }
        (base, _) => {
            *base = overlay.clone();
            top_level_keys(overlay)
        }
    }
}

fn merge_value(base: &Expr, overlay: &Expr) -> Expr {
    match (base, overlay) {
        (Expr::Map(_), Expr::Map(_)) => {
            let mut merged = base.clone();
            merge_table_expr(&mut merged, overlay);
            merged
        }
        (Expr::List(base_items), Expr::List(overlay_items))
            if id_keyed_items(base_items) && id_keyed_items(overlay_items) =>
        {
            merge_id_keyed_items(base_items, overlay_items)
        }
        _ => overlay.clone(),
    }
}

fn merge_id_keyed_items(base_items: &[Expr], overlay_items: &[Expr]) -> Expr {
    let mut merged = base_items.to_vec();
    for overlay in overlay_items {
        let Some(overlay_id) = item_id(overlay) else {
            continue;
        };
        if let Some(slot) = merged
            .iter_mut()
            .find(|item| item_id(item).as_deref() == Some(overlay_id.as_str()))
        {
            *slot = overlay.clone();
        } else {
            merged.push(overlay.clone());
        }
    }
    Expr::List(merged)
}

fn id_keyed_items(items: &[Expr]) -> bool {
    !items.is_empty() && items.iter().all(|item| item_id(item).is_some())
}

fn item_id(item: &Expr) -> Option<String> {
    match sim_value::access::field_any(item, "id") {
        Some(Expr::String(id)) => Some(id.clone()),
        Some(Expr::Symbol(id)) => Some(id.as_qualified_str()),
        _ => None,
    }
}

fn top_level_keys(table: &Expr) -> Vec<String> {
    match table {
        Expr::Map(entries) => entries.iter().map(|(key, _)| key_label(key)).collect(),
        _ => Vec::new(),
    }
}

fn key_label(key: &Expr) -> String {
    config_field_name(key)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{key:?}"))
}

#[cfg(test)]
mod tests {
    use sim_value::access::field_any;
    use sim_value::build::{entry, int, list, map, sym, text};

    use super::*;
    use crate::ConfigView;

    fn lib() -> Symbol {
        Symbol::qualified("sim", "cookbook")
    }

    #[test]
    fn higher_layers_replace_scalars_and_preserve_absent_fields() {
        let lower = ConfigDir::one(
            lib(),
            map(vec![("mode", text("built-in")), ("keep", int(1))]),
        )
        .unwrap();
        let upper = ConfigDir::one(lib(), map(vec![("mode", text("work"))])).unwrap();

        let effective = merge_layers(&[
            ConfigLayer::new(ConfigSource::BuiltIn { lib: lib() }, lower),
            ConfigLayer::new(
                ConfigSource::Explicit {
                    label: "work".to_owned(),
                },
                upper,
            ),
        ]);

        let table = effective.dir.table(&lib()).unwrap();
        assert_eq!(field_any(&table.table, "mode"), Some(&text("work")));
        assert_eq!(field_any(&table.table, "keep"), Some(&int(1)));
        assert_eq!(
            effective
                .trace
                .iter()
                .find(|trace| trace.key == "mode")
                .unwrap()
                .source,
            ConfigSource::Explicit {
                label: "work".to_owned()
            }
        );
    }

    #[test]
    fn equivalent_scalar_keys_replace_in_both_directions() {
        let lower = ConfigDir::one(
            lib(),
            Expr::Map(vec![
                (Expr::String("mode".to_owned()), text("built-in")),
                (sym("keep"), int(1)),
            ]),
        )
        .unwrap();
        let upper = ConfigDir::one(lib(), map(vec![("mode", text("work"))])).unwrap();

        let effective = merge_layers(&[
            ConfigLayer::new(ConfigSource::BuiltIn { lib: lib() }, lower),
            ConfigLayer::new(
                ConfigSource::Explicit {
                    label: "symbol-over-string".to_owned(),
                },
                upper,
            ),
        ]);
        let table = effective.dir.table(&lib()).unwrap();
        let view = ConfigView::new(table);

        assert_eq!(view.string("mode"), Some("work"));
        assert_eq!(view.i64("keep"), Some(1));
        assert_eq!(table.entries().unwrap().len(), 2);
        assert_eq!(
            effective
                .trace
                .iter()
                .find(|trace| trace.key == "mode")
                .unwrap()
                .source,
            ConfigSource::Explicit {
                label: "symbol-over-string".to_owned()
            }
        );

        let lower = ConfigDir::one(lib(), map(vec![("mode", text("built-in"))])).unwrap();
        let upper = ConfigDir::one(
            lib(),
            Expr::Map(vec![(Expr::String("mode".to_owned()), text("work"))]),
        )
        .unwrap();
        let effective = merge_layers(&[
            ConfigLayer::new(ConfigSource::BuiltIn { lib: lib() }, lower),
            ConfigLayer::new(
                ConfigSource::Explicit {
                    label: "string-over-symbol".to_owned(),
                },
                upper,
            ),
        ]);

        assert_eq!(
            ConfigView::new(effective.dir.table(&lib()).unwrap()).string("mode"),
            Some("work")
        );
        assert_eq!(
            effective
                .trace
                .iter()
                .find(|trace| trace.key == "mode")
                .unwrap()
                .source,
            ConfigSource::Explicit {
                label: "string-over-symbol".to_owned()
            }
        );
    }

    #[test]
    fn equivalent_nested_map_keys_merge_without_duplicates() {
        let lower = ConfigDir::one(
            lib(),
            map(vec![(
                "settings",
                Expr::Map(vec![
                    (sym("mode"), text("built-in")),
                    (sym("keep"), text("lower")),
                ]),
            )]),
        )
        .unwrap();
        let upper = ConfigDir::one(
            lib(),
            Expr::Map(vec![(
                Expr::String("settings".to_owned()),
                Expr::Map(vec![(Expr::String("mode".to_owned()), text("work"))]),
            )]),
        )
        .unwrap();

        let effective = merge_layers(&[
            ConfigLayer::new(ConfigSource::BuiltIn { lib: lib() }, lower),
            ConfigLayer::new(
                ConfigSource::Explicit {
                    label: "work".to_owned(),
                },
                upper,
            ),
        ]);
        let view = ConfigView::new(effective.dir.table(&lib()).unwrap());
        let settings = ConfigView::from_entries(view.table("settings").unwrap());

        assert_eq!(settings.string("mode"), Some("work"));
        assert_eq!(settings.string("keep"), Some("lower"));
        assert_eq!(view.table("settings").unwrap().len(), 2);
        assert_eq!(
            effective
                .trace
                .iter()
                .find(|trace| trace.key == "settings")
                .unwrap()
                .source,
            ConfigSource::Explicit {
                label: "work".to_owned()
            }
        );
    }

    #[test]
    fn id_keyed_repeated_tables_replace_by_id() {
        let lower = ConfigDir::one(
            lib(),
            map(vec![(
                "loadable_lib",
                Expr::List(vec![
                    map(vec![("id", text("numbers")), ("source", text("stable"))]),
                    map(vec![("id", text("shape")), ("source", text("stable"))]),
                ]),
            )]),
        )
        .unwrap();
        let upper = ConfigDir::one(
            lib(),
            map(vec![(
                "loadable_lib",
                Expr::List(vec![
                    map(vec![("id", text("shape")), ("source", text("work"))]),
                    map(vec![("id", text("music")), ("source", text("work"))]),
                ]),
            )]),
        )
        .unwrap();

        let effective = merge_layers(&[
            ConfigLayer::new(ConfigSource::BuiltIn { lib: lib() }, lower),
            ConfigLayer::new(
                ConfigSource::Explicit {
                    label: "work".to_owned(),
                },
                upper,
            ),
        ]);

        let list = match field_any(&effective.dir.table(&lib()).unwrap().table, "loadable_lib") {
            Some(Expr::List(items)) => items,
            other => panic!("expected merged list, got {other:?}"),
        };
        assert_eq!(list.len(), 3);
        assert_eq!(field_any(&list[1], "source"), Some(&text("work")));
        assert_eq!(field_any(&list[2], "id"), Some(&text("music")));
        let _ = entry("checked", Expr::Bool(true));
    }

    #[test]
    fn id_keyed_repeated_tables_use_config_field_identity() {
        let lower = ConfigDir::one(
            lib(),
            Expr::Map(vec![(
                Expr::String("loadable_lib".to_owned()),
                list(vec![
                    Expr::Map(vec![
                        (Expr::String("id".to_owned()), text("shape")),
                        (sym("source"), text("stable")),
                    ]),
                    map(vec![("id", text("numbers")), ("source", text("stable"))]),
                ]),
            )]),
        )
        .unwrap();
        let upper = ConfigDir::one(
            lib(),
            map(vec![(
                "loadable_lib",
                list(vec![Expr::Map(vec![
                    (sym("id"), text("shape")),
                    (Expr::String("source".to_owned()), text("work")),
                ])]),
            )]),
        )
        .unwrap();

        let effective = merge_layers(&[
            ConfigLayer::new(ConfigSource::BuiltIn { lib: lib() }, lower),
            ConfigLayer::new(
                ConfigSource::Explicit {
                    label: "work".to_owned(),
                },
                upper,
            ),
        ]);

        let list = match ConfigView::new(effective.dir.table(&lib()).unwrap()).get("loadable_lib") {
            Some(Expr::List(items)) => items,
            other => panic!("expected merged list, got {other:?}"),
        };
        assert_eq!(list.len(), 2);
        assert_eq!(field_any(&list[0], "id"), Some(&text("shape")));
        assert_eq!(field_any(&list[0], "source"), Some(&text("work")));
        assert_eq!(field_any(&list[1], "id"), Some(&text("numbers")));
        assert_eq!(
            effective
                .trace
                .iter()
                .find(|trace| trace.key == "loadable_lib")
                .unwrap()
                .source,
            ConfigSource::Explicit {
                label: "work".to_owned()
            }
        );
    }
}
