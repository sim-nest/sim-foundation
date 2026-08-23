# Exact binding

Serialize the application payload first, derive the canonical context digest in the calling protocol, then construct `StateBinding`. Inject a read-only retained-key ring, a reviewed `CryptoRng` through `CryptoNonceSource`, and the platform wall clock. Protect with the current key; open under the identical binding.

If the value must be single-use, derive a non-secret opaque claim key and call `ConsumptionLedger::claim`. Encryption alone does not stop replay.
