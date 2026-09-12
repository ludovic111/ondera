# ondera-engine

Native Rust session/command store, DSP, audio library, CPAL device workers, recording and atomic
project persistence. The desktop and headless export share this crate. It has no GUI dependency.

Run `cargo test -p ondera-engine`, or benchmark with
`cargo run --release -p ondera-engine --example benchmark` from the repository root.

Render graphs are prepared off-thread. Audio callbacks consume bounded queues and publish atomic
telemetry. Source buffers use shared ownership; replaced graphs are reclaimed outside callbacks.
Hardware-independent tests cover rendering, transport, sessions, history and real-time allocations.
