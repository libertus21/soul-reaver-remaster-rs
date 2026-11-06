//! Determinism harness: stable hashing for the simulation state.
//!
//! Determinism is the premise of this engine: the original advances on a fixed
//! 60 Hz rational tick over plain structs, ordered arenas and a fixed 48-slot
//! tracker. A regression still runs, so it is only caught by comparing state.
//! The policy lives in `docs/notes/determinism.md`; the gate is stated in
//! `docs/changes/engine-skeleton/tasks.md`.
//!
//! T1 has no world and no tick loop, so [`state_hash`] folds only what exists
//! today: the parsed [`PlayOptions`] or the typed error text. T2 grows this
//! module to fold the world state (tick count, live order, class, position,
//! visible) into the canonical FNV-1a 64 digest from `spec.md`. The hasher and
//! the "same input, same bytes" tests are the pattern that must survive that
//! growth unchanged.

use std::path::Path;

use crate::engine::play::{PlayError, PlayOptions};

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Stable FNV-1a 64 hasher.
///
/// `DefaultHasher` is explicitly unstable across Rust releases; FNV-1a is
/// dependency-free and byte-exact on every platform, which the digest contract
/// requires.
#[derive(Debug, Clone, Copy)]
pub struct StateHasher {
    state: u64,
}

impl StateHasher {
    pub const fn new() -> Self {
        Self {
            state: FNV_OFFSET_BASIS,
        }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u64::from(*byte);
            self.state = self.state.wrapping_mul(FNV_PRIME);
        }
    }

    /// Fold a variable-length segment behind its length so concatenation stays
    /// unambiguous: `"ab"+"c"` must not hash like `"a"+"bc"`.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_u64(bytes.len() as u64);
        self.write(bytes);
    }

    pub fn write_u8(&mut self, value: u8) {
        self.write(&[value]);
    }

    pub fn write_u32(&mut self, value: u32) {
        self.write(&value.to_le_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    pub fn finish(&self) -> u64 {
        self.state
    }
}

impl Default for StateHasher {
    fn default() -> Self {
        Self::new()
    }
}

fn write_path(hasher: &mut StateHasher, path: &Path) {
    hasher.write_bytes(path.as_os_str().as_encoded_bytes());
}

fn write_options(hasher: &mut StateHasher, options: &PlayOptions) {
    write_path(hasher, &options.bigfile);
    match &options.textures {
        Some(path) => {
            hasher.write_u8(1);
            write_path(hasher, path);
        }
        None => hasher.write_u8(0),
    }
    hasher.write_bytes(options.unit.as_bytes());
    match options.ticks {
        Some(ticks) => {
            hasher.write_u8(1);
            hasher.write_u32(ticks);
        }
        None => hasher.write_u8(0),
    }
    hasher.write_u8(u8::from(options.headless));
    hasher.write_u8(options.camera as u8);
}

fn error_tag(error: &PlayError) -> u8 {
    match error {
        PlayError::UnknownOption(_) => 0,
        PlayError::MissingValue(_) => 1,
        PlayError::MissingArchive => 2,
        PlayError::InvalidTicks(_) => 3,
        PlayError::ZeroTicksHeadless => 4,
        PlayError::InvalidCamera(_) => 5,
        PlayError::NotImplemented => 6,
    }
}

/// Hash the parsed options alone.
pub fn options_hash(options: &PlayOptions) -> u64 {
    let mut hasher = StateHasher::new();
    write_options(&mut hasher, options);
    hasher.finish()
}

/// Hash a typed error by variant and exact message.
pub fn error_hash(error: &PlayError) -> u64 {
    let mut hasher = StateHasher::new();
    hasher.write_u8(error_tag(error));
    hasher.write_bytes(error.to_string().as_bytes());
    hasher.finish()
}

/// Hash the observable result of one setup step.
///
/// T2 replaces the `Ok` arm with the world-state fold and keeps this signature,
/// so the gate test grows from "same argv" to "N ticks" without changing shape.
pub fn state_hash(result: &Result<PlayOptions, PlayError>) -> u64 {
    let mut hasher = StateHasher::new();
    match result {
        Ok(options) => {
            hasher.write_u8(0);
            write_options(&mut hasher, options);
        }
        Err(error) => {
            hasher.write_u8(1);
            hasher.write_u8(error_tag(error));
            hasher.write_bytes(error.to_string().as_bytes());
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::{error_hash, options_hash, state_hash, StateHasher};
    use crate::engine::play::{PlayError, PlayOptions};

    fn parse(input: &[&str]) -> Result<PlayOptions, PlayError> {
        let argv: Vec<String> = input.iter().map(|token| token.to_string()).collect();
        PlayOptions::parse(&argv)
    }

    #[test]
    fn hasher_is_stable_and_length_prefixed() {
        let mut split = StateHasher::new();
        split.write_bytes(b"ab");
        split.write_bytes(b"c");

        let mut joined = StateHasher::new();
        joined.write_bytes(b"a");
        joined.write_bytes(b"bc");
        assert_ne!(split.finish(), joined.finish());

        let mut first = StateHasher::new();
        first.write(b"same");
        let mut second = StateHasher::new();
        second.write(b"same");
        assert_eq!(first.finish(), second.finish());
    }

    #[test]
    fn same_argv_parses_byte_identical_and_hashes_equal() {
        let input = [
            "--bigfile",
            "BIGFILE.DAT",
            "--textures",
            "TEXTURES.BIG",
            "--unit",
            "under2",
            "--ticks",
            "120",
            "--headless",
            "--camera",
            "player",
        ];
        let first = parse(&input).expect("first parse");
        let second = parse(&input).expect("second parse");
        assert_eq!(first, second);
        assert_eq!(options_hash(&first), options_hash(&second));
        assert_eq!(state_hash(&Ok(first)), state_hash(&Ok(second)));
    }

    #[test]
    fn same_invalid_argv_repeats_variant_message_and_hash() {
        let input = ["--bigfile", "B", "--camera", "top"];
        let first = parse(&input).expect_err("first parse must fail");
        let second = parse(&input).expect_err("second parse must fail");
        assert_eq!(first, second);
        assert_eq!(first.to_string(), second.to_string());
        assert_eq!(error_hash(&first), error_hash(&second));
        assert_eq!(state_hash(&Err(first)), state_hash(&Err(second)));
    }

    #[test]
    fn different_argv_changes_the_hash() {
        let base = parse(&["--bigfile", "B", "--unit", "under1"]).expect("base parse");
        let other = parse(&["--bigfile", "B", "--unit", "under2"]).expect("other parse");
        assert_ne!(base, other);
        assert_ne!(options_hash(&base), options_hash(&other));
        assert_ne!(state_hash(&Ok(base)), state_hash(&Ok(other)));
    }
}
