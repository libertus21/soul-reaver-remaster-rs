//! Engine core: a headless simulation layer built on the read-only asset
//! layer. This slice only wires the `--play` option facade; the world model,
//! clock, tracker, hooks and renderer arrive in later slices.

pub mod play;
