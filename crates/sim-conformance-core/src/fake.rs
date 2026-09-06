//! Deterministic fake ports for independently authored conformance cases.

use std::collections::{BTreeMap, VecDeque};

use crate::{ConformanceError, StorageId};

/// Explicit clock read by a pack instead of ambient time.
pub trait CheckClock {
    /// Returns the next monotonic tick.
    fn now(&mut self) -> u64;
}

/// Deterministic manually advanced clock.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FakeClock {
    now: u64,
}

impl FakeClock {
    /// Creates a clock at an explicit tick.
    pub const fn new(now: u64) -> Self {
        Self { now }
    }

    /// Advances without consulting host time.
    pub fn advance(&mut self, ticks: u64) -> Result<(), ConformanceError> {
        self.now = self
            .now
            .checked_add(ticks)
            .ok_or(ConformanceError::BoundExceeded("clock"))?;
        Ok(())
    }
}

impl CheckClock for FakeClock {
    fn now(&mut self) -> u64 {
        self.now
    }
}

/// Read-only byte materialization port used by pure packet and pack tests.
pub trait InputPort {
    /// Loads the exact bytes at a declared location.
    fn read(&mut self, location: &StorageId) -> Result<Vec<u8>, ConformanceError>;
}

/// In-memory exact-byte port with deterministic request accounting.
#[derive(Clone, Debug, Default)]
pub struct FakeInputPort {
    objects: BTreeMap<StorageId, Vec<u8>>,
    reads: Vec<StorageId>,
}

impl FakeInputPort {
    /// Stores bytes at their computed location and returns that location.
    pub fn insert(&mut self, bytes: Vec<u8>) -> StorageId {
        let id = StorageId::for_bytes(&bytes);
        self.objects.insert(id.clone(), bytes);
        id
    }

    /// Returns the exact observed read sequence.
    pub fn reads(&self) -> &[StorageId] {
        &self.reads
    }
}

impl InputPort for FakeInputPort {
    fn read(&mut self, location: &StorageId) -> Result<Vec<u8>, ConformanceError> {
        self.reads.push(location.clone());
        self.objects
            .get(location)
            .cloned()
            .ok_or_else(|| ConformanceError::MissingSurface("stored input".into()))
    }
}

/// Fake effect boundary used to prove a pure checker cannot perform real work.
pub trait CheckEffectPort {
    /// Records a declared operation and returns the next predetermined result.
    fn invoke(&mut self, operation: &str) -> Result<String, ConformanceError>;
}

/// Queue-backed fake effect port with no operating-system client.
#[derive(Clone, Debug, Default)]
pub struct FakeEffectPort {
    results: VecDeque<Result<String, ConformanceError>>,
    calls: Vec<String>,
}

impl FakeEffectPort {
    /// Appends a predetermined result.
    pub fn push(&mut self, result: Result<String, ConformanceError>) {
        self.results.push_back(result);
    }

    /// Returns every requested operation in order.
    pub fn calls(&self) -> &[String] {
        &self.calls
    }
}

impl CheckEffectPort for FakeEffectPort {
    fn invoke(&mut self, operation: &str) -> Result<String, ConformanceError> {
        self.calls.push(operation.to_owned());
        self.results.pop_front().unwrap_or_else(|| {
            Err(ConformanceError::MissingSurface(
                "fake effect result".into(),
            ))
        })
    }
}
