//! SR1-remaster `.SRL` clip header parser (voice-line animation container).
//!
//! Clean-room, shapes only; see `docs/notes/srl-animation.md`. A `.SRL` is a
//! 20-byte header (`SRL\x02`, track count, version, frame count, frames per
//! second) followed by `tracks * frames` 48-byte cells laid out frame-major.
//! The container arithmetic is confirmed on all 60 shipped files. The cell is a
//! twelve-`f32` 3x4 transform, but the track-to-bone binding is UNKNOWN, so this
//! reader prints counts and the length identity only; it never claims a pose.
//! Tests use synthetic bytes only — never game files.

use std::fmt;
use std::path::Path;

pub const MAGIC: &[u8; 4] = b"SRL\x02";
pub const HEADER_LEN: u64 = 20;
pub const CELL_STRIDE: u64 = 48;
/// Hard ceilings so a corrupt header cannot overflow or drive a huge predict.
pub const MAX_TRACKS: u32 = 4096;
pub const MAX_FRAMES: u32 = 1_000_000;

/// Header fields of one `.SRL` clip. Track semantics stay UNKNOWN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrlClip {
    pub version: u32,
    pub tracks: u32,
    pub frames: u32,
    pub fps: u32,
}

impl SrlClip {
    /// Play length in seconds (`frames / fps`); `None` when `fps` is zero.
    pub fn duration_seconds(&self) -> Option<f64> {
        (self.fps != 0).then(|| f64::from(self.frames) / f64::from(self.fps))
    }

    /// Exact on-disk length this header predicts, or `None` on overflow.
    pub fn expected_len(&self) -> Option<u64> {
        let cells = u64::from(self.tracks).checked_mul(u64::from(self.frames))?;
        cells.checked_mul(CELL_STRIDE)?.checked_add(HEADER_LEN)
    }
}

#[derive(Debug)]
pub enum SrlError {
    Io(std::io::Error),
    NotSrl(String),
    InvalidSrl(String),
}

impl fmt::Display for SrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "cannot read SRL file: {error}"),
            Self::NotSrl(message) => write!(f, "not an SRL clip: {message}"),
            Self::InvalidSrl(message) => write!(f, "invalid SRL clip: {message}"),
        }
    }
}

impl std::error::Error for SrlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl SrlError {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Io(_) => 1,
            Self::NotSrl(_) | Self::InvalidSrl(_) => 2,
        }
    }
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

pub fn parse(bytes: &[u8]) -> Result<SrlClip, SrlError> {
    let file_len = bytes.len() as u64;
    if file_len < 4 {
        return Err(SrlError::NotSrl(format!(
            "file has {file_len} bytes, fewer than the 4-byte magic"
        )));
    }
    if &bytes[0..4] != MAGIC {
        return Err(SrlError::NotSrl(format!(
            "bad magic {:02X?} (expected 53 52 4C 02 for `SRL\\x02`)",
            &bytes[0..4]
        )));
    }
    if file_len < HEADER_LEN {
        return Err(SrlError::InvalidSrl(format!(
            "header needs {HEADER_LEN} bytes, file has {file_len}"
        )));
    }
    let tracks = u32_at(bytes, 4).unwrap();
    let version = u32_at(bytes, 8).unwrap();
    let frames = u32_at(bytes, 12).unwrap();
    let fps = u32_at(bytes, 16).unwrap();
    if tracks == 0 {
        return Err(SrlError::InvalidSrl("track count is zero".to_string()));
    }
    if frames == 0 {
        return Err(SrlError::InvalidSrl("frame count is zero".to_string()));
    }
    if tracks > MAX_TRACKS {
        return Err(SrlError::InvalidSrl(format!(
            "insane track count {tracks} (limit {MAX_TRACKS})"
        )));
    }
    if frames > MAX_FRAMES {
        return Err(SrlError::InvalidSrl(format!(
            "insane frame count {frames} (limit {MAX_FRAMES})"
        )));
    }
    let clip = SrlClip { version, tracks, frames, fps };
    let expected = clip
        .expected_len()
        .ok_or_else(|| SrlError::InvalidSrl("length arithmetic overflowed".to_string()))?;
    if expected != file_len {
        return Err(SrlError::InvalidSrl(format!(
            "length mismatch: header {tracks} tracks x {frames} frames x {CELL_STRIDE} + {HEADER_LEN} = {expected} bytes, file has {file_len}"
        )));
    }
    Ok(clip)
}

/// Read and parse one `.SRL`; returns the clip plus the file length.
pub fn load(path: &Path) -> Result<(SrlClip, u64), SrlError> {
    let bytes = std::fs::read(path).map_err(SrlError::Io)?;
    let clip = parse(&bytes)?;
    Ok((clip, bytes.len() as u64))
}

/// Two display lines: the clip summary, then the confirmed layout identity.
pub fn summary_lines(name: &str, clip: &SrlClip, bytes: u64) -> Vec<String> {
    let duration = clip
        .duration_seconds()
        .map_or_else(|| "n/a".to_string(), |seconds| format!("{seconds:.3}s"));
    vec![
        format!(
            "srl {name} version={} tracks={} frames={} fps={} duration={duration} bytes={bytes}",
            clip.version, clip.tracks, clip.frames, clip.fps
        ),
        format!(
            "  layout: {HEADER_LEN}-byte header + {} tracks x {} frames x {CELL_STRIDE}-byte cells = {bytes} bytes (frame-major); track-to-bone binding UNKNOWN",
            clip.tracks, clip.frames
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip_bytes(tracks: u32, frames: u32, fps: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&tracks.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&frames.to_le_bytes());
        bytes.extend_from_slice(&fps.to_le_bytes());
        let cells = (tracks * frames) as usize;
        bytes.extend(std::iter::repeat_n(0u8, cells * CELL_STRIDE as usize));
        bytes
    }

    #[test]
    fn parses_header_and_predicts_exact_length() {
        let clip = parse(&clip_bytes(2, 3, 25)).expect("synthetic clip must parse");
        assert_eq!(clip, SrlClip { version: 1, tracks: 2, frames: 3, fps: 25 });
        assert_eq!(clip.expected_len(), Some(20 + 6 * 48));
        assert_eq!(clip.duration_seconds(), Some(0.12));
    }

    #[test]
    fn duration_is_none_only_when_fps_is_zero() {
        let clip = SrlClip { version: 1, tracks: 1, frames: 60, fps: 0 };
        assert_eq!(clip.duration_seconds(), None);
    }

    #[test]
    fn bad_magic_and_short_files_are_not_srl() {
        assert!(matches!(parse(b"SR"), Err(SrlError::NotSrl(_))));
        let mut bad = clip_bytes(1, 1, 24);
        bad[0..4].copy_from_slice(b"NOPE");
        assert!(matches!(parse(&bad), Err(SrlError::NotSrl(_))));
    }

    #[test]
    fn truncated_header_and_length_mismatch_are_loud() {
        let full = clip_bytes(1, 1, 24);
        assert!(matches!(parse(&full[..4]), Err(SrlError::InvalidSrl(_))));
        let mut cut = full.clone();
        cut.pop();
        let error = parse(&cut).unwrap_err();
        assert!(error.to_string().contains("length mismatch"), "got: {error}");
        let mut extra = full.clone();
        extra.push(0);
        assert!(parse(&extra).unwrap_err().to_string().contains("length mismatch"));
    }

    #[test]
    fn zero_and_insane_counts_are_refused() {
        assert!(parse(&clip_bytes(0, 1, 24)).unwrap_err().to_string().contains("track count is zero"));
        assert!(parse(&clip_bytes(1, 0, 24)).unwrap_err().to_string().contains("frame count is zero"));
        let mut huge = clip_bytes(1, 1, 24);
        huge[4..8].copy_from_slice(&(MAX_TRACKS + 1).to_le_bytes());
        assert!(parse(&huge).unwrap_err().to_string().contains("insane track"));
        let mut many = clip_bytes(1, 1, 24);
        many[12..16].copy_from_slice(&(MAX_FRAMES + 1).to_le_bytes());
        assert!(parse(&many).unwrap_err().to_string().contains("insane frame"));
    }

    #[test]
    fn load_missing_path_maps_to_io() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(matches!(load(&tmp.path().join("absent.srl")), Err(SrlError::Io(_))));
    }

    #[test]
    fn load_reports_the_file_length() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("clip.srl");
        std::fs::write(&path, clip_bytes(3, 4, 30)).expect("write");
        let (clip, bytes) = load(&path).expect("must load");
        assert_eq!((clip.tracks, clip.frames, clip.fps), (3, 4, 30));
        assert_eq!(bytes, 20 + 12 * 48);
    }

    #[test]
    fn summary_names_counts_duration_and_unknown_binding() {
        let clip = SrlClip { version: 1, tracks: 7, frames: 156, fps: 24 };
        let lines = summary_lines("RCLASH01", &clip, 20 + 7 * 156 * 48);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("tracks=7 frames=156 fps=24 duration=6.500s"));
        assert!(lines[1].contains("frame-major"));
        assert!(lines[1].contains("track-to-bone binding UNKNOWN"));
    }
}
