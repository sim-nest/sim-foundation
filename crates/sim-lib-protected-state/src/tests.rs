use std::{collections::BTreeMap, sync::Arc, thread};

use rand_core::{Infallible, TryCryptoRng, TryRng};
use sim_host_core::{WallClock, WallTimestamp};
use sim_kernel::{AssocTable, Cx, DefaultFactory, HandleSeed, NoopEvalPolicy, Symbol};

use super::*;

struct Ring {
    current: String,
    keys: BTreeMap<String, [u8; KEY_BYTES]>,
}
impl Ring {
    fn with(current: &str, entries: &[(&str, u8)]) -> Self {
        Self {
            current: current.into(),
            keys: entries
                .iter()
                .map(|(id, byte)| ((*id).into(), [*byte; KEY_BYTES]))
                .collect(),
        }
    }
}
impl KeyRing for Ring {
    fn current_key_id(&self) -> Result<String, ProtectError> {
        Ok(self.current.clone())
    }
    fn key(&self, id: &str) -> Result<Option<SecretKey>, ProtectError> {
        Ok(self.keys.get(id).copied().map(SecretKey::new))
    }
}
struct FixedNonce([u8; NONCE_BYTES]);
impl NonceSource for FixedNonce {
    fn fill_nonce(&self, out: &mut [u8; NONCE_BYTES]) -> Result<(), ProtectError> {
        *out = self.0;
        Ok(())
    }
}
struct Clock(u64);
impl WallClock for Clock {
    fn now(&self) -> sim_kernel::Result<WallTimestamp> {
        Ok(WallTimestamp::from_unix_millis(self.0))
    }
}

fn binding() -> StateBinding {
    StateBinding::new("continue", "agent", "subject-7", [9_u8; 32], 2_000).unwrap()
}
fn service(ring: Ring, nonce: Arc<dyn NonceSource>, now: u64) -> ProtectedState {
    ProtectedState::new(Arc::new(ring), nonce, Arc::new(Clock(now)))
}

#[test]
fn deterministic_vector_and_exact_binding() {
    let state = service(
        Ring::with("key-a", &[("key-a", 7)]),
        Arc::new(FixedNonce([3; NONCE_BYTES])),
        1_000,
    );
    let envelope = state.protect(b"opaque bytes", &binding()).unwrap();
    assert_eq!(
        hex(&envelope),
        "53505331010100050000001c6b65792d61030303030303030303030303030303030303030303030303eb762a6a0044e6920e642688946de5fb7f2dc41001fd2e4da67b983d"
    );
    assert_eq!(
        state.open(&envelope, &binding()).unwrap().expose(),
        b"opaque bytes"
    );
    let substitutions = [
        StateBinding::new("other", "agent", "subject-7", [9; 32], 2_000).unwrap(),
        StateBinding::new("continue", "other", "subject-7", [9; 32], 2_000).unwrap(),
        StateBinding::new("continue", "agent", "other", [9; 32], 2_000).unwrap(),
        StateBinding::new("continue", "agent", "subject-7", [8; 32], 2_000).unwrap(),
    ];
    for wrong in substitutions {
        assert!(matches!(state.open(&envelope, &wrong), Err(OpenError)));
    }
}

#[test]
fn rotation_retains_old_key_and_rejects_retired_key_uniformly() {
    let old = service(
        Ring::with("old", &[("old", 1)]),
        Arc::new(FixedNonce([4; NONCE_BYTES])),
        1_000,
    );
    let envelope = old.protect(b"state", &binding()).unwrap();
    let rotated = service(
        Ring::with("new", &[("old", 1), ("new", 2)]),
        Arc::new(FixedNonce([5; NONCE_BYTES])),
        1_000,
    );
    assert_eq!(
        rotated.open(&envelope, &binding()).unwrap().expose(),
        b"state"
    );
    let retired = service(
        Ring::with("new", &[("new", 2)]),
        Arc::new(FixedNonce([5; NONCE_BYTES])),
        1_000,
    );
    assert!(matches!(
        retired.open(&envelope, &binding()),
        Err(OpenError)
    ));
}

#[test]
fn malformed_tampered_expired_future_and_huge_inputs_are_bounded_rejections() {
    let state = service(
        Ring::with("k", &[("k", 1)]),
        Arc::new(FixedNonce([0; NONCE_BYTES])),
        1_000,
    );
    let envelope = state.protect(b"secret", &binding()).unwrap();
    for end in 0..envelope.len() {
        assert!(matches!(
            state.open(&envelope[..end], &binding()),
            Err(OpenError)
        ));
    }
    let mut tampered = envelope.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(matches!(state.open(&tampered, &binding()), Err(OpenError)));
    let mut future = envelope.clone();
    future[4] = FORMAT_VERSION + 1;
    assert!(matches!(state.open(&future, &binding()), Err(OpenError)));
    let expired = service(
        Ring::with("k", &[("k", 1)]),
        Arc::new(FixedNonce([0; NONCE_BYTES])),
        2_000,
    );
    assert!(matches!(
        expired.open(&envelope, &binding()),
        Err(OpenError)
    ));
    assert_eq!(
        state.protect(&vec![0; MAX_PLAINTEXT_BYTES + 1], &binding()),
        Err(ProtectError::LimitExceeded)
    );
    assert!(matches!(
        state.open(&vec![0; MAX_ENVELOPE_BYTES + 1], &binding()),
        Err(OpenError)
    ));
}

#[test]
fn reviewed_crypto_rng_source_produces_unique_nonces() {
    let source = Arc::new(CryptoNonceSource::new(CounterCryptoRng::new(42)));
    let state = service(Ring::with("k", &[("k", 1)]), source, 1_000);
    let first = state.protect(b"same", &binding()).unwrap();
    let second = state.protect(b"same", &binding()).unwrap();
    assert_ne!(first, second);
    assert_eq!(state.open(&first, &binding()).unwrap().expose(), b"same");
    assert_eq!(state.open(&second, &binding()).unwrap().expose(), b"same");
}

#[test]
fn canonical_table_consumption_has_one_concurrent_winner() {
    let table = Arc::new(AssocTable::new());
    let winners = (0..32)
        .map(|seed| {
            let table = table.clone();
            thread::spawn(move || {
                let mut cx = Cx::new(
                    Arc::new(NoopEvalPolicy),
                    Arc::new(DefaultFactory),
                    HandleSeed::new(seed),
                );
                TableConsumptionLedger::new(table.as_ref())
                    .claim(&mut cx, Symbol::qualified("protected-state", "claim-7"))
                    .unwrap()
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .filter(|won| *won)
        .count();
    assert_eq!(winners, 1);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct CounterCryptoRng(u64);

impl CounterCryptoRng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
}

impl TryRng for CounterCryptoRng {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(self.next_word() as u32)
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.next_word())
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
        for chunk in dst.chunks_mut(8) {
            let bytes = self.next_word().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        Ok(())
    }
}

impl TryCryptoRng for CounterCryptoRng {}

impl CounterCryptoRng {
    fn next_word(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}
