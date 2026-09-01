use std::sync::Arc;

use sim_host_core::{DeterministicTime, MonotonicTimestamp, PlatformTime, WallClock};

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
