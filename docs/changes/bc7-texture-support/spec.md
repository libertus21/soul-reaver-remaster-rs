# Spec: bc7-texture-support

`cargo run -- <BC7 DDS>` opens a winit window that renders the given BC7 texture via a natively-uploaded compressed wgpu texture. The slice-1 DXT1 path keeps working untouched.

## Quick path

1. Run `cargo run -- "<install>\1\TEX\0_0.DDS"` (any `dxgiFormat` 98 file; path is runtime-only, never committed).
2. A window opens showing the texture.
3. Compare by eye against the same file in a reference DDS viewer — pixels match.

## Observed real-file header values (read-only, first 256 bytes, 2026-10-11)

| File | Size | Magic | FourCC | `dxgiFormat` (@128) | WxH | Mipmaps |
|---|---|---|---|---|---|---|
| `1\TEX\0_0.DDS` | 349,588 | `DDS ` | `DX10` | 98 | 512x512 | 6 |
| `1\TEX\1000_0.DDS` | 349,588 | `DDS ` | `DX10` | 98 | 512x512 | 6 |
| `1\TEX\1000_1.DDS` | 349,588 | `DDS ` | `DX10` | 98 | 512x512 | 6 |

Also observed (identical on all three): header size 124, flags 131078, pitch 2048, depth 0, `rgbBitCount` 0, caps 4198400, `resourceDim` 3 (TEXTURE2D), `miscFlag` 0, `arraySize` 1.

Size cross-check (validates the design's block math, no decoding involved): header is 4 + 124 + 20 = 148 bytes; base mip is (512/4)^2 blocks x 16 bytes = 262,144 bytes; 148 + 262,144 = 262,292 <= 349,588, so the base mip is fully present. Remaining bytes are the 5 lower mips (349,440 - 262,144 = 87,296 + 148 header = 349,588 exactly), which this slice intentionally does not upload.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI contract unchanged: exactly one arg (DDS path); missing/unreadable path is a LOUD error (message + nonzero exit) |
| R2 | `dxgiFormat` 98 files load via a new `load_bc7` path: header dims parsed, base-mip blocks sliced from the file, NO `image`-crate decode involved |
| R3 | Base-mip blocks uploaded to a `Bc7RgbaUnorm` wgpu texture (`TEXTURE_COMPRESSION_BC` feature) and drawn with the existing fullscreen-quad shader; window behavior (open until closed, close exits 0) unchanged |
| R4 | DXT1/legacy path (`load_rgba8` + `show_rgba8`) keeps working with zero behavior change — no regression on the 33 files the user playtest-accepted |
| R5 | Unknown/unsupported `dxgiFormat` (anything except 98 and whatever `image` decodes, incl. BC7-sRGB 99) still fails LOUD: stderr message naming the format value, nonzero exit, no fallback guessing |
| R6 | Malformed BC7 input (dims not multiples of 4, zero dims, file shorter than header + base mip) fails LOUD via the new `InvalidBc7` error naming the offending values, nonzero exit |
| R7 | Missing `TEXTURE_COMPRESSION_BC` adapter support fails LOUD (stderr names the feature), nonzero exit, no window |
| R8 | No game bytes, paths, or offsets committed; whitelist `.gitignore` verified per commit; tests use synthetic bytes only |

## Acceptance scenarios

- [ ] **Happy path (BC7):** `cargo run -- <dxgiFormat-98 TEX/*.DDS>` (e.g. `1\TEX\0_0.DDS`) opens a window; texture visibly matches the reference viewer (user playtest note records file + result).
- [ ] **No regression (DXT1):** `cargo run -- <legacy DXT1 DDS>` still renders exactly as in slice 1.
- [ ] **Unknown format:** a DDS with any other undecodable `dxgiFormat` (e.g. synthetic 987654, real 99 if encountered) exits nonzero with an error naming the format value.
- [ ] **Truncated BC7:** a `dxgiFormat`-98 file shorter than header + base mip exits nonzero with an `InvalidBc7`-family message naming expected vs found sizes.
- [ ] **Unit level:** `cargo test` covers BC7 header math (base-mip size, block slicing on synthetic bytes) without touching game files.
- [ ] **Legal:** `git status --short` shows only intended source/doc files; `*.dds` can never appear.

## Next step

See `design.md` for modules and data flow, `tasks.md` for the ordered work units.
