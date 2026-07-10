//! `axiom-benches`: shared helpers for the Axiom IR benchmark suite.
//!
//! The crate exposes pure dataset generators (see [`generators`]) so every
//! `[[bench]]` target can build identical workloads. Benchmarks measure the
//! performance characteristics the Axiom IR spec requires; see `benches/README.md`
//! for the methodology. Nothing here executes the runtime — generators only
//! emit source text for the Axiom textual language.

pub mod generators;
