//! Immutable ownership declarations and separately scoped qualification.

use std::collections::BTreeSet;

use sim_kernel::{Datum, Symbol};

use crate::{
    CommandId, ConformanceError, DependencyUseSetId, OwnerBindingId, SemanticId, field, qualified,
    strings, text,
};

/// Stable declaration key for a public or planned surface.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceKey(String);

impl SurfaceKey {
    /// Creates a nonempty bounded key.
    pub fn new(value: impl Into<String>) -> Result<Self, ConformanceError> {
        let value = value.into();
        if value.is_empty() || value.len() > 4_096 || value.chars().any(char::is_control) {
            return Err(ConformanceError::BoundExceeded("surface key"));
        }
        Ok(Self(value))
    }

    /// Returns the declaration spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One exact repository command declared by an owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerCommand {
    /// Semantic command identity.
    pub id: CommandId,
    /// Repository-relative working directory policy.
    pub cwd: String,
    /// Exact argument vector; no shell reinterpretation is implied.
    pub argv: Vec<String>,
    /// Environment policy identity.
    pub environment: String,
}

/// Whether activation extends an owner, extracts a reusable owner, or adds a product.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerDisposition {
    /// Add behavior to the existing singular owner.
    ExtendExisting,
    /// Introduce a lower reusable owner with independently named consumers.
    ExtractReusable,
    /// Add a separately loadable leaf product.
    AddLoadableProduct,
}

impl OwnerDisposition {
    fn datum(self) -> Datum {
        Datum::Symbol(Symbol::qualified(
            "conformance",
            match self {
                Self::ExtendExisting => "extend-existing",
                Self::ExtractReusable => "extract-reusable",
                Self::AddLoadableProduct => "add-loadable-product",
            },
        ))
    }
}

/// Existing or planned status of a bound surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurfaceStatus {
    /// Exact current source and package evidence exists.
    Existing,
    /// The surface is a declaration owned by its producing phase.
    Planned {
        /// Sole phase allowed to produce the surface.
        producing_phase: String,
    },
}

/// One surface declaration in an immutable owner binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundSurface {
    /// Stable declaration key.
    pub key: SurfaceKey,
    /// Public Rust symbol, route, specimen, or command name.
    pub public_name: String,
    /// Current activation status.
    pub status: SurfaceStatus,
}

impl BoundSurface {
    fn datum(&self) -> Result<Datum, ConformanceError> {
        let (status, phase) = match &self.status {
            SurfaceStatus::Existing => ("existing", Datum::Nil),
            SurfaceStatus::Planned { producing_phase } => {
                ("planned", text(producing_phase.clone()))
            }
        };
        Ok(Datum::Node {
            tag: qualified("conformance/bound-surface-v1")?,
            fields: vec![
                field("key", text(self.key.0.clone()))?,
                field("public-name", text(self.public_name.clone()))?,
                field("status", text(status))?,
                field("producing-phase", phase)?,
            ],
        })
    }
}

/// Complete immutable design declaration for one responsibility.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerBinding {
    id: OwnerBindingId,
    /// Stable law name.
    law: String,
    /// Structural disposition.
    disposition: OwnerDisposition,
    /// Owning public package names.
    owners: Vec<String>,
    /// Declared public surfaces.
    surfaces: Vec<BoundSurface>,
    /// Independent consumers used to justify extraction or integration.
    consumers: Vec<String>,
    /// Checked downward dependency statement.
    dependency_direction: String,
    /// Index route declaration.
    route: SurfaceKey,
    /// Conformance specimen declaration.
    specimen: SurfaceKey,
    /// Exact owner validation command.
    validation: OwnerCommand,
    /// Exact owner docs command.
    docs: OwnerCommand,
}

impl OwnerBinding {
    /// Validates and identifies an immutable binding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        law: String,
        disposition: OwnerDisposition,
        owners: Vec<String>,
        surfaces: Vec<BoundSurface>,
        consumers: Vec<String>,
        dependency_direction: String,
        route: SurfaceKey,
        specimen: SurfaceKey,
        validation: OwnerCommand,
        docs: OwnerCommand,
    ) -> Result<Self, ConformanceError> {
        if law.is_empty()
            || owners.is_empty()
            || surfaces.is_empty()
            || dependency_direction.is_empty()
        {
            return Err(ConformanceError::UnresolvedBinding);
        }
        unique(owners.iter().map(String::as_str), "owner")?;
        unique(surfaces.iter().map(|item| item.key.as_str()), "surface")?;
        unique(consumers.iter().map(String::as_str), "consumer")?;
        if disposition == OwnerDisposition::ExtractReusable && consumers.len() < 2 {
            return Err(ConformanceError::UnresolvedBinding);
        }
        let fields = vec![
            field("law", text(law.clone()))?,
            field("disposition", disposition.datum())?,
            field("owners", strings(owners.clone()))?,
            field(
                "surfaces",
                Datum::Vector(
                    surfaces
                        .iter()
                        .map(BoundSurface::datum)
                        .collect::<Result<_, _>>()?,
                ),
            )?,
            field("consumers", strings(consumers.clone()))?,
            field("dependency-direction", text(dependency_direction.clone()))?,
            field("route", text(route.0.clone()))?,
            field("specimen", text(specimen.0.clone()))?,
            field("validation", validation.id.to_datum())?,
            field("docs", docs.id.to_datum())?,
        ];
        Ok(Self {
            id: SemanticId::from_fields(fields)?,
            law,
            disposition,
            owners,
            surfaces,
            consumers,
            dependency_direction,
            route,
            specimen,
            validation,
            docs,
        })
    }

    /// Returns the stable binding identity.
    pub const fn id(&self) -> &OwnerBindingId {
        &self.id
    }

    /// Returns the stable law name.
    pub fn law(&self) -> &str {
        &self.law
    }

    /// Returns the structural disposition.
    pub const fn disposition(&self) -> OwnerDisposition {
        self.disposition
    }

    /// Returns the owning public package names.
    pub fn owners(&self) -> &[String] {
        &self.owners
    }

    /// Returns every declared surface.
    pub fn surfaces(&self) -> &[BoundSurface] {
        &self.surfaces
    }

    /// Returns the independently named consumers.
    pub fn consumers(&self) -> &[String] {
        &self.consumers
    }

    /// Returns the checked dependency direction.
    pub fn dependency_direction(&self) -> &str {
        &self.dependency_direction
    }

    /// Returns the Index route declaration.
    pub const fn route(&self) -> &SurfaceKey {
        &self.route
    }

    /// Returns the conformance specimen declaration.
    pub const fn specimen(&self) -> &SurfaceKey {
        &self.specimen
    }

    /// Returns the exact validation command.
    pub const fn validation(&self) -> &OwnerCommand {
        &self.validation
    }

    /// Returns the exact documentation command.
    pub const fn docs(&self) -> &OwnerCommand {
        &self.docs
    }

    /// Looks up a declared surface without treating Planned as implemented.
    pub fn surface(&self, key: &SurfaceKey) -> Option<&BoundSurface> {
        self.surfaces.iter().find(|surface| &surface.key == key)
    }
}

/// How a surface participates in one packet or phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceUseRole {
    /// The surface must already be released and qualified.
    ReleasedDependency,
    /// The packet is allowed to produce this still-Planned target.
    FundedTarget,
}

/// One declared surface use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceUse {
    /// Surface declaration key.
    pub surface: SurfaceKey,
    /// Required status at admission.
    pub role: SurfaceUseRole,
}

/// Canonically identified dependency/target declaration made before packet creation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyUseSet {
    id: DependencyUseSetId,
    /// Phase that consumes dependencies and funds targets.
    phase: String,
    /// Binding that owns every funded target.
    owner: OwnerBindingId,
    /// Complete declared uses.
    uses: Vec<SurfaceUse>,
}

impl DependencyUseSet {
    /// Validates unique surface roles and computes semantic identity.
    pub fn new(
        phase: String,
        owner: OwnerBindingId,
        mut uses: Vec<SurfaceUse>,
    ) -> Result<Self, ConformanceError> {
        if phase.is_empty() {
            return Err(ConformanceError::UnresolvedBinding);
        }
        uses.sort_by(|a, b| a.surface.cmp(&b.surface));
        unique(uses.iter().map(|item| item.surface.as_str()), "surface use")?;
        let use_data = uses
            .iter()
            .map(|item| {
                Ok(Datum::Node {
                    tag: qualified("conformance/surface-use-v1")?,
                    fields: vec![
                        field("surface", text(item.surface.0.clone()))?,
                        field(
                            "role",
                            text(match item.role {
                                SurfaceUseRole::ReleasedDependency => "released-dependency",
                                SurfaceUseRole::FundedTarget => "funded-target",
                            }),
                        )?,
                    ],
                })
            })
            .collect::<Result<Vec<_>, ConformanceError>>()?;
        let id = SemanticId::from_fields(vec![
            field("phase", text(phase.clone()))?,
            field("owner", owner.to_datum())?,
            field("uses", Datum::Vector(use_data))?,
        ])?;
        Ok(Self {
            id,
            phase,
            owner,
            uses,
        })
    }

    /// Returns the use-set identity.
    pub const fn id(&self) -> &DependencyUseSetId {
        &self.id
    }

    /// Returns the consuming and producing phase.
    pub fn phase(&self) -> &str {
        &self.phase
    }

    /// Returns the binding that owns every funded target.
    pub const fn owner(&self) -> &OwnerBindingId {
        &self.owner
    }

    /// Returns the complete declared surface uses.
    pub fn uses(&self) -> &[SurfaceUse] {
        &self.uses
    }
}

pub(crate) fn unique<'a>(
    values: impl IntoIterator<Item = &'a str>,
    kind: &'static str,
) -> Result<(), ConformanceError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(ConformanceError::DuplicateField(format!("{kind}:{value}")));
        }
    }
    Ok(())
}
