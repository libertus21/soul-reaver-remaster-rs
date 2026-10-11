//! DDS texture viewer (`dds-texture-viewer` + `bc7-texture-support`).
//!
//! Usage: `cargo run -- <path-to-DDS>` — opens a window showing the
//! texture. BC7 (`dxgiFormat` 98) files render via a natively-uploaded
//! compressed texture; everything else decodes to RGBA8 as before. The
//! game path arrives via CLI arg at runtime only and is never
//! committed. Every [`dds::ViewerError`] maps to a clear stderr message
//! + `ExitCode(1)`; closing the window exits `0`.

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
            render::show_bc7(compressed.width, compressed.height, &compressed.blocks)
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
