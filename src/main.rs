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
//! exits 0 when it validates; the entry table stays UNKNOWN. `--tshow <i>`
//! decodes one slot as its raw 256x256 A1R5G5B5 image (headerless, not
//! compressed) and shows it; `empty`/`zero` slots stay loud. A trailing
//! `--map <TEX_HD.MAP>` pair adds the engine's alias names to `--tlist`
//! lines and lets `--tshow` accept a texture name, refusing to guess when a
//! name is shared.
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
mod cli_out;
// Launcher CLI arg parser (pure). Viewer only strips three no-op flags.
#[allow(dead_code)]
mod launcher_cli;
mod dds;
mod gallery;
mod mesh_entry;
mod render;
mod slot_texture;
mod tex_hd_map;
// Transitional: Task 2 wires the mesh arm; until then the parser is
// test-only so dead-code warnings are allowed here, nowhere else.
#[allow(dead_code)]
mod srm;
// Transitional: the `--audio` arm lands next; until then the parser is
// test-only so dead-code warnings are allowed here, nowhere else.
#[allow(dead_code)]
mod snd;
mod textures;
mod textures_object;

use std::path::Path;
use std::process::ExitCode;

use cli_out::outln;

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
        eprintln!("   or: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tlist --map <TEX_HD.MAP>");
        eprintln!("   or: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tshow <entry|name>");
        eprintln!("   or: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tshow <entry|name> --map <TEX_HD.MAP>");
        eprintln!("   or: soul-reaver-remaster-rs --mesh <FILE.SRM>");
        eprintln!("   or: soul-reaver-remaster-rs --mesh-textured <FILE.SRM> --tex-dir <TEX>");
        eprintln!("   or: soul-reaver-remaster-rs --mesh-all <OBJECT_dir> --tex-dir <TEX>");
        eprintln!("`--textures ... --tlist` lists slot classes; `--tshow <entry>` shows");
        eprintln!("one slot's raw 256x256 A1R5G5B5 image.");
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
                outln!(
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
/// reads exactly that one slot, and shows a `data` payload as raw 256x256
/// A1R5G5B5 converted to RGBA8 through the existing window (close/Esc exits
/// 0); `empty`/`zero` slots stay loud (exit 1) with no window.
/// The optional trailing `--map <TEX_HD.MAP>` pair adds the engine's slot
/// names (`--tlist`) or a name selector (`--tshow`); everything without it
/// stays byte-for-byte unchanged.
fn run_textures(args: &[String]) -> ExitCode {
    let Some(path_arg) = args.first() else {
        eprintln!("usage: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tlist");
        return ExitCode::from(1);
    };
    let path = Path::new(path_arg);
    let tlist = matches!(args.get(1).map(String::as_str), Some("--tlist") if args.len() == 2 || (args.len() == 4 && args.get(2).map(String::as_str) == Some("--map")));
    let tshow = matches!(args.get(1).map(String::as_str), Some("--tshow") if args.len() == 3 || (args.len() == 5 && args.get(3).map(String::as_str) == Some("--map")));
    if !tlist && !tshow {
        eprintln!("usage: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tlist");
        eprintln!("   or: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tlist --map <TEX_HD.MAP>");
        eprintln!("   or: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tshow <entry|name>");
        eprintln!("   or: soul-reaver-remaster-rs --textures <TEXTURES.BIG> --tshow <entry|name> --map <TEX_HD.MAP>");
        return ExitCode::from(1);
    }
    let map_arg = match (args.get(1).map(String::as_str), args.len()) {
        (Some("--tlist"), 4) => Some(Path::new(&args[3])),
        (Some("--tshow"), 5) => Some(Path::new(&args[4])),
        _ => None,
    };
    let layout = match textures::describe(path) {
        Ok(layout) => layout,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let map_context = match map_arg {
        Some(map_path) => match load_map(map_path, path) {
            Ok(context) => Some(context),
            Err(message) => {
                eprintln!("error: {message}");
                return ExitCode::from(1);
            }
        },
        None => None,
    };
    if tlist {
        outln!("{}", layout.summary());
        let scan = match textures::scan_slots(path, &layout.header) {
            Ok(scan) => scan,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(1);
            }
        };
        let named = map_context.as_ref().map(|(bytes, group)| {
            let map = tex_hd_map::TextureMap::parse(bytes)
                .expect("map length was validated when the context was loaded");
            (map, *group)
        });
        return match list_report(&scan, named.as_ref()) {
            Ok(report) => {
                outln!("{report}");
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
            if let Some((bytes, group)) = map_context.as_ref() {
                let map = tex_hd_map::TextureMap::parse(bytes)
                    .expect("map length was validated when the context was loaded");
                match resolve_texture_name(
                    &map,
                    *group,
                    layout.header.entry_count,
                    path,
                    selector,
                ) {
                    Ok(index) => index,
                    Err(message) => {
                        eprintln!("error: {message}");
                        return ExitCode::from(1);
                    }
                }
            } else if tex_hd_map::TextureName::parse(selector).is_some() {
                eprintln!("error: texture names require --map <TEX_HD.MAP>");
                return ExitCode::from(1);
            } else {
                eprintln!(
                    "error: {}",
                    textures::TexturesError::SlotSelectorInvalid {
                        selector: selector.clone(),
                        count: layout.header.entry_count,
                    }
                );
                return ExitCode::from(1);
            }
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
    let name_suffix = match map_context.as_ref() {
        Some((bytes, group)) => {
            let map = tex_hd_map::TextureMap::parse(bytes)
                .expect("map length was validated when the context was loaded");
            let slot = match u16::try_from(index) {
                Ok(slot) => slot,
                Err(_) => {
                    eprintln!(
                        "error: {}",
                        tex_hd_map::MapError::SlotOutOfCapacity {
                            slot: u16::MAX,
                            group: *group
                        }
                    );
                    return ExitCode::from(1);
                }
            };
            match tshow_suffix(&map, *group, slot) {
                Ok(suffix) => suffix,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(1);
                }
            }
        }
        None => String::new(),
    };
    match textures::decode_slot_at(path, &layout.header, index as u16) {
        Ok((offset, rgba)) => {
            outln!(
                "slot {index} (offset {offset}) decoded: {}x{} A1R5G5B5 -> RGBA8; showing image{name_suffix}",
                slot_texture::WIDTH,
                slot_texture::HEIGHT,
            );
            match render::show_rgba8(
                slot_texture::WIDTH as u32,
                slot_texture::HEIGHT as u32,
                &rgba,
            ) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

/// Read and validate the explicit `--map` path, then select the block from
/// the container stem. Wrong length, unreadable files, and unknown stems are
/// loud; no implicit discovery happens anywhere.
fn load_map(
    map_path: &Path,
    container: &Path,
) -> Result<(Vec<u8>, tex_hd_map::Group), String> {
    let bytes = std::fs::read(map_path)
        .map_err(|e| format!("cannot read TEX_HD.MAP: {e}"))?;
    tex_hd_map::TextureMap::parse(&bytes).map_err(|e| e.to_string())?;
    let stem = container
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let group = tex_hd_map::Group::from_container_stem(&stem).ok_or_else(|| {
        format!(
            "cannot map names for '{}': expected TEXTURES.BIG, MOVIES.BIG, or BONUS.BIG",
            file_label(container)
        )
    })?;
    Ok((bytes, group))
}

/// Container name for messages (never an absolute install path).
fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// `--tlist` body: the preserved report, with `; names ...` appended to
/// each slot line when a map is present. Without a map this is exactly
/// `scan.report()`.
fn list_report(
    scan: &textures::SlotScan,
    named: Option<&(tex_hd_map::TextureMap<'_>, tex_hd_map::Group)>,
) -> Result<String, tex_hd_map::MapError> {
    let Some((map, group)) = named else {
        return Ok(scan.report());
    };
    let mut suffixes = Vec::with_capacity(scan.classes.len());
    for slot in 0..scan.classes.len() {
        let slot16 = u16::try_from(slot).unwrap_or(u16::MAX);
        let names = map.names(*group, slot16)?;
        suffixes.push(join_names(&names));
    }
    Ok(append_name_suffixes(&scan.report(), &suffixes))
}

/// Display form of one slot's names: deduped comma list, or `-` when the
/// row is all sentinel.
fn join_names(names: &[tex_hd_map::TextureName]) -> String {
    if names.is_empty() {
        return "-".to_string();
    }
    names
        .iter()
        .map(tex_hd_map::TextureName::display)
        .collect::<Vec<_>>()
        .join(",")
}

/// Append `; names <suffix>` to every per-slot line, leaving the addressing
/// line, summary, and footer untouched.
fn append_name_suffixes(report: &str, suffixes: &[String]) -> String {
    let mut out = String::with_capacity(report.len() + suffixes.len() * 16);
    let mut slot = 0usize;
    for (line_index, line) in report.lines().enumerate() {
        if line_index > 0 {
            out.push('\n');
        }
        out.push_str(line);
        if line.starts_with("slot ") {
            let suffix = suffixes.get(slot).map(String::as_str).unwrap_or("-");
            out.push_str("; names ");
            out.push_str(suffix);
            slot += 1;
        }
    }
    out
}

/// Resolve a `--tshow` texture-name selector to exactly one slot. Zero
/// matches and shared names are loud; the first match is never chosen
/// silently.
fn resolve_texture_name(
    map: &tex_hd_map::TextureMap<'_>,
    group: tex_hd_map::Group,
    slot_count: u16,
    container: &Path,
    selector: &str,
) -> Result<usize, String> {
    let name = tex_hd_map::TextureName::parse(selector).ok_or_else(|| {
        format!(
            "invalid texture name '{selector}': expected <id>_<sub>, M\\<id>_<sub>, or B\\<id>_<sub>"
        )
    })?;
    let matches = map.slots_for_name(group, slot_count, &name);
    match matches.as_slice() {
        [] => Err(format!(
            "texture name '{}' is not referenced by any slot in {}",
            name.display(),
            file_label(container)
        )),
        [only] => Ok(usize::from(*only)),
        many => {
            let shown = many
                .iter()
                .take(8)
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "texture name '{}' is referenced by {} slots (e.g. {shown}); select one by index",
                name.display(),
                many.len()
            ))
        }
    }
}

/// `--tshow <index> --map` stdout suffix for one row.
fn tshow_suffix(
    map: &tex_hd_map::TextureMap<'_>,
    group: tex_hd_map::Group,
    slot: u16,
) -> Result<String, tex_hd_map::MapError> {
    let names = map.names(group, slot)?;
    Ok(format!("; names {}", join_names(&names)))
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
    outln!(
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
    outln!(
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
    outln!(
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
        list_report, resolve_texture_name, run_bigfile, run_mesh, run_mesh_all,
        run_mesh_textured, run_textures, strip_launcher_noops, tshow_suffix,
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

    /// All-sentinel `TEX_HD.MAP` (every slot "no reference" until overridden).
    fn synthetic_map() -> Vec<u8> {
        vec![0xFF; crate::tex_hd_map::MAP_BYTES]
    }

    /// Write one 16-code row of the synthetic map; missing codes stay sentinel.
    fn put_map_row(map: &mut [u8], row: usize, codes: &[u32]) {
        let start = row * crate::tex_hd_map::ROW_BYTES;
        for i in 0..crate::tex_hd_map::ROW_CODES {
            let code = codes
                .get(i)
                .copied()
                .unwrap_or(crate::tex_hd_map::SENTINEL);
            let at = start + i * 4;
            map[at..at + 4].copy_from_slice(&code.to_le_bytes());
        }
    }

    #[test]
    fn list_report_with_map_appends_names_and_keeps_the_footer() {
        use crate::tex_hd_map::{Group, TextureMap};
        use crate::textures::{SlotClass, SlotScan};

        let mut bytes = synthetic_map();
        put_map_row(&mut bytes, 0, &[0, 32_000, 64_000, 0]);
        let map = TextureMap::parse(&bytes).expect("synthetic map must parse");
        let scan = SlotScan {
            classes: vec![SlotClass::Data, SlotClass::Zero, SlotClass::Data],
        };
        let report = list_report(&scan, Some(&(map, Group::Textures)))
            .expect("row reads must be in capacity");
        assert!(
            report.contains("slot 0 offset 4096 data; names 0_0,M\\0_0,B\\0_0"),
            "got: {report}"
        );
        assert!(
            report.contains("slot 1 offset 135168 zero; names -"),
            "got: {report}"
        );
        assert!(
            report.contains("slots: 3 total, 0 empty, 1 zero, 2 data"),
            "got: {report}"
        );
        assert!(
            report.starts_with("addressing: fixed 131072-byte slots"),
            "got: {report}"
        );
        assert_eq!(
            list_report(&scan, None).expect("no-map report is infallible"),
            scan.report(),
            "without a map the body must be byte-for-byte unchanged"
        );
    }

    #[test]
    fn list_report_with_map_never_writes_an_out_of_capacity_row() {
        use crate::tex_hd_map::{Group, MapError, TextureMap};
        use crate::textures::{SlotClass, SlotScan};

        let bytes = synthetic_map();
        let map = TextureMap::parse(&bytes).expect("synthetic map must parse");
        let scan = SlotScan {
            classes: vec![SlotClass::Data; crate::tex_hd_map::GROUP_ROWS + 1],
        };
        let err = list_report(&scan, Some(&(map, Group::Bonus)))
            .expect_err("row 2000 must be rejected");
        assert_eq!(
            err,
            MapError::SlotOutOfCapacity {
                slot: 2_000,
                group: Group::Bonus
            }
        );
    }

    #[test]
    fn textures_tlist_with_map_names_a_valid_container() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "TEXTURES.BIG", &synthetic_big(512, 3));
        let mut map_bytes = synthetic_map();
        put_map_row(&mut map_bytes, 1, &[16]);
        let map = write_tmp(&tmp, "TEX_HD.MAP", &map_bytes);
        assert_eq!(
            run_textures(&[big, "--tlist".to_string(), "--map".to_string(), map]),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn textures_tlist_with_bad_map_length_exits_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "TEXTURES.BIG", &synthetic_big(512, 2));
        for len in [
            crate::tex_hd_map::MAP_BYTES - 1,
            crate::tex_hd_map::MAP_BYTES + 1,
        ] {
            let map = write_tmp(&tmp, "TEX_HD.MAP", &vec![0xFF; len]);
            assert_eq!(
                run_textures(&[big.clone(), "--tlist".to_string(), "--map".to_string(), map]),
                ExitCode::from(1)
            );
        }
    }

    #[test]
    fn textures_tlist_with_map_unknown_stem_exits_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "E3.BIG", &synthetic_big(512, 2));
        let map = write_tmp(&tmp, "TEX_HD.MAP", &synthetic_map());
        assert_eq!(
            run_textures(&[big, "--tlist".to_string(), "--map".to_string(), map]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn textures_tlist_with_missing_map_exits_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "TEXTURES.BIG", &synthetic_big(512, 2));
        let absent = tmp
            .path()
            .join("absent.map")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            run_textures(&[big, "--tlist".to_string(), "--map".to_string(), absent]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn resolve_texture_name_is_unique_or_loud() {
        use crate::tex_hd_map::{Group, TextureMap};
        use std::path::Path;

        let mut bytes = synthetic_map();
        put_map_row(&mut bytes, 1, &[64]);
        put_map_row(&mut bytes, 2, &[64]);
        put_map_row(&mut bytes, 3, &[32_016]);
        put_map_row(&mut bytes, 4, &[80]);
        let map = TextureMap::parse(&bytes).expect("synthetic map must parse");
        let container = Path::new("TEXTURES.BIG");

        assert_eq!(
            resolve_texture_name(&map, Group::Textures, 8, container, "5_0"),
            Ok(4)
        );
        assert_eq!(
            resolve_texture_name(&map, Group::Textures, 8, container, "m\\1_0"),
            Ok(3),
            "bank prefixes are case-insensitive"
        );
        let ambiguous =
            resolve_texture_name(&map, Group::Textures, 8, container, "4_0")
                .expect_err("a shared name must never resolve silently");
        assert!(
            ambiguous.contains("2 slots")
                && ambiguous.contains("1, 2")
                && ambiguous.contains("select one by index"),
            "got: {ambiguous}"
        );
        let unknown =
            resolve_texture_name(&map, Group::Textures, 8, container, "9_9")
                .expect_err("an absent name must be loud");
        assert!(
            unknown.contains("not referenced") && unknown.contains("TEXTURES.BIG"),
            "got: {unknown}"
        );
        let invalid =
            resolve_texture_name(&map, Group::Textures, 8, container, "abc")
                .expect_err("invalid syntax must be loud");
        assert!(
            invalid.contains("invalid texture name 'abc'")
                && invalid.contains("<id>_<sub>"),
            "got: {invalid}"
        );
    }

    #[test]
    fn resolve_texture_name_lists_at_most_eight_candidates() {
        use crate::tex_hd_map::{Group, TextureMap};
        use std::path::Path;

        let mut bytes = synthetic_map();
        for row in 0..10 {
            put_map_row(&mut bytes, row, &[16]);
        }
        let map = TextureMap::parse(&bytes).expect("synthetic map must parse");
        let err = resolve_texture_name(&map, Group::Textures, 12, Path::new("TEXTURES.BIG"), "1_0")
            .expect_err("ten matches must be loud");
        assert!(
            err.contains("referenced by 10 slots")
                && err.contains("0, 1, 2, 3, 4, 5, 6, 7"),
            "got: {err}"
        );
    }

    #[test]
    fn tshow_suffix_formats_names_or_dash() {
        use crate::tex_hd_map::{Group, TextureMap};

        let mut bytes = synthetic_map();
        put_map_row(&mut bytes, 0, &[0, 32_000, 64_000, 0]);
        let map = TextureMap::parse(&bytes).expect("synthetic map must parse");
        assert_eq!(
            tshow_suffix(&map, Group::Textures, 0).expect("row must read"),
            "; names 0_0,M\\0_0,B\\0_0"
        );
        assert_eq!(
            tshow_suffix(&map, Group::Textures, 1).expect("row must read"),
            "; names -"
        );
    }

    #[test]
    fn textures_tshow_name_without_map_exits_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "TEXTURES.BIG", &synthetic_big(512, 3));
        assert_eq!(
            run_textures(&[big.clone(), "--tshow".to_string(), "4_0".to_string()]),
            ExitCode::from(1)
        );
        // A selector that is neither an index nor a name keeps its message.
        assert_eq!(
            run_textures(&[big, "--tshow".to_string(), "abc".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn textures_tshow_name_with_map_resolves_before_any_window() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "TEXTURES.BIG", &synthetic_big(512, 3));
        let mut map_bytes = synthetic_map();
        put_map_row(&mut map_bytes, 1, &[64]);
        let map = write_tmp(&tmp, "TEX_HD.MAP", &map_bytes);
        // Unique name resolves to slot 1, which is zero here: payload error,
        // no window (the resolution did run).
        assert_eq!(
            run_textures(&[
                big.clone(),
                "--tshow".to_string(),
                "4_0".to_string(),
                "--map".to_string(),
                map.clone()
            ]),
            ExitCode::from(1)
        );
        assert_eq!(
            run_textures(&[
                big.clone(),
                "--tshow".to_string(),
                "9_9".to_string(),
                "--map".to_string(),
                map
            ]),
            ExitCode::from(1)
        );
        put_map_row(&mut map_bytes, 2, &[64]);
        let ambiguous = write_tmp(&tmp, "ambiguous.map", &map_bytes);
        assert_eq!(
            run_textures(&[
                big,
                "--tshow".to_string(),
                "4_0".to_string(),
                "--map".to_string(),
                ambiguous
            ]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn textures_map_shape_errors_exit_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let big = write_tmp(&tmp, "TEXTURES.BIG", &synthetic_big(512, 2));
        let map = write_tmp(&tmp, "TEX_HD.MAP", &synthetic_map());
        // `--map` without a value.
        assert_eq!(
            run_textures(&[big.clone(), "--tlist".to_string(), "--map".to_string()]),
            ExitCode::from(1)
        );
        // Trailing junk after the map pair.
        assert_eq!(
            run_textures(&[
                big.clone(),
                "--tlist".to_string(),
                "--map".to_string(),
                map.clone(),
                "extra".to_string()
            ]),
            ExitCode::from(1)
        );
        // `--map` in any other position.
        assert_eq!(
            run_textures(&[
                big.clone(),
                "--map".to_string(),
                map.clone(),
                "--tlist".to_string()
            ]),
            ExitCode::from(1)
        );
        // `--tshow` map shapes: missing value, then trailing junk.
        assert_eq!(
            run_textures(&[
                big.clone(),
                "--tshow".to_string(),
                "0".to_string(),
                "--map".to_string()
            ]),
            ExitCode::from(1)
        );
        assert_eq!(
            run_textures(&[
                big.clone(),
                "--tshow".to_string(),
                "0".to_string(),
                "--map".to_string(),
                map.clone(),
                "extra".to_string()
            ]),
            ExitCode::from(1)
        );
        // Valid shape on a zero slot stays loud after resolution.
        assert_eq!(
            run_textures(&[big, "--tshow".to_string(), "0".to_string(), "--map".to_string(), map]),
            ExitCode::from(1)
        );
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
    fn textures_tshow_rejects_selector_and_range() {
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
        assert_eq!(
            run_textures(&[big, "--tshow".to_string(), "-1".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn textures_tshow_no_payload_slots_stay_loud_exit_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let zero = write_tmp(&tmp, "zero.big", &synthetic_big(512, 3));
        assert_eq!(
            run_textures(&[zero, "--tshow".to_string(), "0".to_string()]),
            ExitCode::from(1)
        );
        let mut empty_bytes = synthetic_big(512, 3);
        empty_bytes[4096..].fill(0xFF);
        let empty = write_tmp(&tmp, "empty.big", &empty_bytes);
        assert_eq!(
            run_textures(&[empty, "--tshow".to_string(), "2".to_string()]),
            ExitCode::from(1)
        );
        let invalid = write_tmp(&tmp, "invalid.big", &synthetic_header(256, 3));
        assert_eq!(
            run_textures(&[invalid, "--tshow".to_string(), "0".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn textures_tshow_data_slot_decodes_one_slot_without_a_window() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let mut bytes = synthetic_big(512, 2);
        bytes[4096] = 0xFF; // one non-zero byte makes slot 0 `data`
        let path = write_tmp(&tmp, "data.big", &bytes);
        let header = crate::textures::BigHeader {
            header_size: 512,
            entry_count: 2,
        };
        let (offset, rgba) =
            crate::textures::decode_slot_at(std::path::Path::new(&path), &header, 0)
                .expect("data slot must decode");
        assert_eq!(offset, 4096);
        assert_eq!(rgba.len(), 256 * 256 * 4);
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