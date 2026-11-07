//! T10 synthetic-archive acceptance tests.
//!
//! Builds a minimal `BIGFILE.DAT` in a temporary directory (one valid `.PCM`
//! room plus synthetic model containers) and runs the production
//! `headless_run` path. No game bytes are read or written.

use super::{HeadlessReport, RunError, boot_world, headless_run};
use crate::engine::clock::MAX_CATCH_UP;
use crate::engine::frame::FrameState;
use crate::engine::world::World;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn suffix(name: &str) -> [u8; 4] {
    let bytes = name.as_bytes();
    let mut suffix = [0u8; 4];
    suffix.copy_from_slice(&bytes[bytes.len() - 4..]);
    suffix.make_ascii_uppercase();
    suffix
}

/// Minimal archive: count, index entries, then the payloads.
fn synthetic_archive(entries: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let table_len = 4 + entries.len() * 16;
    let mut bytes = (entries.len() as u32).to_le_bytes().to_vec();
    let mut offset = table_len;
    for (index, (suffix, payload)) in entries.iter().enumerate() {
        bytes.extend_from_slice(&(index as u32 + 1).to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        bytes.extend_from_slice(suffix);
        offset += payload.len();
    }
    for (_, payload) in entries {
        bytes.extend_from_slice(payload);
    }
    bytes
}

/// Rename the shared synthetic object container in place.
fn named_model(name: &str) -> Vec<u8> {
    let mut bytes = crate::object::synthetic_model();
    let base = crate::level::data_base(1);
    bytes[base + 0x4C..base + 0x58].fill(0);
    bytes[base + 0x4C..base + 0x4C + name.len()].copy_from_slice(name.as_bytes());
    bytes
}

/// Minimal valid `.PCM` room: one triangle plus the given placed records.
fn pcm_room(name: &str, objects: &[(&str, [i16; 3])]) -> Vec<u8> {
    const NAME_AT: usize = 0x80;
    const ROOM_AT: usize = 0x100;
    const VERTEX_AT: usize = 0x200;
    const FACE_AT: usize = 0x240;
    const OBJECT_AT: usize = 0x300;

    let mut body = vec![0u8; OBJECT_AT + objects.len() * 0x4C + 16];
    body[0..4].copy_from_slice(&(ROOM_AT as u32).to_le_bytes());
    body[4 * 0x26..4 * 0x26 + 4].copy_from_slice(&(NAME_AT as u32).to_le_bytes());
    body[NAME_AT..NAME_AT + name.len()].copy_from_slice(name.as_bytes());

    body[ROOM_AT + 0x08..ROOM_AT + 0x0C].copy_from_slice(&(objects.len() as u32).to_le_bytes());
    body[ROOM_AT + 0x0C..ROOM_AT + 0x10].copy_from_slice(&(OBJECT_AT as u32).to_le_bytes());
    body[ROOM_AT + 0x10..ROOM_AT + 0x14].copy_from_slice(&3u32.to_le_bytes());
    body[ROOM_AT + 0x14..ROOM_AT + 0x18].copy_from_slice(&1u32.to_le_bytes());
    body[ROOM_AT + 0x1C..ROOM_AT + 0x20].copy_from_slice(&(VERTEX_AT as u32).to_le_bytes());
    body[ROOM_AT + 0x20..ROOM_AT + 0x24].copy_from_slice(&(FACE_AT as u32).to_le_bytes());

    for (index, vertex) in [[0i16, 0, 0], [100, 0, 0], [0, 100, 0]].iter().enumerate() {
        let at = VERTEX_AT + index * 12;
        for axis in 0..3 {
            body[at + 2 * axis..at + 2 * axis + 2].copy_from_slice(&vertex[axis].to_le_bytes());
        }
    }
    for (slot, index) in [0u16, 1, 2].iter().enumerate() {
        let at = FACE_AT + 2 * slot;
        body[at..at + 2].copy_from_slice(&index.to_le_bytes());
    }
    body[FACE_AT + 0x0A..FACE_AT + 0x0C].copy_from_slice(&u16::MAX.to_le_bytes());

    for (index, (object, position)) in objects.iter().enumerate() {
        let at = OBJECT_AT + index * 0x4C;
        body[at..at + object.len()].copy_from_slice(object.as_bytes());
        for axis in 0..3 {
            body[at + 0x20 + 2 * axis..at + 0x20 + 2 * axis + 2]
                .copy_from_slice(&position[axis].to_le_bytes());
        }
        body[at + 0x28..at + 0x2C].copy_from_slice(&4u32.to_le_bytes());
    }

    let mut bytes = 2u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&4u32.to_le_bytes());
    bytes.resize(crate::level::data_base(2), 0);
    bytes.extend_from_slice(&body);
    bytes
}

fn fixture(objects: &[(&str, [i16; 3])], models: &[&str]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut entries: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    entries.push((suffix("under1"), pcm_room("under1", objects)));
    for model in models {
        entries.push((suffix(model), named_model(model)));
    }
    let path = dir.path().join("BIGFILE.DAT");
    std::fs::write(&path, synthetic_archive(&entries)).expect("synthetic archive");
    (dir, path)
}

fn run_report(path: &Path, ticks: u32) -> HeadlessReport {
    let world = World::load(path, "under1").expect("the synthetic world must load");
    let mut frame = FrameState::new(world);
    headless_run(&mut frame, path, ticks).expect("the synthetic world must tick")
}

const GOLDEN_HEADLESS_DIGEST: u64 = 0x8dd6_3315_4e26_f1c4;

#[test]
fn windowed_boot_matches_the_headless_live_count_and_digest() {
    let (_dir, path) = fixture(
        &[
            ("raziel", [0, 0, 0]),
            ("capsule0", [10, 0, 0]),
            ("capsule1", [5000, 0, 0]),
            ("marker", [1, 0, 0]),
            ("ghost1", [4, 0, 0]),
        ],
        &["capsule0", "capsule1"],
    );

    // Headless reference: the production run seeds and ticks this input.
    let world = World::load(&path, "under1").expect("the synthetic world must load");
    let mut headless = FrameState::new(world);
    let report = headless_run(&mut headless, &path, 60).expect("headless run");

    // Windowed boot: the same shared boot function, then the same tick order.
    let world = World::load(&path, "under1").expect("the synthetic world must load");
    let mut windowed = FrameState::new(world);
    let counts = boot_world(&mut windowed, &path).expect("windowed boot");
    for index in 0..60u64 {
        windowed
            .run_tick(crate::engine::clock::ObjectTick::step(index))
            .expect("windowed tick");
    }

    assert_eq!(counts.placed, report.placed);
    assert_eq!(counts.admitted, report.admitted);
    assert_eq!(counts.skipped_markers, report.skipped_markers);
    assert_eq!(counts.inert, report.inert);
    assert_eq!(counts.unresolved, report.unresolved);
    assert_eq!(
        windowed.live().len(),
        report.live,
        "windowed boot must produce the same live count as headless"
    );
    assert_eq!(
        crate::engine::determinism::world_state_hash(&windowed.world_state()),
        report.digest,
        "windowed boot must produce the same canonical digest as headless"
    );
}

#[test]
fn headless_line_has_the_documented_contract_and_counts() {
    let (_dir, path) = fixture(
        &[
            ("raziel", [0, 0, 0]),
            ("capsule0", [10, 0, 0]),
            ("marker", [1, 0, 0]),
            ("marker", [2, 0, 0]),
            ("campath", [3, 0, 0]),
            ("ghost1", [4, 0, 0]),
        ],
        &["capsule0"],
    );
    let report = run_report(&path, 3);
    assert_eq!(report.unit, "under1");
    assert_eq!(report.rooms, 1);
    assert_eq!(report.placed, 6);
    assert_eq!(report.admitted, 3);
    assert_eq!(report.tracked, 2);
    assert_eq!(report.skipped_markers, 3);
    assert_eq!(report.inert, 0);
    assert_eq!(report.unresolved, 1);
    assert_eq!(report.live, 2);
    assert_eq!(report.ticks, 3);
    assert_eq!(
        report.placed,
        report.admitted + report.skipped_markers + report.inert,
        "the four counters partition the placed records"
    );
    assert!(
        report.tracked + report.unresolved <= report.admitted,
        "tracked and unresolved are distinct admitted names"
    );
    let expected = format!(
        "engine unit=under1 rooms=1 placed=6 admitted=3 tracked=2 skipped_markers=3 inert=0 unresolved=1 live=2 ticks=3 digest={:016x}",
        report.digest
    );
    assert_eq!(report.line(), expected);
}

#[test]
fn markers_never_enter_the_tracker() {
    let (_dir, path) = fixture(
        &[
            ("raziel", [0, 0, 0]),
            ("capsule0", [10, 0, 0]),
            ("marker", [1, 0, 0]),
            ("campath", [2, 0, 0]),
            ("splob", [3, 0, 0]),
        ],
        &["capsule0"],
    );
    let world = World::load(&path, "under1").expect("the synthetic world must load");
    let mut frame = FrameState::new(world);
    let report = headless_run(&mut frame, &path, 1).expect("the synthetic world must tick");
    assert_eq!(report.skipped_markers, 3);
    assert_eq!(report.tracked, 2);
    let names: Vec<&str> = frame
        .tracker()
        .slots()
        .iter()
        .filter_map(|slot| slot.name())
        .collect();
    for marker in ["marker", "campath", "splob"] {
        assert!(!names.contains(&marker), "{marker} must never be tracked");
    }
}

#[test]
fn the_49th_distinct_live_name_is_engine_exit_four() {
    let names: Vec<String> = (0..49).map(|index| format!("capsule{index:02}")).collect();
    let objects: Vec<(&str, [i16; 3])> = names
        .iter()
        .map(|name| (name.as_str(), [0, 0, 0]))
        .collect();
    let models: Vec<&str> = names.iter().map(String::as_str).collect();
    let (_dir, path) = fixture(&objects, &models);

    let world = World::load(&path, "under1").expect("the synthetic world must load");
    let mut frame = FrameState::new(world);
    let error = headless_run(&mut frame, &path, 1).expect_err("the tracker cap must bite");
    assert!(matches!(error, RunError::Invariant(_)));
    assert_eq!(error.exit_code(), 4);
    assert!(error.to_string().contains("MAX_OBJECTS=48"));
}

#[test]
fn the_same_run_twice_yields_the_same_digest_and_golden() {
    let (_dir, path) = fixture(
        &[
            ("raziel", [0, 0, 0]),
            ("capsule0", [10, 0, 0]),
            ("capsule1", [5000, 0, 0]),
            ("ghost1", [4, 0, 0]),
        ],
        &["capsule0", "capsule1"],
    );
    let first = run_report(&path, 60);
    let second = run_report(&path, 60);
    assert_eq!(first, second, "the whole acceptance report must replay");
    assert_eq!(first.digest, second.digest);
    assert_eq!(
        first.digest, GOLDEN_HEADLESS_DIGEST,
        "headless digest moved: admitted set, live order, fold or FNV changed (observed {:#018x})",
        first.digest
    );
}

#[test]
fn changing_the_tick_count_changes_the_digest() {
    let (_dir, path) = fixture(&[("raziel", [0, 0, 0])], &[]);
    let sixty = run_report(&path, 60);
    let sixty_one = run_report(&path, 61);
    assert_ne!(sixty.digest, sixty_one.digest);
    assert_eq!((sixty.ticks, sixty_one.ticks), (60, 61));
}

#[test]
fn fixed_step_bound_holds_on_the_synthetic_world() {
    let (_dir, path) = fixture(&[("raziel", [0, 0, 0])], &[]);
    let world = World::load(&path, "under1").expect("the synthetic world must load");
    let mut frame = FrameState::new(world);
    assert_eq!(
        frame.frame(Duration::from_secs(10)).expect("advance"),
        MAX_CATCH_UP as usize,
        "a ten-second stall is capped"
    );
    let mut total = MAX_CATCH_UP as usize;
    loop {
        let executed = frame.frame(Duration::ZERO).expect("drain");
        if executed == 0 {
            break;
        }
        total += executed;
    }
    assert_eq!(total, 600, "ten seconds at 60 Hz is 600 ticks, none lost");
    assert_eq!(frame.ticks(), 600);
}
