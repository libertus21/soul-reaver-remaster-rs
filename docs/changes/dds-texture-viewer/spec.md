# Spec: dds-texture-viewer

`cargo run -- <path-to-DDS>` opens a winit window that renders the given texture via a wgpu fullscreen quad.

## Quick path

1. Run `cargo run -- "D:\Juego\planescape\Soul Reaver I-II\1\TEX\0_0.DDS"` (any real `.DDS` works; path is runtime-only, never committed).
2. A window opens showing the texture.
3. Compare by eye against the same file in a reference DDS viewer — pixels match.

## Observed real-file header values (read-only, first 256 bytes, 2026-10-11)

| File | Size | Magic | FourCC | `dxgiFormat` (@128) | WxH | Mipmaps |
|---|---|---|---|---|---|---|
| `1\TEX\0_0.DDS` | 349,588 | `DDS ` | `DX10` | 98 | 512x512 | 6 |
| `1\TEX\1000_0.DDS` | 349,588 | `DDS ` | `DX10` | 98 | 512x512 | 6 |
| `1\TEX\1000_1.DDS` | 349,588 | `DDS ` | `DX10` | 98 | 512x512 | 6 |

Also observed on `1000_1.DDS`: header size 124, flags 131078, `rgbBitCount` 0, caps 4198400. Semantic mapping of `dxgiFormat` 98 to a named BC variant: UNKNOWN — the implementation must decode through the `image` crate and fail loudly on anything it cannot decode. Never invented here.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI takes exactly one argument: path to a `.DDS` file; missing/unreadable path is a LOUD error (message + nonzero exit) |
| R2 | Header parsed via `image` DDS support, including the DX10 extended header; base mip level decoded to RGBA8 |
| R3 | Decoded pixels uploaded to a wgpu texture and drawn as a fullscreen quad in a winit window |
| R4 | Unknown/unsupported `dxgiFormat` (anything `image` cannot decode) fails LOUD: clear stderr message naming the format value, nonzero exit, no fallback guessing |
| R5 | Non-DDS input (bad magic) fails LOUD the same way |
| R6 | Window stays open until closed by the user; close exits cleanly with code 0 |
| R7 | No game bytes, paths, or offsets committed; whitelist `.gitignore` verified per commit |

## Acceptance scenarios

- [ ] **Happy path:** `cargo run -- <real TEX/*.DDS>` opens a window; texture visibly matches the reference viewer (user playtest note records file + result).
- [ ] **Unknown format:** a DDS whose `dxgiFormat` `image` cannot decode exits nonzero with an error naming the format value.
- [ ] **Non-DDS input:** passing a non-DDS file (e.g. a `.TXT`) exits nonzero with a clear error, no window, no panic-without-message.
- [ ] **Missing arg:** no argument prints usage to stderr and exits nonzero.
- [ ] **Unit level:** `cargo test` covers header parsing (magic/fourcc/dxgiFormat extraction on synthetic bytes) without touching game files.
- [ ] **Legal:** `git status --short` shows only intended source/doc files; `*.dds` can never appear.

## Next step

See `design.md` for modules and data flow, `tasks.md` for the ordered work units.
