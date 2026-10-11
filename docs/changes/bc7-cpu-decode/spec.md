# Spec: bc7-cpu-decode

`cargo run -- <BC7 DDS>` opens a winit window that renders the given BC7 texture as CPU-decoded RGBA8 through the existing `show_rgba8` path. No compressed GPU format is requested, so it works on adapters WITHOUT `TEXTURE_COMPRESSION_BC`. The DXT1 path keeps working untouched.

## Quick path

1. Run `cargo run -- "<install>\1\TEX\0_0.DDS"` (any `dxgiFormat` 98 file; path is runtime-only, never committed).
2. A window opens showing the texture — on THIS machine, where slice 2 failed loud.
3. Compare by eye against the same file in a reference DDS viewer — pixels match.

## Observed real-file header values (read-only, first 256 bytes, re-confirmed 2026-10-11)

| File | Size | Magic | `dxgiFormat` (@128) | WxH |
|---|---|---|---|---|
| `1\TEX\0_0.DDS` | 349,588 | `DDS ` | 98 | 512x512 |

Matches the slice-2 spec table exactly (512x512, 6 mips, 349,588 bytes = 148 header + 262,144 base mip + 87,296 lower mips). One file re-read this slice (rule allows ≤3); the other two were verified in slice 2 and are unchanged on a read-only install. No game bytes copied anywhere.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI contract unchanged: exactly one arg (DDS path); missing/unreadable path is a LOUD error (message + nonzero exit) |
| R2 | `dxgiFormat` 98 files decode on the CPU: slice-2 header math reused verbatim (`load_bc7_from_bytes`), each 16-byte block decoded to RGBA8 via the pinned `bcdec_rs` crate, base mip only |
| R3 | Decoded pixels render via the EXISTING `show_rgba8` (`Rgba8Unorm`, no feature flags); window behavior (open until closed, close exits 0) unchanged; NO `TEXTURE_COMPRESSION_BC` request anywhere on this path |
| R4 | DXT1/legacy path keeps working with zero behavior change — no regression on the files the user playtest-accepted |
| R5 | Unknown/unsupported `dxgiFormat` (anything except 98 and whatever `image` decodes, incl. BC7-sRGB 99) still fails LOUD: stderr message naming the format value, nonzero exit, no fallback guessing |
| R6 | Malformed BC7 input (dims not multiples of 4, zero dims, file shorter than header + base mip) still fails LOUD via `InvalidBc7` naming the offending values, nonzero exit |
| R7 | `cargo build` emits ZERO warnings (`dead_code` wrappers removed, not silenced — see design) |
| R8 | No game bytes, paths, or offsets committed; whitelist `.gitignore` verified per commit; tests use synthetic bytes only; decoder crate license recorded (design) |

## Acceptance scenarios

- [ ] **Happy path (BC7 on BC-less GPU):** `cargo run -- <dxgiFormat-98 TEX/*.DDS>` (e.g. `1\TEX\0_0.DDS`) opens a window ON THIS MACHINE; texture visibly matches the reference viewer (user playtest note records file + result). This is the slice's raison d'être.
- [ ] **No regression (DXT1):** `cargo run -- <legacy DXT1 DDS>` still renders exactly as in slices 1–2.
- [ ] **Unknown format:** a DDS with any other undecodable `dxgiFormat` (e.g. synthetic 987654, real 99 if encountered) exits nonzero with an error naming the format value.
- [ ] **Truncated BC7:** a `dxgiFormat`-98 file shorter than header + base mip exits nonzero with an `InvalidBc7`-family message naming expected vs found sizes.
- [ ] **Unit level:** `cargo test` covers the CPU decode on synthetic blocks (shape + golden + invalid-block behavior) without touching game files.
- [ ] **Zero warnings:** `cargo build` output contains no `warning:`; `cargo test` is green.
- [ ] **Legal:** `git status --short` shows only intended source/doc files; `*.dds` can never appear.

## Next step

See `design.md` for the crate choice, data flow, and the supersede decision; `tasks.md` for the ordered work units.
