//! Probe contracts for configuration defaults.

use sim_kernel::Symbol;

use crate::{ConfigLayer, ProbeMode};

/// Capability grants available to a configuration probe.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigProbeCaps {
    /// Permission to inspect environment-variable presence.
    pub env: bool,
    /// Permission to inspect operating-system facts.
    pub os: bool,
    /// Permission to inspect hardware inventory.
    pub hardware_inventory: bool,
    /// Permission to inspect network inventory.
    pub network_inventory: bool,
}

/// Request passed to a configuration probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigProbeRequest {
    /// Library whose defaults are being requested.
    pub lib: Symbol,
    /// Probe mode selected by the host.
    pub mode: ProbeMode,
    /// Capabilities granted to this probe call.
    pub caps: ConfigProbeCaps,
}

/// Outcome reported by a configuration probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigProbeStatus {
    /// A config layer was emitted and applied.
    Applied,
    /// The probe intentionally did not emit a layer.
    Skipped {
        /// Human-readable skip reason.
        reason: String,
    },
    /// The probe was denied a required capability.
    Denied {
        /// Capability name that was required.
        capability: String,
    },
    /// The probe failed without making configuration discovery fatal.
    Failed {
        /// Human-readable failure message.
        message: String,
    },
}

/// Report emitted by one configuration probe call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigProbeReport {
    /// Probe implementation id.
    pub probe: Symbol,
    /// Library whose defaults were requested.
    pub lib: Symbol,
    /// Probe mode used for this call.
    pub mode: ProbeMode,
    /// Probe outcome.
    pub status: ConfigProbeStatus,
    /// Top-level config keys emitted by the probe layer.
    pub emitted_keys: Vec<String>,
}

/// Source of modeled or real configuration defaults.
pub trait ConfigProbe {
    /// Returns the stable probe implementation id.
    fn symbol(&self) -> Symbol;

    /// Runs the probe and returns an optional config layer plus a report.
    fn probe(&self, request: &ConfigProbeRequest) -> (Option<ConfigLayer>, ConfigProbeReport);
}

#[cfg(test)]
mod tests {
    use sim_kernel::Expr;

    use super::*;
    use crate::{ConfigDir, ConfigSource};

    struct FakeProbe {
        symbol: Symbol,
    }

    impl FakeProbe {
        fn new() -> Self {
            Self {
                symbol: Symbol::qualified("config", "fake"),
            }
        }
    }

    impl ConfigProbe for FakeProbe {
        fn symbol(&self) -> Symbol {
            self.symbol.clone()
        }

        fn probe(&self, request: &ConfigProbeRequest) -> (Option<ConfigLayer>, ConfigProbeReport) {
            let emitted_keys = vec!["backend".to_owned()];
            let layer = ConfigLayer::new(
                ConfigSource::Probe {
                    probe: self.symbol(),
                    mode: request.mode,
                },
                ConfigDir::one(
                    request.lib.clone(),
                    Expr::Map(vec![(
                        Expr::Symbol(Symbol::new("backend")),
                        Expr::String("modeled".to_owned()),
                    )]),
                )
                .unwrap(),
            );
            let report = ConfigProbeReport {
                probe: self.symbol(),
                lib: request.lib.clone(),
                mode: request.mode,
                status: ConfigProbeStatus::Applied,
                emitted_keys,
            };
            (Some(layer), report)
        }
    }

    #[test]
    fn probe_mode_defaults_to_modeled() {
        assert_eq!(ProbeMode::default(), ProbeMode::Modeled);
    }

    #[test]
    fn probe_returns_optional_layer_and_typed_report() {
        let probe = FakeProbe::new();
        let request = ConfigProbeRequest {
            lib: Symbol::qualified("stream", "host"),
            mode: ProbeMode::default(),
            caps: ConfigProbeCaps::default(),
        };

        let (layer, report) = probe.probe(&request);

        assert_eq!(report.probe, Symbol::qualified("config", "fake"));
        assert_eq!(report.lib, Symbol::qualified("stream", "host"));
        assert_eq!(report.mode, ProbeMode::Modeled);
        assert_eq!(report.status, ConfigProbeStatus::Applied);
        assert_eq!(report.emitted_keys, ["backend"]);
        assert!(matches!(
            layer.unwrap().source,
            ConfigSource::Probe {
                probe,
                mode: ProbeMode::Modeled
            } if probe == Symbol::qualified("config", "fake")
        ));
    }
}
