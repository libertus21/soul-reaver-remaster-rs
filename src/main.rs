//! DDS texture viewer (`dds-texture-viewer` + `bc7-cpu-decode` +
//! `texture-gallery`).
//!
//! Usage: `cargo run -- <path-to-DDS | path-to-directory>` — a file opens
//! a window showing that texture; a directory opens a gallery browsing
//! its sorted `*.DDS` files one at a time (ArrowRight/ArrowLeft, Esc to
//! exit). BC7 (`dxgiFormat` 98) files are CPU-decoded to RGBA8 and render
//! through the same upload path as everything else, so they work on GPUs
//! without BC compression support. Game paths arrive via CLI arg at
//! runtime only and are never committed. Gallery mode skips undecodable
//! entries loud and continues; single-file mode dies loud on the first
//! error (unchanged). Every [`dds::ViewerError`] maps to a clear stderr
//! message + `ExitCode(1)`; closing the window exits `0`.

mod dds;
mod gallery;
mod render;

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arg = match std::env::args().nth(1) {
        Some(arg) => arg,
        None => {
            eprintln!("usage: soul-reaver-remaster-rs <path-to-DDS | path-to-directory>");
            eprintln!("Opens a window showing one .DDS texture file, or browses a");
            eprintln!("directory of .DDS files as a gallery (ArrowRight/ArrowLeft to");
            eprintln!("step, Esc or window close to exit).");
            return ExitCode::from(1);
        }
    };
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
    let result = match &texture {
        dds::LoadedTexture::Rgba8(image) => {
            render::show_rgba8(image.width, image.height, &image.rgba8)
        }
        dds::LoadedTexture::Bc7(compressed) => {
            let image = dds::decode_bc7_to_rgba8(compressed);
            render::show_rgba8(image.width, image.height, &image.rgba8)
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
