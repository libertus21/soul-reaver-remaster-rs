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
//! `cargo run -- --mesh <FILE.SRM> --pose <bone>:<degrees>` additionally
//! rotates exactly one decoded `extra = 0` bone's weighted vertices in
//! mesh space and shows the deformed mesh through the same path.
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
mod level;
mod mesh_entry;
// Pure one-bone pose math, wired into the `--mesh --pose` arm below.
mod pose;
mod render;
mod slot_texture;
mod tex_hd_map;
// Transitional: Task 2 wires the mesh arm; until then the parser is
// test-only so dead-code warnings are allowed here, nowhere else.
#[allow(dead_code)]
mod srm;
mod snd;
mod textures;
mod textures_object;
mod wav;

use std::path::Path;
use std::process::ExitCode;

use cli_out::outln;

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    // Compatibility no-ops: stripped before dispatch, no output, no exit
    // change. (Parser-only flags are never wired into the viewer.)
    let args = strip_launcher_noops(raw);
    if let Some(message) = pose_mode_conflict(&args) {
        eprintln!("{message}");
        return ExitCode::from(1);
    }
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
    if args.first().is_some_and(|a| a == "--audio") {
        return run_audio(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "--level-info") {
        return run_level_info(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "--level-records") {
        return run_level_records(&args[1..]);
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
        eprintln!("   or: soul-reaver-remaster-rs --mesh <FILE.SRM> --pose <bone>:<degrees>");
        eprintln!("   or: soul-reaver-remaster-rs --mesh-textured <FILE.SRM> --tex-dir <TEX>");
        eprintln!("   or: soul-reaver-remaster-rs --mesh-all <OBJECT_dir> --tex-dir <TEX>");
        eprintln!("   or: soul-reaver-remaster-rs --audio <FILE.SND> --alist");
        eprintln!("   or: soul-reaver-remaster-rs --audio <FILE.SND> --wave <n> --out <FILE.wav>");
        eprintln!("   or: soul-reaver-remaster-rs --level-info <.PCM file | room directory>");
        eprintln!("   or: soul-reaver-remaster-rs --level-records <.PCM file | room directory>");
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

/// Distinguish a pose argument-shape error from the generic mesh usage
/// error. `--pose` is only valid directly after `--mesh <FILE.SRM>`.
fn parse_mesh_args(args: &[String]) -> Result<(&Path, Option<&str>), String> {
    match args {
        [path] => Ok((Path::new(path), None)),
        [path, flag, spec] if flag == "--pose" => Ok((Path::new(path), Some(spec.as_str()))),
        _ => Err(mesh_usage_error(args)),
    }
}

fn mesh_usage_error(args: &[String]) -> String {
    let usage = "usage: soul-reaver-remaster-rs --mesh <FILE.SRM> [--pose <bone>:<degrees>]";
    if args.iter().any(|a| a == "--pose") {
        if let Some(flag) = args
            .iter()
            .skip(1)
            .find(|a| is_mode_flag(a) && a.as_str() != "--pose")
        {
            return format!(
                "{usage}\nerror: --pose cannot be combined with {flag}; run one mode at a time"
            );
        }
    }
    format!(
        "{usage}\nerror: --mesh expects <FILE.SRM> or <FILE.SRM> --pose <bone>:<degrees>, got {} arg(s)",
        args.len()
    )
}

/// Refuse `--pose` anywhere except as the second word of the mesh mode,
/// before any file is opened or any window is created.
fn pose_mode_conflict(args: &[String]) -> Option<String> {
    if args.first().is_some_and(|a| a == "--mesh") || !args.iter().any(|a| a == "--pose") {
        return None;
    }
    let other = args
        .iter()
        .skip(1)
        .find(|a| is_mode_flag(a))
        .or_else(|| args.first().filter(|a| is_mode_flag(a)));
    Some(match other {
        Some(flag) => format!(
            "error: --pose cannot be combined with {flag}; --pose is only valid as `--mesh <FILE.SRM> --pose <bone>:<degrees>`"
        ),
        None => "error: --pose is only valid as `--mesh <FILE.SRM> --pose <bone>:<degrees>`".to_string(),
    })
}

fn is_mode_flag(arg: &str) -> bool {
    matches!(
        arg,
        "--bigfile"
            | "--textures"
            | "--tlist"
            | "--tshow"
            | "--show"
            | "--list"
            | "--mesh-textured"
            | "--mesh-all"
            | "--tex-dir"
            | "--map"
            | "--audio"
            | "--alist"
            | "--wave"
            | "--out"
    )
}

/// Prepare the pose path shared by the CLI and its tests: refuse the
/// `extra > 0` class, parse the spec, map joints to mesh space, and apply
/// the one-bone blend. Returns the parsed spec for the stdout line.
fn prepare_pose(
    mesh: &srm::Mesh,
    spec: &str,
) -> Result<(pose::PoseSpec, pose::PoseOutcome), String> {
    mesh.poseable_bones().map_err(|e| e.to_string())?;
    let spec = pose::parse_spec(spec)?;
    let joints = pose::mesh_space_joints(&mesh.bones.joints);
    let outcome = pose::apply(
        &mesh.positions,
        &mesh.bone_indices,
        &mesh.bone_weights,
        &joints,
        spec,
    )?;
    Ok((spec, outcome))
}

/// `--mesh <FILE.SRM> [--pose <bone>:<degrees>]`: parse one SR1-remaster
/// mesh and show it as a flat-shaded auto-rotating solid. With `--pose`,
/// exactly one decoded `extra = 0` bone's weighted vertices are rotated
/// in mesh space before the existing render path. Any parse, class, or
/// pose failure is a loud stderr error + exit 1 with no window; Esc/window
/// close exits 0.
fn run_mesh(args: &[String]) -> ExitCode {
    run_mesh_with(args, render::show_mesh)
}

fn run_mesh_with(
    args: &[String],
    show: impl FnOnce(&[[f32; 3]], &[u16], &str) -> Result<(), String>,
) -> ExitCode {
    let (path, pose_spec) = match parse_mesh_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(1);
        }
    };
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
    let (positions, pose_line) = match pose_spec {
        Some(spec) => match prepare_pose(&mesh, spec) {
            Ok((parsed, outcome)) => (
                outcome.positions,
                Some(format!(
                    "pose bone {}, {} deg, {} vertices moved",
                    parsed.bone, parsed.degrees, outcome.moved_vertices
                )),
            ),
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return ExitCode::from(1);
            }
        },
        None => (mesh.positions.clone(), None),
    };
    outln!(
        "{} vertices, {} triangles, skeleton: {} matrices, {} joints",
        mesh.positions.len(),
        mesh.indices.len() / 3,
        mesh.bones.matrices.len(),
        mesh.bones.joints.len()
    );
    if let Some(line) = pose_line {
        outln!("{line}");
    }
    let title = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    if let Err(e) = show(&positions, &mesh.indices, &title) {
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

/// `--audio <FILE.SND> --alist` lists every wave; `--audio <FILE.SND>
/// --wave <n> --out <FILE.wav>` decodes exactly one wave to a scratch
/// WAV outside any git working tree. Anything else is a usage error.
fn run_audio(args: &[String]) -> ExitCode {
    let usage = || {
        eprintln!("usage: soul-reaver-remaster-rs --audio <FILE.SND> --alist");
        eprintln!(
            "   or: soul-reaver-remaster-rs --audio <FILE.SND> --wave <n> --out <FILE.wav>"
        );
    };
    let Some(snd_arg) = args.first() else {
        usage();
        return ExitCode::from(1);
    };
    let list = args.len() == 2 && args[1] == "--alist";
    let decode = args.len() == 5 && args[1] == "--wave" && args[3] == "--out";
    if !list && !decode {
        usage();
        return ExitCode::from(1);
    }
    let wave_index = if decode {
        match args[2].parse::<usize>() {
            Ok(index) => Some(index),
            Err(_) => {
                usage();
                return ExitCode::from(1);
            }
        }
    } else {
        None
    };

    let snd_path = Path::new(snd_arg);
    let track = track_name(snd_path);
    let snd_bytes = match std::fs::read(snd_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("audio: cannot read {}: {error}", snd_path.display());
            return ExitCode::from(1);
        }
    };
    let bank = match snd::Bank::parse(&snd_bytes) {
        Ok(bank) => bank,
        Err(error) => {
            eprintln!("audio: {error}");
            return ExitCode::from(1);
        }
    };
    let smp_path = snd_path.with_extension("SMP");
    let smp_bytes = match std::fs::read(&smp_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            eprintln!(
                "audio: sibling sample bank not found: expected {}",
                smp_path.display()
            );
            return ExitCode::from(1);
        }
    };
    let body = match snd::bank_body(&smp_bytes) {
        Ok(body) => body,
        Err(error) => {
            eprintln!("audio: {error}");
            return ExitCode::from(1);
        }
    };
    let spans = match bank.spans(body.len()) {
        Ok(spans) => spans,
        Err(error) => {
            eprintln!("audio: {error}");
            return ExitCode::from(1);
        }
    };

    if list {
        outln!(
            "track {track} programs {} zones {} waves {} sequences {} bank {} sample-rate {} (chosen)",
            bank.programs(),
            bank.zones(),
            bank.waves(),
            bank.sequences(),
            bank.bank_version(),
            snd::PLAYBACK_RATE
        );
        for span in &spans {
            let info = match bank.info(body, span.index) {
                Ok(info) => info,
                Err(error) => {
                    eprintln!("audio: {error}");
                    return ExitCode::from(1);
                }
            };
            let loop_text = match (info.loop_start, info.loop_end) {
                (Some(start), Some(end)) => format!("{start}..{end}"),
                _ => "-".to_string(),
            };
            let duration = info.samples as f64 / f64::from(snd::PLAYBACK_RATE);
            outln!(
                "wave {} offset {} bytes {} frames {} samples {} duration {duration:.3}s loop {loop_text}",
                info.span.index,
                info.span.offset,
                info.span.len,
                info.frames,
                info.samples
            );
        }
        return ExitCode::SUCCESS;
    }

    let index = wave_index.expect("decode shape parsed the index");
    if index >= bank.waves() {
        eprintln!("audio: wave {index} out of range (0..{})", bank.waves());
        return ExitCode::from(1);
    }
    let pcm = match bank.decode(body, index) {
        Ok(pcm) => pcm,
        Err(error) => {
            eprintln!("audio: {error}");
            return ExitCode::from(1);
        }
    };
    let out_path = Path::new(&args[4]);
    let parent = out_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        eprintln!(
            "audio: output directory does not exist: {}",
            parent.display()
        );
        return ExitCode::from(1);
    }
    if let Err(message) = wav::ensure_outside_git_tree(out_path) {
        eprintln!("audio: {message}");
        return ExitCode::from(1);
    }
    if let Err(error) = wav::write_mono_pcm16(out_path, &pcm, snd::PLAYBACK_RATE) {
        eprintln!("audio: cannot write {}: {error}", out_path.display());
        return ExitCode::from(1);
    }

    let peak = pcm.iter().map(|sample| sample.unsigned_abs()).max().unwrap_or(0);
    let sum_squares: f64 = pcm
        .iter()
        .map(|&sample| f64::from(sample) * f64::from(sample))
        .sum();
    let rms = if pcm.is_empty() {
        0.0
    } else {
        (sum_squares / pcm.len() as f64).sqrt()
    };
    let nonzero = pcm.iter().filter(|&&sample| sample != 0).count();
    let nonzero_pct = if pcm.is_empty() {
        0.0
    } else {
        nonzero as f64 * 100.0 / pcm.len() as f64
    };
    let duration = pcm.len() as f64 / f64::from(snd::PLAYBACK_RATE);
    outln!(
        "wrote {}: {track} wave {index}, {} samples @ {} Hz (chosen), {duration:.3}s, peak {peak}, rms {rms:.0}, nonzero {nonzero_pct:.1}%",
        out_path.display(),
        pcm.len(),
        snd::PLAYBACK_RATE
    );
    ExitCode::SUCCESS
}

/// Read-only `--level-info` arm: one line per unit, then one per link.
fn run_level_info(args: &[String]) -> ExitCode {
    let result = match args {
        [path] if !path.starts_with('-') => level::read_path(Path::new(path)),
        _ => Err(level::LevelError::Usage("expected --level-info <path>")),
    };
    let units = match result {
        Ok(units) => units,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(error.exit_code());
        }
    };
    for line in units.iter().flat_map(level::report_lines) {
        outln!("{line}");
    }
    ExitCode::SUCCESS
}

/// Read-only `--level-records` arm: per-unit block shape-class census.
/// Every unit is read before the first line is printed, so a failure on any
/// unit exits without partial output.
fn run_level_records(args: &[String]) -> ExitCode {
    let result = match args {
        [path] if !path.starts_with('-') => level::read_path(Path::new(path)),
        _ => Err(level::LevelError::Usage("expected --level-records <path>")),
    };
    let units = match result {
        Ok(units) => units,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(error.exit_code());
        }
    };
    for line in units.iter().flat_map(level::record_lines) {
        outln!("{line}");
    }
    ExitCode::SUCCESS
}

/// Track label for messages: the file stem, never an install path.
fn track_name(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<unknown>".to_string())
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
    use super::run_level_info;
    use super::{
        list_report, pose_mode_conflict, resolve_texture_name, run_audio, run_bigfile,
        run_level_records, run_mesh, run_mesh_all, run_mesh_textured, run_mesh_with, run_textures,
        strip_launcher_noops, tshow_suffix,
    };
    use std::process::ExitCode;
    use std::rc::Rc;

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
    fn level_info_maps_the_exit_matrix() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let mut pcm = 2u32.to_le_bytes().to_vec();
        pcm.extend_from_slice(&0u32.to_le_bytes());
        pcm.extend_from_slice(&4u32.to_le_bytes());
        pcm.resize(crate::level::data_base(2), 0);
        pcm.extend_from_slice(b"lair5\0\0\0\0\0\0\0\0\0");
        let pcm_path = write_tmp(&tmp, "lair5.PCM", &pcm);
        let pmf_path = write_tmp(&tmp, "notes.PMF", &[0u8; 44]);
        let empty = tmp.path().join("EMPTY");
        std::fs::create_dir(&empty).expect("EMPTY must be creatable");
        let empty = empty.to_string_lossy().into_owned();
        assert_eq!(run_level_info(&[]), ExitCode::from(1));
        assert_eq!(run_level_info(&["--other".to_string()]), ExitCode::from(1));
        assert_eq!(run_level_info(&[pcm_path.clone(), pcm_path.clone()]), ExitCode::from(1));
        assert_eq!(run_level_info(&[pmf_path]), ExitCode::from(3));
        assert_eq!(run_level_info(&[empty]), ExitCode::from(2));
        assert_eq!(run_level_info(&[pcm_path]), ExitCode::SUCCESS);
    }

    /// Minimal ascending `.PCM` whose block 0 is 4 bytes and whose name
    /// lives inside the last block, so the shape classes stay deterministic.
    fn level_pcm(blocks: &[Vec<u8>]) -> Vec<u8> {
        let mut offsets = Vec::with_capacity(blocks.len());
        let mut body = Vec::new();
        for block in blocks {
            offsets.push(body.len() as u32);
            body.extend_from_slice(block);
        }
        let mut bytes = (blocks.len() as u32).to_le_bytes().to_vec();
        for offset in offsets {
            bytes.extend_from_slice(&offset.to_le_bytes());
        }
        bytes.resize(crate::level::data_base(blocks.len() as u32), 0);
        bytes.extend_from_slice(&body);
        bytes
    }

    #[test]
    fn level_records_census_maps_the_exit_matrix() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let mut last = vec![0x01u8; 72];
        last[..5].copy_from_slice(b"lair5");
        let pcm = level_pcm(&[vec![0u8; 4], vec![0x02u8; 40], last]);
        let pcm_path = write_tmp(&tmp, "lair5.PCM", &pcm);
        let pmf_path = write_tmp(&tmp, "notes.PMF", &[0u8; 44]);
        let empty = tmp.path().join("EMPTY");
        std::fs::create_dir(&empty).expect("EMPTY must be creatable");
        let empty = empty.to_string_lossy().into_owned();
        assert_eq!(run_level_records(&[]), ExitCode::from(1));
        assert_eq!(run_level_records(&["--other".to_string()]), ExitCode::from(1));
        assert_eq!(
            run_level_records(&[pcm_path.clone(), pcm_path.clone()]),
            ExitCode::from(1)
        );
        assert_eq!(run_level_records(&[pmf_path]), ExitCode::from(3));
        assert_eq!(run_level_records(&[empty]), ExitCode::from(2));
        assert_eq!(run_level_records(&[pcm_path]), ExitCode::SUCCESS);
        let dir = tmp.path().to_string_lossy().into_owned();
        assert_eq!(run_level_records(&[dir]), ExitCode::SUCCESS);
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

    /// Identity 3x4 affine that passes the `extra > 0` orthonormality check.
    const IDENTITY_AFFINE: [f32; 12] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
    ];

    /// Minimal synthetic `.SRM` (no shaders, no materials, sentinel arm)
    /// with a caller-chosen matrix pool, joint prefix, and weighted vertices.
    fn synthetic_srm(
        extra: &[[f32; 12]],
        joints: &[[f32; 3]],
        verts: &[([f32; 3], [u8; 3], [u8; 3])],
    ) -> Vec<u8> {
        use crate::srm::{
            FLAG_BYTES, JOINT_SLOTS, MAGIC, OFF_MATERIAL, OFF_V, SENTINEL, VERTEX_STRIDE,
        };
        let mut b = Vec::new();
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&(extra.len() as u32).to_le_bytes());
        for m in extra {
            for v in m {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        let mut pool = [[0.0f32; 3]; JOINT_SLOTS];
        pool[..joints.len()].copy_from_slice(joints);
        for j in pool.iter().flatten() {
            b.extend_from_slice(&j.to_le_bytes());
        }
        b.extend_from_slice(&SENTINEL.to_le_bytes());
        b.extend(std::iter::repeat_n(0u8, FLAG_BYTES as usize));
        b.extend_from_slice(&(verts.len() as u32).to_le_bytes());
        b.extend_from_slice(&3u32.to_le_bytes());
        for (pos, idx, weights) in verts {
            for x in pos {
                b.extend_from_slice(&x.to_le_bytes());
            }
            b.extend(std::iter::repeat_n(0u8, OFF_MATERIAL - 12));
            b.push(0);
            b.extend_from_slice(idx);
            b.push(0);
            b.extend_from_slice(weights);
            b.push(0);
            b.extend(std::iter::repeat_n(
                0u8,
                VERTEX_STRIDE as usize - OFF_V - 1,
            ));
        }
        for f in [0u16, 1, 2] {
            b.extend_from_slice(&f.to_le_bytes());
        }
        b
    }

    /// Joint 0 at the origin with its tail at `+Y`, so the derived swing
    /// axis is `+X` and 45 degrees maps `[0, 0, 1]` to `[0, -s, c]`.
    fn pose_joints() -> [[f32; 3]; 2] {
        [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
    }

    fn weighted_verts() -> [([f32; 3], [u8; 3], [u8; 3]); 3] {
        [
            ([0.0, 0.0, 1.0], [0, 0, 0], [255, 0, 0]),
            ([2.0, 0.0, 0.0], [0, 0, 0], [255, 0, 0]),
            ([0.0, 0.0, 0.0], [0, 0, 0], [255, 0, 0]),
        ]
    }

    fn rest_positions() -> Vec<[f32; 3]> {
        weighted_verts().iter().map(|(pos, _, _)| *pos).collect()
    }

    fn pose_args(path: &str, spec: &str) -> Vec<String> {
        vec![path.to_string(), "--pose".to_string(), spec.to_string()]
    }

    /// Run the mesh CLI with a capturing renderer, so tests prove whether
    /// the render call was reached without ever opening a window.
    fn run_mesh_capture(args: &[String]) -> (ExitCode, bool, Option<Vec<[f32; 3]>>) {
        let reached = Rc::new(std::cell::Cell::new(false));
        let captured = Rc::new(std::cell::RefCell::new(None));
        let (seen, sink) = (Rc::clone(&reached), Rc::clone(&captured));
        let code = run_mesh_with(args, move |positions, _, _| {
            seen.set(true);
            *sink.borrow_mut() = Some(positions.to_vec());
            Ok(())
        });
        let rendered = reached.get().then(|| captured.borrow_mut().take()).flatten();
        (code, reached.get(), rendered)
    }

    fn assert_mesh_refused(args: &[String], why: &str) {
        let (code, reached, positions) = run_mesh_capture(args);
        assert_eq!(code, ExitCode::from(1), "{why}: must exit 1");
        assert!(!reached, "{why}: must not reach the render call");
        assert!(positions.is_none(), "{why}: must not render positions");
    }

    #[test]
    fn mesh_rest_path_reaches_render_unchanged() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let path = write_tmp(&tmp, "rest.srm", &synthetic_srm(&[], &pose_joints(), &weighted_verts()));
        let (code, reached, positions) = run_mesh_capture(&[path]);
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(reached, "rest --mesh must still reach the render call");
        assert_eq!(positions.expect("render positions"), rest_positions());
    }

    #[test]
    fn mesh_pose_valid_spec_reaches_render_with_deformed_positions() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let path = write_tmp(&tmp, "pose.srm", &synthetic_srm(&[], &pose_joints(), &weighted_verts()));
        let (code, reached, positions) = run_mesh_capture(&pose_args(&path, "0:45"));
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(reached, "a valid --pose spec must reach the render call");
        let rendered = positions.expect("render must receive posed positions");
        let moved = rendered[0];
        let diag = std::f32::consts::FRAC_1_SQRT_2;
        assert!((moved[1] + diag).abs() < 1e-4, "expected a -45-degree X swing, got {moved:?}");
        assert!((moved[2] - diag).abs() < 1e-4);
        assert_eq!(rendered[1], [2.0, 0.0, 0.0], "pivot vertices stay put");
        let (code, reached, positions) = run_mesh_capture(&pose_args(&path, "0:0"));
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(reached, "zero angle still reaches the render call");
        assert_eq!(positions.expect("render positions"), rest_positions());
    }

    #[test]
    fn mesh_pose_refusals_exit_1_without_a_window() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let rest = write_tmp(&tmp, "rest.srm", &synthetic_srm(&[], &pose_joints(), &weighted_verts()));
        let hd = write_tmp(&tmp, "hd.srm", &synthetic_srm(&[IDENTITY_AFFINE], &pose_joints(), &weighted_verts()));
        assert_mesh_refused(&pose_args(&hd, "0:45"), "extra > 0 model");
        for spec in ["7", ":45", "7:", "7:45:0", "x:45", "7:nan", "7:361", "128:45"] {
            assert_mesh_refused(&pose_args(&rest, spec), &format!("bad spec {spec:?}"));
        }
        assert_mesh_refused(&pose_args(&rest, "5:45"), "bone with no weighted vertex");
        let mut mixed = pose_args(&rest, "0:45");
        mixed.push("--tshow".to_string());
        assert_mesh_refused(&mixed, "--pose with an incompatible mode");
        let mut cut = synthetic_srm(&[], &pose_joints(), &weighted_verts());
        cut.truncate(20);
        let cut = write_tmp(&tmp, "cut.srm", &cut);
        assert_mesh_refused(&pose_args(&cut, "0:45"), "truncated bone block");
    }

    #[test]
    fn pose_outside_the_mesh_mode_is_refused() {
        let args = ["--textures", "TEXTURES.BIG", "--tshow", "0", "--pose", "7:45"].map(String::from);
        let err = pose_mode_conflict(&args).expect("--pose in another mode must be refused");
        assert!(err.contains("--tshow"), "{err}");
        assert!(pose_mode_conflict(&["--mesh", "a.srm", "--pose", "7:45"].map(String::from)).is_none());
        let bare = pose_mode_conflict(&["--pose", "7:45"].map(String::from));
        assert!(bare.expect("bare --pose must be refused").contains("--pose"));
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

    /// Synthetic 16-byte frame with all nibbles 7 (filter 0, shift 0).
    fn audio_frame(flag: u8) -> Vec<u8> {
        let mut frame = vec![0u8; crate::snd::FRAME_BYTES];
        frame[1] = flag;
        for byte in &mut frame[2..] {
            *byte = 0x77;
        }
        frame
    }

    /// Synthetic `.SND`: one program, one zone (wave 0), given offsets.
    fn audio_snd(waves: &[u32]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"DNSa");
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&0x0106u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(waves.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 4]);
        let mut zone = [0u8; crate::snd::ZONE_BYTES];
        zone[7] = 127;
        bytes.extend_from_slice(&zone);
        for &offset in waves {
            bytes.extend_from_slice(&offset.to_le_bytes());
        }
        bytes
    }

    fn audio_smp(body: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PMSa");
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(body);
        bytes
    }

    fn write_audio_pair(dir: &tempfile::TempDir, waves: &[u32], body: &[u8]) -> String {
        let snd = write_tmp(dir, "TRACK.SND", &audio_snd(waves));
        write_tmp(dir, "TRACK.SMP", &audio_smp(body));
        snd
    }

    #[test]
    fn audio_alist_lists_every_wave_and_exits_0() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let body = [audio_frame(0), audio_frame(0)].concat();
        let snd = write_audio_pair(&tmp, &[0, 16], &body);
        assert_eq!(
            run_audio(&[snd, "--alist".to_string()]),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn audio_wave_decode_writes_a_canonical_wav() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let body = [audio_frame(0), audio_frame(7)].concat();
        let snd = write_audio_pair(&tmp, &[0], &body);
        let out = tmp.path().join("one.wav");
        assert_eq!(
            run_audio(&[
                snd,
                "--wave".to_string(),
                "0".to_string(),
                "--out".to_string(),
                out.to_string_lossy().into_owned(),
            ]),
            ExitCode::SUCCESS
        );
        let bytes = std::fs::read(&out).expect("wav must exist");
        assert_eq!(bytes.len(), crate::wav::WAV_HEADER_BYTES + 28 * 2);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(
            u32::from_le_bytes(bytes[24..28].try_into().expect("rate field")),
            44_100
        );
        assert_eq!(
            u32::from_le_bytes(bytes[40..44].try_into().expect("data size")),
            28 * 2
        );
    }

    #[test]
    fn audio_decode_refuses_an_out_of_range_wave() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let snd = write_audio_pair(&tmp, &[0], &audio_frame(0));
        let out = tmp.path().join("one.wav");
        assert_eq!(
            run_audio(&[
                snd,
                "--wave".to_string(),
                "999".to_string(),
                "--out".to_string(),
                out.to_string_lossy().into_owned(),
            ]),
            ExitCode::from(1)
        );
        assert!(!out.exists(), "a refused run must leave no WAV behind");
    }

    #[test]
    fn audio_refuses_a_bad_magic_and_a_foreign_header_size() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let body = audio_frame(0);
        write_tmp(&tmp, "TRACK.SMP", &audio_smp(&body));

        let mut bad_magic = audio_snd(&[0]);
        bad_magic[0..4].copy_from_slice(b"DNSb");
        let snd = write_tmp(&tmp, "TRACK.SND", &bad_magic);
        assert_eq!(
            run_audio(&[snd, "--alist".to_string()]),
            ExitCode::from(1)
        );

        let mut bad_size = audio_snd(&[0]);
        bad_size[4..8].copy_from_slice(&28u32.to_le_bytes());
        let snd = write_tmp(&tmp, "TRACK.SND", &bad_size);
        assert_eq!(
            run_audio(&[snd, "--alist".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn audio_refuses_a_missing_bank_and_a_size_mismatch() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let snd = write_tmp(&tmp, "TRACK.SND", &audio_snd(&[0]));
        assert_eq!(
            run_audio(&[snd.clone(), "--alist".to_string()]),
            ExitCode::from(1)
        );

        let mut smp = audio_smp(&audio_frame(0));
        smp[4..8].copy_from_slice(&15u32.to_le_bytes());
        write_tmp(&tmp, "TRACK.SMP", &smp);
        assert_eq!(
            run_audio(&[snd, "--alist".to_string()]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn audio_refuses_an_output_inside_a_git_tree() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let snd = write_audio_pair(&tmp, &[0], &audio_frame(0));
        std::fs::create_dir(tmp.path().join(".git")).expect("synthetic .git dir");
        let out = tmp.path().join("refused.wav");
        assert_eq!(
            run_audio(&[
                snd,
                "--wave".to_string(),
                "0".to_string(),
                "--out".to_string(),
                out.to_string_lossy().into_owned(),
            ]),
            ExitCode::from(1)
        );
        assert!(!out.exists(), "the git guard must refuse before writing");
    }

    #[test]
    fn audio_refuses_a_missing_output_parent() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let snd = write_audio_pair(&tmp, &[0], &audio_frame(0));
        let out = tmp.path().join("absent").join("one.wav");
        assert_eq!(
            run_audio(&[
                snd,
                "--wave".to_string(),
                "0".to_string(),
                "--out".to_string(),
                out.to_string_lossy().into_owned(),
            ]),
            ExitCode::from(1)
        );
    }

    #[test]
    fn audio_usage_shapes_exit_1() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let snd = write_audio_pair(&tmp, &[0], &audio_frame(0));
        assert_eq!(
            run_audio(&[snd.clone(), "--wave".to_string(), "1".to_string()]),
            ExitCode::from(1)
        );
        assert_eq!(
            run_audio(&[snd, "--alist".to_string(), "extra".to_string()]),
            ExitCode::from(1)
        );
    }
}