use sim_kernel::{Error, Result};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
/// One observed wall-clock instant in Unix milliseconds.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WallTimestamp(u64);
impl WallTimestamp {
    /// Constructs an explicit Unix-millisecond observation.
    pub const fn from_unix_millis(value: u64) -> Self {
        Self(value)
    }
    /// Returns Unix milliseconds.
    pub const fn unix_millis(self) -> u64 {
        self.0
    }
}
/// One process-local monotonic observation in nanoseconds from an injected epoch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MonotonicTimestamp(u64);
impl MonotonicTimestamp {
    /// Constructs an explicit monotonic nanosecond value.
    pub const fn from_nanos(value: u64) -> Self {
        Self(value)
    }
    /// Returns monotonic nanoseconds.
    pub const fn nanos(self) -> u64 {
        self.0
    }
}
/// Object-safe source of optional human wall-time evidence.
pub trait WallClock: Send + Sync {
    /// Observes wall time, which may move backward.
    fn now(&self) -> Result<WallTimestamp>;
    /// Observes Unix milliseconds.
    fn now_ms(&self) -> Result<u64> {
        self.now().map(WallTimestamp::unix_millis)
    }
}
/// Object-safe source of correctness-safe elapsed-time observations.
pub trait MonotonicClock: Send + Sync {
    /// Observes a monotonic timestamp.
    fn now_monotonic(&self) -> Result<MonotonicTimestamp>;
}
/// Executor-neutral platform timer binding.
pub trait Timer: Send + Sync {
    /// Advances or waits until the supplied deadline.
    fn wait_until(&self, deadline: MonotonicTimestamp) -> Result<()>;
}
/// Complete explicitly supplied platform-time binding.
#[derive(Clone)]
pub struct PlatformTime {
    /// Human-facing wall observations.
    pub wall: Arc<dyn WallClock>,
    /// Correctness-safe monotonic observations.
    pub monotonic: Arc<dyn MonotonicClock>,
    /// Timer paired with the monotonic clock.
    pub timer: Arc<dyn Timer>,
}
/// Legacy zero-valued model wall clock retained for source compatibility; performs no host observation.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemWallClock;
impl WallClock for SystemWallClock {
    fn now(&self) -> Result<WallTimestamp> {
        Ok(WallTimestamp::from_unix_millis(0))
    }
}
/// Deterministic wall/monotonic/timer model with one shared timeline.
#[derive(Debug)]
pub struct DeterministicTime {
    next_wall_ms: AtomicU64,
    now_ns: AtomicU64,
    wall_step_ns: u64,
}
impl DeterministicTime {
    /// Creates a deterministic timeline.
    pub const fn new(wall_epoch_ms: u64, wall_step_ms: u64) -> Self {
        Self {
            next_wall_ms: AtomicU64::new(wall_epoch_ms),
            now_ns: AtomicU64::new(0),
            wall_step_ns: wall_step_ms.saturating_mul(1_000_000),
        }
    }
}
impl Clone for DeterministicTime {
    fn clone(&self) -> Self {
        Self {
            next_wall_ms: AtomicU64::new(self.next_wall_ms.load(Ordering::Acquire)),
            now_ns: AtomicU64::new(self.now_ns.load(Ordering::Acquire)),
            wall_step_ns: self.wall_step_ns,
        }
    }
}
impl WallClock for DeterministicTime {
    fn now(&self) -> Result<WallTimestamp> {
        self.next_wall_ms
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(self.wall_step_ns / 1_000_000)
            })
            .map(WallTimestamp::from_unix_millis)
            .map_err(|_| Error::Eval("deterministic wall clock overflow".into()))
    }
}
impl MonotonicClock for DeterministicTime {
    fn now_monotonic(&self) -> Result<MonotonicTimestamp> {
        Ok(MonotonicTimestamp::from_nanos(
            self.now_ns.load(Ordering::Acquire),
        ))
    }
}
impl Timer for DeterministicTime {
    fn wait_until(&self, deadline: MonotonicTimestamp) -> Result<()> {
        self.now_ns.fetch_max(deadline.nanos(), Ordering::AcqRel);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_binding_preserves_wall_behavior_and_composes_timer() {
        let time = Arc::new(DeterministicTime::new(1_000, 25));
        let binding = PlatformTime {
            wall: time.clone(),
            monotonic: time.clone(),
            timer: time,
        };
        assert_eq!(binding.wall.now_ms().unwrap(), 1_000);
        assert_eq!(binding.wall.now_ms().unwrap(), 1_025);
        binding
            .timer
            .wait_until(MonotonicTimestamp::from_nanos(50_000_000))
            .unwrap();
        assert_eq!(
            binding.monotonic.now_monotonic().unwrap(),
            MonotonicTimestamp::from_nanos(50_000_000)
        );
    }

    #[test]
    fn deterministic_wall_overflow_remains_fail_closed() {
        let time = DeterministicTime::new(u64::MAX, 1);
        assert!(time.now().is_err());
    }
}
