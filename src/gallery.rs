//! Texture gallery listing + navigation (slice `texture-gallery`).
//!
//! One job: turn a directory into a sorted list of `.DDS` files and move
//! a cursor through it. Decoding stays in [`crate::dds`]; rendering stays
//! in [`crate::render`]. Entries decode lazily (on demand when shown), so
//! `index/total` counts LIST entries, including not-yet-attempted ones.
//!
//! Failure policy: an empty result (zero `.DDS` files) is a LOUD error
//! naming the directory ([`ViewerError::NoDdsFiles`]); the caller exits
//! nonzero with no window. Tests use synthetic temp dirs/files only —
//! never game files.

use std::path::{Path, PathBuf};

use crate::dds::ViewerError;

/// Step direction for [`step_index`]. ArrowRight is `Next`, ArrowLeft is
/// `Prev`. Clamp-at-ends: pressing past either end stays put (no wrap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Next,
    Prev,
}

/// Collect direct-child `.DDS` files of `dir`, sorted ascending by file
/// name (lexicographic on the file name; natural-number ordering is a
/// non-goal). The extension match is case-insensitive (`.DDS` counts);
/// subdirectories are never traversed; non-files are skipped.
///
/// Returns [`ViewerError::NoDdsFiles`] naming the directory when the
/// result would be empty, and [`ViewerError::Io`] when `dir` cannot be
/// read. Game paths arrive via CLI arg at runtime only; nothing here
/// embeds a game path.
pub fn collect_dds_entries(dir: &Path) -> Result<Vec<PathBuf>, ViewerError> {
    let entries = std::fs::read_dir(dir).map_err(ViewerError::Io)?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(ViewerError::Io)?;
        if !entry.file_type().map_err(ViewerError::Io)?.is_file() {
            continue;
        }
        let path = entry.path();
        let is_dds = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("dds"));
        if is_dds {
            out.push(path);
        }
    }
    out.sort_by(|a, b| {
        let fa = a.file_name();
        let fb = b.file_name();
        fa.cmp(&fb)
    });
    if out.is_empty() {
        return Err(ViewerError::NoDdsFiles(dir.display().to_string()));
    }
    Ok(out)
}

/// Pure cursor move with clamp-at-ends (no wrap). `len` is the list
/// length; `current` must be a valid index when `len > 0`. An empty list
/// stays at `0` (there is nothing to show; the caller errors earlier).
pub fn step_index(current: usize, len: usize, dir: Direction) -> usize {
    if len == 0 {
        return 0;
    }
    match dir {
        Direction::Next => current.saturating_add(1).min(len - 1),
        Direction::Prev => current.saturating_sub(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"synthetic-test-bytes").expect("touch must succeed");
    }

    #[test]
    fn lists_sorted_by_file_name_including_uppercase_extension() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        touch(tmp.path(), "b.dds");
        touch(tmp.path(), "a.DDS");
        touch(tmp.path(), "c.dDs");
        let entries = collect_dds_entries(tmp.path()).expect("three dds files must list");
        let names: Vec<String> = entries
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.DDS", "b.dds", "c.dDs"]);
    }

    #[test]
    fn excludes_non_dds_files_and_subdirectories() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        touch(tmp.path(), "keep.dds");
        touch(tmp.path(), "notes.txt");
        touch(tmp.path(), "image.png");
        touch(tmp.path(), "no-extension");
        let sub = tmp.path().join("sub.dds");
        std::fs::create_dir(&sub).expect("subdir named *.dds must be creatable");
        touch(&sub, "nested.dds");
        let entries = collect_dds_entries(tmp.path()).expect("one dds file must list");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].file_name().unwrap().to_string_lossy(),
            "keep.dds"
        );
    }

    #[test]
    fn empty_dir_is_a_loud_error_naming_the_directory() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let err = collect_dds_entries(tmp.path()).expect_err("empty dir must fail");
        match &err {
            ViewerError::NoDdsFiles(dir) => assert!(
                dir.contains(&tmp.path().display().to_string()),
                "error must name the directory, got: {dir}"
            ),
            other => panic!("expected NoDdsFiles, got: {other:?}"),
        }
        let msg = err.to_string();
        assert!(
            msg.contains(&tmp.path().display().to_string()),
            "Display must name the directory, got: {msg}"
        );
    }

    #[test]
    fn dir_without_dds_but_with_other_files_is_empty_too() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        touch(tmp.path(), "readme.txt");
        assert!(matches!(
            collect_dds_entries(tmp.path()),
            Err(ViewerError::NoDdsFiles(_))
        ));
    }

    #[test]
    fn step_clamps_at_both_ends_without_wrapping() {
        let len = 5;
        assert_eq!(step_index(0, len, Direction::Prev), 0);
        assert_eq!(step_index(0, len, Direction::Next), 1);
        assert_eq!(step_index(len - 1, len, Direction::Next), len - 1);
        assert_eq!(step_index(len - 1, len, Direction::Prev), len - 2);
        assert_eq!(step_index(2, len, Direction::Next), 3);
        assert_eq!(step_index(2, len, Direction::Prev), 1);
    }

    #[test]
    fn single_entry_list_never_moves() {
        assert_eq!(step_index(0, 1, Direction::Next), 0);
        assert_eq!(step_index(0, 1, Direction::Prev), 0);
    }
}
