# sim-work-core

`sim-work-core` defines pure, bounded work envelopes and implementation packets.
Construction distinguishes already released dependencies from still-Planned
funded targets, reads only declared exact sources through an injected port,
counts complete byte/file/token input, and rejects excess rather than truncating
it. Packet identity covers semantic content and policy while storage locations
remain replaceable transport data.

Worker returns are complete, honestly incomplete, or malformed opaque bytes.
The crate grants no filesystem, process, Git, network, delivery, or approval
authority.

The `descriptor` recipe-policy applies because the recipe records this pure
contract without pretending to execute an implementation worker. Integration
tests exercise deterministic construction and fail-closed admission.
