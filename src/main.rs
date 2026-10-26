//! DDS texture viewer (`dds-texture-viewer` + `bc7-cpu-decode` +
//! `texture-gallery` + `bigfile-index` + `models-3d` + `mesh-textures`
//! + `mesh-batch`).
//!
//! Usage: `cargo run -- <path-to-DDS | path-to-directory>` — a file opens
//! a window showing that texture; a directory opens a gallery browsing
//! its sorted `*.DDS` files one at a time (ArrowRight/ArrowLeft, Esc to
//! exit). `cargo run -- --bigfile <BIGFILE.DAT> --list` prints the
//! archive index (one line per entry); `--show <index|hash>` decodes
//! that entry IN MEMORY (never written to disk) and shows it through
//! the same window when it is a DDS texture. `cargo run -- --textures
//! <TEXTURES.BIG> --tlist` parses the remaster `.BIG` header (a custom
//! layout, NOT the SR1PC bigfile layout), prints the header report, and
//! exits 0 when it validates; the entry table stays UNKNOWN.
//! `cargo run -- --mesh <FILE.SRM>` parses one SR1-remaster mesh
//! (positions + triangle indices only) and shows it as a flat-shaded
//! auto-rotating solid (Esc or window close to exit).
//! `cargo run -- --mesh-textured <FILE.SRM> --tex-dir <TEX>` parses the
//! same mesh plus UVs, material indices, and material records, loads
//! the three DDS textures per material from `<TEX>`, and shows it
//! textured and lit (Esc or window close to exit).
//! `cargo run -- --mesh-all <OBJECT_dir> --tex-dir <TEX>` browses every
//! `.SRM` in a directory as a textured mesh gallery (ArrowLeft /
//! ArrowRight step, Esc exits 0).
//! Game paths arrive via CLI arg at runtime only and are never
//! committed. Gallery mode skips undecodable entries loud and
//! continues; single-file and `--show` modes die loud on the first
//! error (an archive index must be exact, not best-effort). Every
//! [`dds::ViewerError`], [`bigfile::BigfileError`],
//! [`textures::TexturesError`], [`srm::SrmError`],
//! [`textures_object::TextureError`], and [`mesh_entry::MeshEntryError`]
//! maps to a clear stderr message + `ExitCode(1)`; closing the window
//! exits `0`.

mod bigfile;
// Launcher CLI arg parser (pure). Viewer only strips three no-op flags.
#[allow(dead_code)]
mod launcher_cli;
mod dds;
mod gallery;
mod mesh_entry;
mod render;
// Transitional: the `--tshow` arm wires this converter in a later commit;
// until then it is test-only, so dead-code warnings are allowed here.
#[allow(dead_code)]
mod slot_texture;
// Transitional: Task 2 wires the mesh arm; until then the parser is
// test-only so dead-code warnings are allowed here, nowhere else.
#[allow(dead_code)]
mod srm;
mod textures;
mod textures_object;

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    // Compatibility no-ops: stripped before dispatch, no output, no exit
    // change. (Parser-only flags are never wired into the viewer.)
    let args = strip_launcher_noops(raw);
    if args.first().is_some_and(|a| a == "--bigfile") {
        return run_bigfile(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "--textures") {
        return run_textures(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "--mesh") {
        return run_mesh(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "--mesh-textured") {
        return run_mesh_textured(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "--mesh-all") {
        return run_mesh_all(&args[1..]);
    }
    let Some(arg) = args.first() else {
        eprintln!("usage: soul-reaver-remaster-rs <path-to-DDS | path-to-directory>");
        eprintln!("   or: soul-reaver-remaster-rs --bigfile <BIGFILE.DAT> --list");
        eprintln!("   or: soul-reaver-remaster-rs --bigfile <BIGFILE.DAT> --show <index|hash>");
        eprintln!("   or: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tlist");
        eprintln!("   or: soul-reaver-remaster-rs --mesh <FILE.SRM>");
        eprintln!("   or: soul-reaver-remaster-rs --mesh-textured <FILE.SRM> --tex-dir <TEX>");
        eprintln!("   or: soul-reaver-remaster-rs --mesh-all <OBJECT_dir> --tex-dir <TEX>");
        eprintln!("Opens a window showing one .DDS texture file, or browses a");
        eprintln!("directory of .DDS files as a gallery (ArrowRight/ArrowLeft to");
        eprintln!("step, Esc or window close to exit).");
        return ExitCode::from(1);
    };
    if args.len() != 1 {
        eprintln!("error: expected exactly one path argument, got {}", args.len());
        return ExitCode::from(1);
    }
    let path = Path::new(&arg);
    if path.is_dir() {
        let entries = match gallery::collect_dds_entries(path) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(1);
            }
        };
        if let Err(e) = render::show_gallery(&entries) {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }
    if !path.is_file() {
        eprintln!("error: no such file or directory: {arg}");
        return ExitCode::from(1);
    }
    let texture = match dds::load_texture(path) {
        Ok(texture) => texture,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = show_texture(&texture) {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// Render one decoded texture through the EXISTING window (CPU BC7 or
/// RGBA8 upload). Shared tail of single-file mode and `--show`.
fn show_texture(texture: &dds::LoadedTexture) -> Result<(), String> {
    match texture {
        dds::LoadedTexture::Rgba8(image) => {
            render::show_rgba8(image.width, image.height, &image.rgba8)
        }
        dds::LoadedTexture::Bc7(compressed) => {
            let image = dds::decode_bc7_to_rgba8(compressed);
            render::show_rgba8(image.width, image.height, &image.rgba8)
        }
    }
}

/// `--bigfile <path> --list | --show <selector>`: index the SR1PC
/// archive and show ONE archived DDS texture from process memory.
/// Corrupt input, unknown selectors, and non-DDS entries fail LOUD
/// (stderr + exit 1); extracted bytes are never written anywhere.
fn run_bigfile(args: &[String]) -> ExitCode {
    let Some(path_arg) = args.first() else {
        eprintln!("usage: soul-reaver-remaster-rs --bigfile <BIGFILE.DAT> --list");
        eprintln!("   or: soul-reaver-remaster-rs --bigfile <BIGFILE.DAT> --show <index|hash>");
        return ExitCode::from(1);
    };
    let path = Path::new(path_arg);
    match args.get(1).map(String::as_str) {
        Some("--list") if args.len() == 2 => {
            let entries = match bigfile::load_index(path) {
                Ok(entries) => entries,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(1);
                }
            };
            for (index, entry) in entries.iter().enumerate() {
                println!(
                    "{index} {:08X} {} {}",
                    entry.name_hash, entry.offset, entry.length
                );
            }
            ExitCode::SUCCESS
        }
        Some("--show") if args.len() == 3 => {
            let entries = match bigfile::load_index(path) {
                Ok(entries) => entries,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(1);
                }
            };
            let selector = &args[2];
            let index = match bigfile::resolve_selector(&entries, selector) {
                Ok(index) => index,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(1);
                }
            };
            let entry = entries[index];
            let bytes = match bigfile::read_entry(path, &entry) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(1);
                }
            };
            if !dds::has_dds_magic(&bytes) {
                eprintln!(
                    "error: {}",
                    bigfile::BigfileError::NotDds(format!(
                        "entry {index} (hash {:08X}) is not a DDS texture (listed, not decoded)",
                        entry.name_hash
                    ))
                );
                return ExitCode::from(1);
            }
            let texture = match dds::decode_memory(&bytes) {
                Ok(texture) => texture,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(1);
                }
            };
            if let Err(e) = show_texture(&texture) {
                eprintln!("error: {e}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: soul-reaver-remaster-rs --bigfile <BIGFILE.DAT> --list");
            eprintln!("   or: soul-reaver-remaster-rs --bigfile <BIGFILE.DAT> --show <index|hash>");
            eprintln!("`--list` prints the entry index (`index hash offset length` per line);");
            eprintln!("`--show` renders one DDS entry in a window (close/Esc exits 0).");
            ExitCode::from(1)
        }
    }
}

/// `--textures <path> --tlist`: a valid header and exact slot geometry
/// print the preserved header report plus one class line per slot and a
/// per-class footer (stdout, exit 0). `--tshow <i>` validates the selector,
/// reads that one slot, and stays loud (exit 1): payload compression is
/// UNKNOWN, so nothing is decoded and no window opens.
fn run_textures(args: &[String]) -> ExitCode {
    let Some(path_arg) = args.first() else {
        eprintln!("usage: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tlist");
        return ExitCode::from(1);
    };
    let path = Path::new(path_arg);
    let tlist = matches!(args.get(1).map(String::as_str), Some("--tlist") if args.len() == 2);
    let tshow = matches!(args.get(1).map(String::as_str), Some("--tshow") if args.len() == 3);
    if !tlist && !tshow {
        eprintln!("usage: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tlist");
        eprintln!("   or: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tshow <entry>");
        return ExitCode::from(1);
    }
    let layout = match textures::describe(path) {
        Ok(layout) => layout,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    if tlist {
        println!("{}", layout.summary());
        return match textures::scan_slots(path, &layout.header) {
            Ok(scan) => {
                println!("{}", scan.report());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(1)
            }
        };
    }
    let selector = &args[2];
    let index = match selector.parse::<usize>() {
        Ok(index) => index,
        Err(_) => {
            eprintln!(
                "error: {}",
                textures::TexturesError::SlotSelectorInvalid {
                    selector: selector.clone(),
                    count: layout.header.entry_count,
                }
            );
            return ExitCode::from(1);
        }
    };
    if index >= usize::from(layout.header.entry_count) {
        eprintln!(
            "error: {}",
            textures::TexturesError::SlotIndexOutOfRange {
                index,
                count: layout.header.entry_count,
            }
        );
        return ExitCode::from(1);
    }
    match textures::slot_class_at(path, &layout.header, index as u16) {
        Ok((offset, class)) => {
            eprintln!(
                "error: {}",
                textures::TexturesError::SlotNotDecodable {
                    index: index as u16,
                    offset,
                    class,
                }
            );
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

/// `--mesh <FILE.SRM>`: parse one SR1-remaster mesh (positions +
/// triangle indices only, bytes stay in process memory) and show it as
/// a flat-shaded auto-rotating solid. Any parse failure is a loud
/// stderr error + exit 1 with no window; Esc/window close exits 0.
fn run_mesh(args: &[String]) -> ExitCode {
    if args.len() != 1 {
        eprintln!("usage: soul-reaver-remaster-rs --mesh <FILE.SRM>");
        if args.is_empty() {
            eprintln!("error: --mesh expects exactly one file argument, got 0");
        } else {
            eprintln!("error: --mesh expects exactly one file argument, got {}", args.len());
        }
        return ExitCode::from(1);
    }
    let path = Path::new(&args[0]);
    if !path.is_file() {
        eprintln!("error: no such file: {}", path.display());
        return ExitCode::from(1);
    }
    let mesh = match srm::load(path) {
        Ok(mesh) => mesh,
        Err(e) => {
            eprintln!("error: {}: {e}", path.display());
            return ExitCode::from(1);
        }
    };
    println!(
        "{} vertices, {} triangles",
        mesh.positions.len(),
        mesh.indices.len() / 3
    );
    let title = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    if let Err(e) = render::show_mesh(&mesh.positions, &mesh.indices, &title) {
        eprintln!("error: {}: {e}", path.display());
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// `--mesh-textured <FILE.SRM> --tex-dir <TEX>`: parse one SR1-remaster
/// mesh (positions + UVs + material indices + material records +
/// triangle indices), load the three DDS textures per material from
/// `<TEX>`, and show it textured and lit. Any parse/load failure is a
/// loud stderr error + exit 1 with no window; Esc/window close exits 0.
fn run_mesh_textured(args: &[String]) -> ExitCode {
    if args.len() != 3 || args[1] != "--tex-dir" {
        eprintln!("usage: soul-reaver-remaster-rs --mesh-textured <FILE.SRM> --tex-dir <TEX>");
        eprintln!(
            "error: --mesh-textured expects exactly <SRM> --tex-dir <TEX>, got {} arg(s)",
            args.len()
        );
        return ExitCode::from(1);
    }
    let srm_path = Path::new(&args[0]);
    let tex_dir = Path::new(&args[2]);
    if !srm_path.is_file() {
        eprintln!("error: no such file: {}", srm_path.display());
        return ExitCode::from(1);
    }
    if !tex_dir.is_dir() {
        eprintln!("error: no such directory: {}", tex_dir.display());
        return ExitCode::from(1);
    }
    let entry = match mesh_entry::load(srm_path, tex_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {}: {e}", srm_path.display());
            return ExitCode::from(1);
        }
    };
    println!(
        "{} vertices, {} triangles, {} materials",
        entry.positions.len(),
        entry.indices.len() / 3,
        entry.materials.len()
    );
    if let Err(e) = render::show_mesh_textured(
        &entry.positions,
        &entry.uvs,
        &entry.indices,
        &entry.material_indices,
        &entry.materials,
        &entry.textures,
        &entry.title,
    ) {
        eprintln!("error: {}: {e}", srm_path.display());
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// `--mesh-all <OBJECT_dir> --tex-dir <TEX>`: browse every `.SRM` in a
/// directory as a textured mesh gallery. ArrowLeft / ArrowRight step
/// (clamped, no wrap), unloadable entries are skipped LOUD, Esc / close
/// exits 0. Zero loadable entries exits 1 with no window.
fn run_mesh_all(args: &[String]) -> ExitCode {
    if args.len() != 3 || args[1] != "--tex-dir" {
        eprintln!("usage: soul-reaver-remaster-rs --mesh-all <OBJECT_dir> --tex-dir <TEX>");
        eprintln!(
            "error: --mesh-all expects exactly <OBJECT_dir> --tex-dir <TEX>, got {} arg(s)",
            args.len()
        );
        return ExitCode::from(1);
    }
    let obj_dir = Path::new(&args[0]);
    let tex_dir = Path::new(&args[2]);
    if !obj_dir.is_dir() {
        eprintln!("error: no such directory: {}", obj_dir.display());
        return ExitCode::from(1);
    }
    if !tex_dir.is_dir() {
        eprintln!("error: no such directory: {}", tex_dir.display());
        return ExitCode::from(1);
    }
    let entries = match srm::collect_srm_entries(obj_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {}: {e}", obj_dir.display());
            return ExitCode::from(1);
        }
    };
    println!(
        "{} .SRM files in {}",
        entries.len(),
        obj_dir.display()
    );
    if let Err(e) = render::show_mesh_gallery(&entries, tex_dir) {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// Strip launcher compatibility no-op flags before dispatch. Pure helper,
/// testable without spawning a process or opening a window.
fn strip_launcher_noops(args: Vec<String>) -> Vec<String> {
    args.into_iter()
        .filter(|a| {
            !matches!(
                a.as_str(),
                "--nosteam" | "-nosteam" | "-nocheck" | "--nocheck" | "-skipintro" | "--skipintro"
            )
        })
        .collect()
}

#[cfg(test)]
mod cli_tests {
    use super::{
        run_bigfile, run_mesh, run_mesh_all, run_mesh_textured, run_textures,
        strip_launcher_noops,
    };
    use std::process::ExitCode;

    fn write_tmp(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> String {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).expect("synthetic mesh file must be writable");
        path.to_string_lossy().into_owned()
    }

    /// Synthetic `.BIG` with EXACT slot geometry: two header words then
    /// zeros in `4096 + count * 131072` bytes. Values are invented.
    fn synthetic_big(header_size: u16, count: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 4096 + usize::from(count) * 131072];
        bytes[0..2].copy_from_slice(&header_size.to_le_bytes());
        bytes[2..4].copy_from_slice(&count.to_le_bytes());
        bytes
    }

    /// Header-only bytes for cases rejected before geometry validation.
    fn synthetic_header(header_size: u16, count: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 1024];
        bytes[0..2].copy_from_slice(&header_size.to_le_bytes());
        bytes[2..4].copy_from_slice(&count.to_le_bytes());
        bytes
    }

    #[test]
    fn textures_tlist_valid_header_exits_0() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "valid.big", &synthetic_big(512, 3));
        assert_eq!(
            run_textures(&[big, "--tlist".to_string()]),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn textures_tlist_each_invalid_header_class_exits_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let truncated = write_tmp(&tmp, "trunc.big", &[0u8; 100]);
        assert_eq!(
            run_textures(&[truncated, "--tlist".to_string()]),
            ExitCode::from(1)
        );
        let wrong = write_tmp(&tmp, "wrong.big", &synthetic_header(256, 3));
        assert_eq!(
            run_textures(&[wrong, "--tlist".to_string()]),
            ExitCode::from(1)
        );
        let zero = write_tmp(&tmp, "zero.big", &synthetic_header(512, 0));
        assert_eq!(
            run_textures(&[zero, "--tlist".to_string()]),
            ExitCode::from(1)
        );
        let huge = write_tmp(&tmp, "huge.big", &synthetic_header(512, 32769));
        assert_eq!(
            run_textures(&[huge, "--tlist".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn textures_tlist_geometry_mismatch_exits_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let exact = synthetic_big(512, 2);
        let mut short = exact.clone();
        short.pop();
        let mut long = exact;
        long.push(0);
        for (name, bytes) in [("short.big", short), ("long.big", long)] {
            let path = write_tmp(&tmp, name, &bytes);
            assert_eq!(
                run_textures(&[path.clone(), "--tlist".to_string()]),
                ExitCode::from(1)
            );
            // Header/geometry errors win over any selector problem.
            assert_eq!(
                run_textures(&[path, "--tshow".to_string(), "0".to_string()]),
                ExitCode::from(1)
            );
        }
    }

    #[test]
    fn textures_tlist_eight_slots_exits_0() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "eight.big", &synthetic_big(512, 8));
        assert_eq!(
            run_textures(&[big, "--tlist".to_string()]),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn textures_tshow_rejects_selector_range_and_decodes_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "valid.big", &synthetic_big(512, 3));
        assert_eq!(
            run_textures(&[big.clone(), "--tshow".to_string(), "abc".to_string()]),
            ExitCode::from(1)
        );
        assert_eq!(
            run_textures(&[big.clone(), "--tshow".to_string(), "3".to_string()]),
            ExitCode::from(1)
        );
        // Valid index still fails loud: compression is UNKNOWN.
        assert_eq!(
            run_textures(&[big, "--tshow".to_string(), "0".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn textures_tshow_stays_loud_exit_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let valid = write_tmp(&tmp, "valid.big", &synthetic_big(512, 3));
        assert_eq!(
            run_textures(&[valid, "--tshow".to_string(), "0".to_string()]),
            ExitCode::from(1)
        );
        let invalid = write_tmp(&tmp, "invalid.big", &synthetic_header(256, 3));
        assert_eq!(
            run_textures(&[invalid, "--tshow".to_string(), "0".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn textures_usage_errors_exit_1() {
        assert_eq!(run_textures(&[]), ExitCode::from(1));
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "valid.big", &synthetic_big(512, 3));
        assert_eq!(run_textures(&[big.clone()]), ExitCode::from(1));
        assert_eq!(
            run_textures(&[big.clone(), "--tlist".to_string(), "extra".to_string()]),
            ExitCode::from(1)
        );
        assert_eq!(
            run_textures(&[big, "--unknown".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn bigfile_list_missing_and_truncated_files_exit_1() {
        // Failure paths only — no window opens. Synthetic bytes in a
        // temp dir; the real archive is never touched.
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let absent = tmp.path().join("absent.big").to_string_lossy().into_owned();
        assert_eq!(run_bigfile(&[absent, "--list".to_string()]), ExitCode::from(1));
        let trunc = tmp.path().join("trunc.big");
        std::fs::write(&trunc, [0x02u8, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04])
            .expect("synthetic file must be writable");
        assert_eq!(
            run_bigfile(&[trunc.to_string_lossy().into_owned(), "--list".to_string()]),
            ExitCode::from(1)
        );
        // S2 HELD rows keep the loud-nonzero contract on the --show path too.
        let absent_show = tmp.path().join("absent-show.big").to_string_lossy().into_owned();
        assert_eq!(
            run_bigfile(&[absent_show, "--show".to_string(), "0".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn mesh_missing_and_extra_args_are_loud_exit_1() {
        assert_eq!(run_mesh(&[]), ExitCode::from(1));
        assert_eq!(run_mesh(&["a.srm".to_string(), "b.srm".to_string()]), ExitCode::from(1));
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let absent = tmp.path().join("absent.srm").to_string_lossy().into_owned();
        assert_eq!(run_mesh(&[absent]), ExitCode::from(1));
    }

    #[test]
    fn mesh_bad_magic_and_truncated_files_are_loud_exit_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let bad = write_tmp(&tmp, "bad.srm", b"NOPE-not-a-mesh-at-all-....");
        assert_eq!(run_mesh(&[bad]), ExitCode::from(1));
        let mut short = b"SRM\x01".to_vec();
        short.extend_from_slice(&[0u8; 8]);
        let cut = write_tmp(&tmp, "cut.srm", &short);
        assert_eq!(run_mesh(&[cut]), ExitCode::from(1));
    }

    #[test]
    fn mesh_textured_wrong_arg_count_or_order_exit_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let srm = write_tmp(&tmp, "x.srm", b"NOPE");
        assert_eq!(run_mesh_textured(&[srm.clone()]), ExitCode::from(1));
        assert_eq!(
            run_mesh_textured(&[srm.clone(), "--wrong".to_string(), tmp.path().to_string_lossy().into_owned()]),
            ExitCode::from(1)
        );
        assert_eq!(
            run_mesh_textured(&[srm.clone(), "--tex-dir".to_string(), tmp.path().to_string_lossy().into_owned(), "extra".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn mesh_textured_missing_files_exit_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let absent_srm = tmp.path().join("absent.srm").to_string_lossy().into_owned();
        let tex_dir = tmp.path().to_string_lossy().into_owned();
        assert_eq!(
            run_mesh_textured(&[absent_srm, "--tex-dir".to_string(), tex_dir]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn mesh_textured_missing_tex_dir_exit_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let srm = write_tmp(&tmp, "x.srm", b"NOPE");
        let absent_dir = tmp.path().join("no-such-dir").to_string_lossy().into_owned();
        assert_eq!(
            run_mesh_textured(&[srm, "--tex-dir".to_string(), absent_dir]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn mesh_all_wrong_args_exit_1() {
        assert_eq!(run_mesh_all(&[]), ExitCode::from(1));
        assert_eq!(run_mesh_all(&["dir".to_string()]), ExitCode::from(1));
        assert_eq!(
            run_mesh_all(&["dir".to_string(), "wrong".to_string(), "tex".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn mesh_all_missing_dirs_exit_1() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let real = tmp.path().to_string_lossy().into_owned();
        let absent = tmp
            .path()
            .join("no-such")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            run_mesh_all(&[absent.clone(), "--tex-dir".into(), real.clone()]),
            ExitCode::from(1)
        );
        assert_eq!(
            run_mesh_all(&[real, "--tex-dir".into(), absent]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn mesh_all_empty_srm_dir_exit_1() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        let dir = tmp.path().to_string_lossy().into_owned();
        assert_eq!(
            run_mesh_all(&[dir.clone(), "--tex-dir".into(), dir]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn strip_noops_removes_compat_flags() {
        let input = vec![
            "--nosteam".to_string(),
            "-nocheck".to_string(),
            "-skipintro".to_string(),
            "--bigfile".to_string(),
            "x.big".to_string(),
        ];
        assert_eq!(
            strip_launcher_noops(input),
            vec!["--bigfile".to_string(), "x.big".to_string()]
        );
    }

    #[test]
    fn strip_noops_preserves_unknown_and_paths() {
        let input = vec!["foo.dds".to_string(), "-foo".to_string()];
        assert_eq!(strip_launcher_noops(input.clone()), input);
    }
}