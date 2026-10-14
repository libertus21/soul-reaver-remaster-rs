//! DDS texture viewer (`dds-texture-viewer` + `bc7-cpu-decode` +
//! `texture-gallery` + `bigfile-index` + `models-3d`).
//!
//! Usage: `cargo run -- <path-to-DDS | path-to-directory>` — a file opens
//! a window showing that texture; a directory opens a gallery browsing
//! its sorted `*.DDS` files one at a time (ArrowRight/ArrowLeft, Esc to
//! exit). `cargo run -- --bigfile <BIGFILE.DAT> --list` prints the
//! archive index (one line per entry); `--show <index|hash>` decodes
//! that entry IN MEMORY (never written to disk) and shows it through
//! the same window when it is a DDS texture. `cargo run -- --textures
//! <TEXTURES.BIG> --tlist` prints the remaster textures-container
//! layout summary (a custom layout, NOT the SR1PC bigfile layout).
//! `cargo run -- --mesh <FILE.SRM>` parses one SR1-remaster mesh
//! (positions + triangle indices only) and shows it as a flat-shaded
//! auto-rotating solid (Esc or window close to exit). Game paths arrive via CLI
//! arg at runtime only and are never committed. Gallery mode skips
//! undecodable entries loud and continues; single-file and `--show`
//! modes die loud on the first error (an archive index must be exact,
//! not best-effort). Every [`dds::ViewerError`],
//! [`bigfile::BigfileError`], [`textures::TexturesError`], and
//! [`srm::SrmError`] maps to
//! a clear stderr message + `ExitCode(1)`; closing the window exits `0`.

mod bigfile;
mod dds;
mod gallery;
mod render;
// Transitional: Task 2 wires the mesh arm; until then the parser is
// test-only so dead-code warnings are allowed here, nowhere else.
#[allow(dead_code)]
mod srm;
mod textures;

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--bigfile") {
        return run_bigfile(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "--textures") {
        return run_textures(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "--mesh") {
        return run_mesh(&args[1..]);
    }
    let Some(arg) = args.first() else {
        eprintln!("usage: soul-reaver-remaster-rs <path-to-DDS | path-to-directory>");
        eprintln!("   or: soul-reaver-remaster-rs --bigfile <BIGFILE.DAT> --list");
        eprintln!("   or: soul-reaver-remaster-rs --bigfile <BIGFILE.DAT> --show <index|hash>");
        eprintln!("   or: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tlist");
        eprintln!("   or: soul-reaver-remaster-rs --mesh <FILE.SRM>");
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
    let texture = match dds::load_texture(&path) {
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

/// `--textures <path> --tlist`: no table is identified, so print the
/// layout summary (stdout) and exit 1 loud naming the next candidate.
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
    // Task-2 hunt verdict: no table identified (specified no-go). `--tlist`
    // carries the summary on stdout; `--tshow` reports the selector loud.
    // Both name the file and the next candidate on stderr and exit 1.
    if tlist {
        println!("{}", layout.summary());
    } else {
        eprintln!("error: {}", textures::TexturesError::BadSelector(args[2].clone()));
    }
    eprintln!(
        "error: no entry table identified in '{}' ({}; next candidate: {})",
        path.display(),
        layout.size_class(),
        textures::NEXT_CANDIDATE
    );
    ExitCode::from(1)
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

#[cfg(test)]
mod cli_tests {
    use super::run_mesh;
    use std::process::ExitCode;

    fn write_tmp(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> String {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).expect("synthetic mesh file must be writable");
        path.to_string_lossy().into_owned()
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
}
