use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Barrier, Mutex},
    task::{Context, Poll, Wake, Waker},
    thread,
};
fn reason(text: &str) -> CancellationReason {
    CancellationReason::new(text).unwrap()
}
#[derive(Default)]
struct WakeCount(Mutex<usize>);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        *self.0.lock().unwrap() += 1;
    }
}
fn poll(waiter: &mut CancellationWaiter, wake: &Arc<WakeCount>) -> Poll<CancellationReason> {
    let waker = Waker::from(wake.clone());
    Pin::new(waiter).poll(&mut Context::from_waker(&waker))
}

#[test]
fn cancel_before_register_has_no_lost_wakeup() {
    let token = Cancellation::new();
    assert!(token.cancel(reason("deadline")));
    assert_eq!(
        poll(&mut token.cancelled(), &Arc::default()),
        Poll::Ready(reason("deadline"))
    );
}
#[test]
fn cancel_during_registration_has_one_terminal_transition() {
    for _ in 0..128 {
        let token = Cancellation::new();
        let barrier = Arc::new(Barrier::new(2));
        let other = token.clone();
        let other_barrier = barrier.clone();
        let join = thread::spawn(move || {
            other_barrier.wait();
            other.cancel(reason("race"))
        });
        let mut waiter = token.cancelled();
        barrier.wait();
        let wake = Arc::default();
        let first = poll(&mut waiter, &wake);
        assert!(join.join().unwrap());
        assert_eq!(poll(&mut waiter, &wake), Poll::Ready(reason("race")));
        assert!(matches!(first, Poll::Pending | Poll::Ready(_)));
    }
}
#[test]
fn duplicate_cancel_preserves_first_reason() {
    let token = Cancellation::new();
    assert!(token.cancel(reason("first")));
    assert!(!token.cancel(reason("second")));
    assert_eq!(token.reason(), Some(reason("first")));
}
#[test]
fn parent_propagates_to_live_children_without_reverse_authority() {
    let parent = Cancellation::new();
    let child = parent.child();
    let grandchild = child.child();
    assert!(parent.cancel(reason("request complete")));
    assert_eq!(child.reason(), Some(reason("request complete")));
    assert_eq!(grandchild.reason(), Some(reason("request complete")));
    let independent = Cancellation::new();
    assert!(independent.child().cancel(reason("local")));
    assert!(!independent.is_cancelled());
}
#[test]
fn dropped_waiter_releases_application_waker() {
    let token = Cancellation::new();
    let application = Arc::new(WakeCount::default());
    let mut waiter = token.cancelled();
    assert_eq!(poll(&mut waiter, &application), Poll::Pending);
    let weak = Arc::downgrade(&application);
    drop(application);
    drop(waiter);
    assert!(weak.upgrade().is_none());
    assert!(token.cancel(reason("cleanup")));
}
#[test]
fn timeout_conversion_and_request_completion_are_idempotent() {
    let request = Cancellation::new();
    let timeout = request.child();
    assert!(timeout.cancel(reason("deadline elapsed")));
    assert!(request.cancel(reason("request completed")));
    assert!(!request.cancel(reason("late timeout")));
    assert_eq!(request.reason(), Some(reason("request completed")));
}
#[test]
fn reasons_are_bounded() {
    assert_eq!(CancellationReason::new(" "), Err(InvalidReason::Empty));
    assert!(matches!(
        CancellationReason::new("x".repeat(MAX_REASON_BYTES + 1)),
        Err(InvalidReason::TooLong { .. })
    ));
}
