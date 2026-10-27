//! Canonical RIFF/WAVE PCM16 mono writer plus the in-repo output guard.
//!
//! The audio slice writes at most one scratch WAV, to a path the user
//! names. The writer refuses any path inside a git working tree so game
//! audio can never land in this repository.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

pub const WAV_HEADER_BYTES: usize = 44;

const PCM_FORMAT: u16 = 1;
const CHANNELS: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;

/// Build the canonical 44-byte header plus little-endian PCM16 samples.
pub fn mono_pcm16(samples: &[i16], rate: u32) -> Vec<u8> {
    let data_bytes = samples.len() * 2;
    let block_align = CHANNELS * BITS_PER_SAMPLE / 8;
    let byte_rate = rate * u32::from(block_align);
    let mut out = Vec::with_capacity(WAV_HEADER_BYTES + data_bytes);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_bytes as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&PCM_FORMAT.to_le_bytes());
    out.extend_from_slice(&CHANNELS.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// Write one complete WAV image. A failed write removes the partial file.
pub fn write_mono_pcm16(path: &Path, samples: &[i16], rate: u32) -> io::Result<()> {
    let bytes = mono_pcm16(samples, rate);
    let mut file = File::create(path)?;
    let written = file.write_all(&bytes).and_then(|()| file.flush());
    if let Err(error) = written {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

/// Refuse a path that, or whose parent chain, contains a `.git` entry.
pub fn ensure_outside_git_tree(path: &Path) -> Result<(), String> {
    let absolute = std::path::absolute(path)
        .map_err(|error| format!("cannot resolve output path {}: {error}", path.display()))?;
    for ancestor in absolute.ancestors() {
        if ancestor.join(".git").exists() {
            return Err(format!(
                "refusing to write inside a git working tree ({}); \
                 game audio must stay outside the repository",
                ancestor.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16_at(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("header field"))
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("header field"))
    }

    #[test]
    fn header_is_canonical_and_sizes_match() {
        let bytes = mono_pcm16(&[0x1234, -1], 44_100);
        assert_eq!(bytes.len(), WAV_HEADER_BYTES + 4);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32_at(&bytes, 4), 40);
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(u32_at(&bytes, 16), 16);
        assert_eq!(u16_at(&bytes, 20), 1);
        assert_eq!(u16_at(&bytes, 22), 1);
        assert_eq!(u32_at(&bytes, 24), 44_100);
        assert_eq!(u32_at(&bytes, 28), 88_200);
        assert_eq!(u16_at(&bytes, 32), 2);
        assert_eq!(u16_at(&bytes, 34), 16);
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(u32_at(&bytes, 40), 4);
        assert_eq!(&bytes[44..46], &0x1234i16.to_le_bytes());
        assert_eq!(&bytes[46..48], &(-1i16).to_le_bytes());
    }

    #[test]
    fn git_guard_accepts_a_plain_temp_dir_and_refuses_a_tree() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        assert!(ensure_outside_git_tree(&tmp.path().join("a.wav")).is_ok());

        std::fs::create_dir(tmp.path().join(".git")).expect("synthetic .git dir");
        let refused = ensure_outside_git_tree(&tmp.path().join("a.wav"))
            .expect_err("a git tree must be refused");
        assert!(
            refused.contains("git working tree")
                && refused.contains("stay outside the repository"),
            "got: {refused}"
        );

        let nested = tmp.path().join("nested");
        std::fs::create_dir(&nested).expect("nested dir");
        assert!(ensure_outside_git_tree(&nested.join("b.wav")).is_err());
    }

    #[test]
    fn writer_emits_the_whole_buffer() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let path = tmp.path().join("a.wav");
        let samples = [1i16, -2, 3];
        write_mono_pcm16(&path, &samples, 44_100).expect("wav must write");
        assert_eq!(
            std::fs::read(&path).expect("wav must read"),
            mono_pcm16(&samples, 44_100)
        );
    }
}
