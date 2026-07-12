//! Report model for effective config provenance.

use sim_kernel::Symbol;

use crate::{ConfigSource, EffectiveConfig};

/// User-facing summary of field provenance in an effective config.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigReport {
    /// Provenance entries, one per effective top-level field.
    pub entries: Vec<ConfigReportEntry>,
}

impl ConfigReport {
    /// Builds a report from an effective config.
    pub fn from_effective(effective: &EffectiveConfig) -> Self {
        Self {
            entries: effective
                .trace
                .iter()
                .map(|trace| ConfigReportEntry {
                    lib: trace.lib.clone(),
                    key: trace.key.clone(),
                    source: source_label(&trace.source),
                })
                .collect(),
        }
    }
}

/// One field's effective source label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigReportEntry {
    /// Library table that contains the field.
    pub lib: Symbol,
    /// Top-level field key.
    pub key: String,
    /// Human-readable source label.
    pub source: String,
}

fn source_label(source: &ConfigSource) -> String {
    match source {
        ConfigSource::BuiltIn { lib } => format!("built-in:{}", lib.as_qualified_str()),
        ConfigSource::Probe { probe, mode } => {
            format!("probe:{}:{mode:?}", probe.as_qualified_str())
        }
        ConfigSource::HomeFile { path } => format!("home-file:{}", path.display()),
        ConfigSource::WorkFile { path } => format!("work-file:{}", path.display()),
        ConfigSource::SingleFile { path } => format!("single-file:{}", path.display()),
        ConfigSource::Site { site } => format!("site:{}", site.as_qualified_str()),
        ConfigSource::Explicit { label } => format!("explicit:{label}"),
    }
}

#[cfg(test)]
mod tests {
    use sim_kernel::Symbol;
    use sim_value::build::{map, text};

    use crate::{ConfigDir, ConfigLayer, merge_layers};

    use super::*;

    #[test]
    fn report_projects_trace_entries() {
        let lib = Symbol::qualified("sim", "cookbook");
        let dir = ConfigDir::one(lib.clone(), map(vec![("mode", text("built-in"))])).unwrap();
        let effective = merge_layers(&[ConfigLayer::new(
            ConfigSource::BuiltIn { lib: lib.clone() },
            dir,
        )]);

        let report = ConfigReport::from_effective(&effective);

        assert_eq!(
            report.entries,
            vec![ConfigReportEntry {
                lib,
                key: "mode".to_owned(),
                source: "built-in:sim/cookbook".to_owned()
            }]
        );
    }
}
