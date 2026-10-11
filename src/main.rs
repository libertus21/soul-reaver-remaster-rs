//! DDS texture viewer (slice 1: `dds-texture-viewer`).
//!
//! Usage: `cargo run -- <path-to-DDS>` — opens a window showing the
//! texture. The game path arrives via CLI arg at runtime only and is
//! never committed. Every [`dds::ViewerError`] maps to a clear stderr
//! message + `ExitCode(1)`; closing the window exits `0`.

mod dds;
mod render;

use std::process::ExitCode;

fn main() -> ExitCode {
    let path = match std::env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("usage: soul-reaver-remaster-rs <path-to-DDS>");
            eprintln!("Opens a window showing the given .DDS texture file.");
            return ExitCode::from(1);
        }
    };
    let image = match dds::load_rgba8(&path) {
        Ok(image) => image,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = render::show_rgba8(image.width, image.height, &image.rgba8) {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
