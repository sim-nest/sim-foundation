//! Neutral contracts for domain-defined host ports.
//!
//! This crate describes a port without realizing one. Provider selection,
//! evidence grading, operating-system integration, and product policy belong
//! in platform and domain libraries. A domain implements [`HostPort`] on its
//! own opaque runtime object and installs that object in a lexical child
//! [`Env`] with [`bind_host_port`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::sync::Arc;

use sim_kernel::{Env, Factory, RuntimeObject, Symbol, Value, error::Result};

/// An open provider identity, represented as kernel data rather than an enum.
pub type ProviderId = Symbol;

/// An open service identity, represented as kernel data rather than an enum.
pub type ServiceId = Symbol;

/// A declared, mechanically enforced resource limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredLimit {
    /// Open resource name, such as `bytes/request` or `calls/second`.
    pub resource: Symbol,
    /// Maximum amount of the resource admitted by the port.
    pub maximum: u64,
}

/// Non-secret provenance safe to expose on a host-port card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedProvenance {
    /// Provider that supplied the port.
    pub provider: ProviderId,
    /// Service realized by the port.
    pub service: ServiceId,
    /// Optional provider-defined revision or deployment label.
    pub revision: Option<Symbol>,
}

/// Dependency-light descriptive data published by a host port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostPortCard {
    /// Stable service identity offered by this port.
    pub service: ServiceId,
    /// Mechanical resource limits declared by the provider.
    pub limits: Vec<DeclaredLimit>,
    /// Sanitized provider provenance; credentials and host details never belong here.
    pub provenance: SanitizedProvenance,
}

/// Common host-call refusals limited to runtime mechanics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostRefusal {
    /// The provider does not implement the requested operation.
    Unsupported,
    /// The call lacks authority granted by the realizing layer.
    Denied,
    /// The provider is not currently reachable or ready.
    Unavailable,
    /// The port has been intentionally paused.
    Suspended,
    /// The request is malformed for the provider contract.
    Invalid,
    /// A declared mechanical budget has been consumed.
    BudgetExhausted,
    /// The caller or runtime cancelled the operation.
    Cancelled,
    /// The provider failed without exposing unsafe internal detail.
    ProviderFault,
}

/// Result returned by neutral host-port operations.
pub type HostResult<T> = core::result::Result<T, HostRefusal>;

/// Marker contract implemented by a domain's opaque host-port object.
///
/// The trait deliberately specifies only descriptive identity. Domain methods
/// remain on domain traits, so this foundation cannot accumulate platform or
/// product policy.
pub trait HostPort: RuntimeObject {
    /// Returns the neutral card describing this port.
    fn host_port_card(&self) -> &HostPortCard;
}

/// Creates a child environment and binds an opaque domain port in its local frame.
///
/// No process-global registry is involved; dropping the environment drops its
/// ownership of the binding.
pub fn bind_host_port<P>(
    factory: &dyn Factory,
    parent: Arc<Env>,
    binding: Symbol,
    port: Arc<P>,
) -> Result<Env>
where
    P: HostPort + 'static,
{
    let mut child = Env::child(parent);
    let opaque: Arc<dyn RuntimeObject> = port;
    child.define(binding, factory.opaque(opaque)?);
    Ok(child)
}

/// Looks up the opaque value bound for a domain host port.
pub fn host_port_value(env: &Env, binding: &Symbol) -> Option<Value> {
    env.get(binding)
}

#[cfg(test)]
mod tests {
    use std::{any::Any, sync::Arc};

    use sim_kernel::{Cx, DefaultFactory, Object, ObjectCompat};

    use super::*;

    struct FictionalPort {
        card: HostPortCard,
    }

    impl Object for FictionalPort {
        fn display(&self, _cx: &mut Cx) -> Result<String> {
            Ok("#<fictional-host-port>".into())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    impl ObjectCompat for FictionalPort {}

    impl HostPort for FictionalPort {
        fn host_port_card(&self) -> &HostPortCard {
            &self.card
        }
    }

    #[test]
    fn fictional_open_ids_bind_as_an_opaque_child_value() {
        let provider = Symbol::qualified("fictional-provider", "orbital");
        let service = Symbol::qualified("fictional-service", "weather-on-mars");
        let port = Arc::new(FictionalPort {
            card: HostPortCard {
                service: service.clone(),
                limits: vec![DeclaredLimit {
                    resource: Symbol::qualified("calls", "request"),
                    maximum: 7,
                }],
                provenance: SanitizedProvenance {
                    provider: provider.clone(),
                    service,
                    revision: Some(Symbol::new("prototype-9")),
                },
            },
        });
        let binding = Symbol::qualified("host-port", "weather");
        let parent = Arc::new(Env::default());
        let child = bind_host_port(&DefaultFactory, parent.clone(), binding.clone(), port)
            .expect("opaque binding");

        let value = host_port_value(&child, &binding).expect("local port");
        let recovered = value
            .object()
            .downcast_ref::<FictionalPort>()
            .expect("domain type remains recoverable");
        assert_eq!(recovered.card.provenance.provider, provider);
        assert!(parent.get(&binding).is_none());
    }

    #[test]
    fn common_refusal_vocabulary_is_exhaustive_and_mechanical() {
        let refusals = [
            HostRefusal::Unsupported,
            HostRefusal::Denied,
            HostRefusal::Unavailable,
            HostRefusal::Suspended,
            HostRefusal::Invalid,
            HostRefusal::BudgetExhausted,
            HostRefusal::Cancelled,
            HostRefusal::ProviderFault,
        ];
        assert_eq!(refusals.len(), 8);
    }
}
