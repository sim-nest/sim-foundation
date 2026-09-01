//! Executor-neutral, explicitly owned cancellation for bounded work.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{
    collections::BTreeMap,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    task::{Context, Poll, Waker},
};

/// Maximum UTF-8 byte length accepted for a cancellation reason.
pub const MAX_REASON_BYTES: usize = 256;

/// Embedded checked cookbook recipe tree.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

/// The immutable explanation attached to the terminal transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancellationReason(Arc<str>);

impl CancellationReason {
    /// Validates and constructs a bounded, non-empty reason.
    pub fn new(reason: impl Into<String>) -> Result<Self, InvalidReason> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(InvalidReason::Empty);
        }
        if reason.len() > MAX_REASON_BYTES {
            return Err(InvalidReason::TooLong {
                actual: reason.len(),
                maximum: MAX_REASON_BYTES,
            });
        }
        Ok(Self(reason.into()))
    }
    /// Returns the validated reason text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why construction of a cancellation reason failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidReason {
    /// Whitespace-only and empty reasons carry no useful evidence.
    Empty,
    /// The reason exceeded the fixed memory and disclosure bound.
    TooLong {
        /// Supplied UTF-8 byte count.
        actual: usize,
        /// Accepted UTF-8 byte count.
        maximum: usize,
    },
}
impl fmt::Display for InvalidReason {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "invalid cancellation reason: {self:?}")
    }
}
impl std::error::Error for InvalidReason {}

#[derive(Debug)]
struct Inner {
    state: Mutex<State>,
}
#[derive(Debug, Default)]
struct State {
    reason: Option<CancellationReason>,
    next_waiter: u64,
    waiters: BTreeMap<u64, Waker>,
    children: Vec<Weak<Inner>>,
}

/// Explicit cancellation authority and observer for one lifetime.
///
/// There is deliberately no global token or ambient lookup. Hosts construct a
/// root, derive children for owned work, and pass those values explicitly.
#[derive(Clone, Debug)]
pub struct Cancellation {
    inner: Arc<Inner>,
}
impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl Cancellation {
    /// Constructs an open, independent cancellation lifetime.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State::default()),
            }),
        }
    }
    /// Constructs an open child which follows this token's terminal state.
    #[must_use]
    pub fn child(&self) -> Self {
        let child = Self::new();
        let inherited = {
            let mut state = self
                .inner
                .state
                .lock()
                .expect("cancellation mutex poisoned");
            match state.reason.clone() {
                Some(reason) => Some(reason),
                None => {
                    state.children.retain(|entry| entry.strong_count() > 0);
                    state.children.push(Arc::downgrade(&child.inner));
                    None
                }
            }
        };
        if let Some(reason) = inherited {
            child.cancel(reason);
        }
        child
    }
    /// Performs the sole open-to-cancelled transition, returning true only to its winner.
    pub fn cancel(&self, reason: CancellationReason) -> bool {
        let (waiters, children) = {
            let mut state = self
                .inner
                .state
                .lock()
                .expect("cancellation mutex poisoned");
            if state.reason.is_some() {
                return false;
            }
            state.reason = Some(reason.clone());
            let waiters = std::mem::take(&mut state.waiters)
                .into_values()
                .collect::<Vec<_>>();
            let children = std::mem::take(&mut state.children)
                .into_iter()
                .filter_map(|child| child.upgrade())
                .collect::<Vec<_>>();
            (waiters, children)
        };
        for child in children {
            Self { inner: child }.cancel(reason.clone());
        }
        for waiter in waiters {
            waiter.wake();
        }
        true
    }
    /// Observes the terminal reason without blocking.
    #[must_use]
    pub fn reason(&self) -> Option<CancellationReason> {
        self.inner
            .state
            .lock()
            .expect("cancellation mutex poisoned")
            .reason
            .clone()
    }
    /// Returns whether the terminal transition has occurred.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.reason().is_some()
    }
    /// Registers an executor-neutral waiter for the terminal reason.
    #[must_use]
    pub fn cancelled(&self) -> CancellationWaiter {
        CancellationWaiter {
            inner: Arc::downgrade(&self.inner),
            registration: None,
        }
    }
}

/// A race-safe future resolving to the immutable terminal reason.
///
/// It holds only a weak token reference and unregisters its stored waker when dropped.
#[derive(Debug)]
pub struct CancellationWaiter {
    inner: Weak<Inner>,
    registration: Option<u64>,
}
impl Future for CancellationWaiter {
    type Output = CancellationReason;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(inner) = self.inner.upgrade() else {
            return Poll::Pending;
        };
        let mut state = inner.state.lock().expect("cancellation mutex poisoned");
        if let Some(reason) = state.reason.clone() {
            if let Some(id) = self.registration.take() {
                state.waiters.remove(&id);
            }
            return Poll::Ready(reason);
        }
        match self.registration {
            Some(id) => {
                if !state
                    .waiters
                    .get(&id)
                    .is_some_and(|old| old.will_wake(cx.waker()))
                {
                    state.waiters.insert(id, cx.waker().clone());
                }
            }
            None => {
                let id = state.next_waiter;
                state.next_waiter = state.next_waiter.wrapping_add(1);
                state.waiters.insert(id, cx.waker().clone());
                self.registration = Some(id);
            }
        }
        Poll::Pending
    }
}
impl Drop for CancellationWaiter {
    fn drop(&mut self) {
        let Some(id) = self.registration else { return };
        if let Some(inner) = self.inner.upgrade()
            && let Ok(mut state) = inner.state.lock()
        {
            state.waiters.remove(&id);
        }
    }
}
#[cfg(test)]
mod tests;
