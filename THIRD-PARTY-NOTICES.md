# Third-party notices

Community code reused by this project. Created on first reuse per the
README checklist; kept current as new dependencies arrive.

## bcdec_rs 0.2.0 (MIT)

- Use: CPU decode of BC7 blocks to RGBA8 (`decode_bc7_to_rgba8` in
  `src/dds.rs`, slice `bc7-cpu-decode`).
- Source: <https://github.com/ScanMountGoat/image_dds> (crate `bcdec_rs`,
  exact pin `=0.2.0` in `Cargo.toml`, `Cargo.lock` committed).
- License: MIT (`license = "MIT"` in the crate manifest). Safe, `no_std`,
  pure-Rust port of the C `bcdec` library (<https://github.com/iOrange/bcdec>),
  fuzzed against bindings to the original for identical behavior.
- Rationale: `image` 0.25 decodes only BC1/BC2/BC3, so no existing
  dependency can decode BC7; this crate's one-block API consumes the
  existing `CompressedBc7.blocks` with no reshaping.
