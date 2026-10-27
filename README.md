# soul-reaver-remaster-rs

Unofficial fan-project Rust rewrite of Legacy of Kain: Soul Reaver 1&2
Remastered (2024, Aspyr). Not affiliated with or endorsed by Aspyr or
Crystal Dynamics. Written with AI coding-agent assistance.

The engine reads data from the player's OWN installed copy of the game
at runtime. This repo contains only our code — never game assets,
extracted files, decompiled code, or database files.

## Quick path

1. Install the game (GOG/Steam build) and verify it runs.
2. `cargo test` — runs the suite (DDS header/error unit tests on
   synthetic bytes; no game files in tests).
3. `cargo run -- <path-to-a-DDS>` — opens a window showing that
   texture (pass your own install path at runtime; never committed).

## Details

| Topic | Decision |
|---|---|
| Stack | Rust (cargo 1.99.0), wgpu + winit + image for slice 1; `bevy_ecs` later if game logic needs it (see `docs/notes/03-dds-viewer-direction.md`) |
| Game data | Read-only at runtime from the user's install; paths via CLI args/env only, never committed |
| Legal guardrails | Whitelist `.gitignore` (verified every commit); single-player/offline owned copy only; credit community work in `THIRD-PARTY-NOTICES.md` once reuse starts |
| Process | Small chained slices (≤400 lines each); proposal → spec → tasks → code → playtest report |

## Checklist

- [x] Slice 1 `dds-texture-viewer`: load one real `.DDS` from the install, show it in a window
- [x] `THIRD-PARTY-NOTICES.md` created on first community-code reuse
      (`bcdec_rs`, slice `bc7-cpu-decode`)
- [ ] README "What works" updated every slice

## What works

- `.BIG` textures (playtest-accepted): reads and renders textures from the
  `.BIG` containers (256 × 256 A1R5G5B5). `--tshow` renders real content
  (stone, faces, legible runes) from `TEXTURES.BIG`, `BONUS.BIG`, `E3.BIG`, and
  `MOVIES.BIG` slots; large flat regions are atlas fill/padding, not a decode
  error. Container contract in the `big-slots` bullet; format proof in
  `docs/changes/big-decompressor/archive.md`.
- `dds-texture-viewer`: `cargo run -- <path-to-a-DDS>` decodes one
  `.DDS` (DX10 extended header via the `image` crate, base mip to
  RGBA8) and shows it as a stretched fullscreen quad in a winit/wgpu
  window. Bad magic, unsupported `dxgiFormat`, and unreadable paths
  fail loud (stderr + nonzero exit, no guessing).
- `bc7-cpu-decode`: BC7 textures via CPU decode (playtest-accepted):
  `cargo run -- <path-to-a-BC7-DDS>` (`dxgiFormat` 98) CPU-decodes the base-mip blocks to RGBA8 (pinned
  `bcdec_rs` 0.2.0, MIT — see `THIRD-PARTY-NOTICES.md`) and shows them
  through the same `Rgba8Unorm` upload as slice 1, so it works on GPUs
  WITHOUT `TEXTURE_COMPRESSION_BC`. Malformed BC7 input fails loud via
  `InvalidBc7`; any other undecodable `dxgiFormat` (including 99) still
  fails loud via `UnsupportedFormat`. Supersedes the slice-2 native
  BC7 upload (kept in git history), which this machine's adapter cannot
  run.
- `texture-gallery` (playtest-accepted: browses 8066 TEX with
  arrows): `cargo run -- <directory>` browses that
  directory's direct-child `*.DDS` files (case-insensitive extension,
  sorted ascending by file name, no recursion) one at a time in the
  existing window through the existing decode + upload path (lazy
  decode, no pre-scan). ArrowRight steps forward, ArrowLeft steps back
  (clamped at the ends, key auto-repeat ignored); the window title and
  one console line both show `<index>/<total> <filename>` (1-based;
  `index/total` counts list entries). Esc or window close exits 0.
  Gallery skip policy: an undecodable entry prints
  `warning: skipping <filename>: <error>` to stderr and browsing
  continues; an empty or all-undecodable directory exits 1 with no
   window. Single-file mode is unchanged.
- `bigfile-index`: `cargo run -- --bigfile <BIGFILE.DAT> --list`
  prints the SR1PC archive index (one `index hash offset length` line
  per entry; `hash` is the stored 8-hex-digit name hash) and exits 0;
  `cargo run -- --bigfile <BIGFILE.DAT> --show <index|hash>` reads
  that entry IN MEMORY (extracted bytes are never written to disk or
  repo) and, when it is a DDS texture, shows it through the existing
  single-file window (close/Esc exits 0). The selector is the decimal
  list index from `--list` or the stored name hash (case-insensitive,
  optional `0x`); DDS entries only — any other archived type is
  listed, never decoded (loud stderr + exit 1). Corrupt/truncated
  containers, out-of-range selectors, and unknown hashes all fail
  loud (stderr + nonzero exit, no guessing). Single-file and gallery
  modes are byte-for-byte unchanged. Layout facts reimplemented
  clean-room from Soul Spiral's `BenLincoln.TheLostWorlds.CDBigFile`
  (GPL-3.0-only, mirror `github.com/TheSerioliOfNosgoth/SoulSpiral-Official`;
  NO Soul Spiral code copied — facts only, so no THIRD-PARTY-NOTICES.md
   entry; see `docs/changes/bigfile-index/design.md`).
- `big-slots`: the four SR1 `.BIG` containers (`TEXTURES.BIG`,
  `MOVIES.BIG`, `BONUS.BIG`, `E3.BIG`) share a verified 512-byte header
  (`0..2` = header size, must be 512; `2..4` = entry count, must be
  `1..=32768`, little-endian) and fixed 131072-byte slots:
  `file_len == 4096 + count * 131072` exactly, slot `i` at
  `4096 + i * 131072`. `cargo run -- --textures <TEXTURES.BIG> --tlist`
  validates that geometry, prints the header report (header size, entry
  count, size class, window classes) plus the addressing line, one
  `slot <i> offset <off> <class>` line per slot (`empty` = all `0xFF`,
  `zero` = all `0x00`, else `data`; whole-slot read, no sampling), and a
  per-class footer, then exits 0. Any length mismatch is a DISTINCT loud
  `slot geometry mismatch` error (exit 1, never clamped). Truncated
  files, wrong header sizes, and implausible counts each fail loud with
  their distinct stderr message. `--tshow <i>` rejects a non-integer or
  out-of-range index distinctly and, for a valid `data` slot, reads exactly
  that one slot, converts its raw 256 x 256 little-endian A1R5G5B5 pixels
  to RGBA8 (bit 15 = alpha, each 5-bit channel expanded by bit replication)
  and shows it through the existing window (close/Esc exits 0); `empty` and
  `zero` slots stay loud (exit 1) with no window. There is no decompressor
  because the payload is not compressed (see
  `docs/notes/s7-big-slot-payload-format.md`). `bigfile.rs` is
  untouched (this is not the SR1PC layout). Bytes `4..512` are observed
   but not validated. Synthetic-bytes tests only. Slot listing is
   playtest-accepted: `--tlist` exits 0 for all four containers with the
   footer `slots: 29 total, 1 empty, 0 zero, 28 data` (`MOVIES.BIG`),
   `1287 total, 1 empty, 0 zero, 1286 data` (`TEXTURES.BIG`),
   `268 total, 1 empty, 0 zero, 267 data` (`BONUS.BIG`), and
   `178 total, 0 empty, 2 zero, 176 data` (`E3.BIG`); see
   `docs/changes/big-slots/archive.md` and the `big-header` slice
   (`docs/changes/big-header/archive.md`).
- `slot-name` (playtest-accepted: 1287/1287 `TEXTURES.BIG` slots named,
  shared names refused): `--map <TEX_HD.MAP>` decodes the engine's own alias
  matrix (exactly 384,000 bytes = 6,000 rows x 64 bytes, 16 little-endian
  `u32` codes per slot row, `0xFFFFFFFF` = none) into numeric asset names.
  `--tlist --map` appends `; names <n1>,<n2>,...` (or `; names -`) to each
  slot line; `--tshow <name> --map` accepts `<id>_<sub>`, `M\<id>_<sub>`, or
  `B\<id>_<sub>` and refuses to guess when a name is shared by several slots
  (loud error listing the candidates, no window). Blocks: `TEXTURES` base 0,
  `MOVIES` base 2,000, `BONUS` base 4,000; `E3.BIG` has no block and stays
  index-only. Without `--map`, every prior line, message, and exit code is
  unchanged. Names are the engine's numeric asset names, not descriptive
  labels. Format proof in `docs/notes/s8-big-slot-names.md`; archive in
  `docs/changes/slot-name/archive.md`.
- `models-3d` (playtest-accepted on a real mesh): `cargo run -- --mesh
  <FILE.SRM>` parses one SR1-remaster loose mesh (format verdict: magic
  `SRM\x01`, 44-byte shader records, 32-byte material records, bone
  block, 128 flag bytes, 32-byte vertex stride, `u16` triangles — both
  planning probes land exactly on EOF) for positions + triangle indices
  ONLY (normals, UVs, weights, materials, bones skipped as opaque byte
  ranges) and shows it as a flat-shaded auto-rotating solid (fixed
  perspective, derivative face normals, culling OFF, depth ON, CPU
  auto-fit to a unit cube, ~0.6 rad/s; Esc or window close exits 0). It
  prints `N vertices, M triangles` (counts only) to stdout; bad magic,
  truncated headers, insane counts, and short vertex/face data fail
  loud (stderr + exit 1, no window). Mesh bytes stay in process memory
  (never written to disk or repo); tests use synthetic bytes only.
   Playtest file: the 86-vertex / 56-triangle loose SRM named by relative
   name in `docs/changes/models-3d/proposal.md`, resolved against the
   user's own install at runtime.
- `animation` decode + one-bone pose (PRs 1-3 of 3): `--mesh` decodes the
  `.SRM` bone block — `extra` HD 3x4 matrices, 128 joint slots, per-vertex
  bone indices and weights — validates orthonormal matrices, 255-weight
  triples, and indices below 128, and prints
  `skeleton: K matrices, 128 joints`. `cargo run -- --mesh <FILE.SRM>
  --pose <bone>:<degrees>` rotates exactly one decoded bone's weighted
  vertices (integer bone `0..=127`, finite `|degrees| <= 360`), prints
  `pose bone B, D deg, V vertices moved`, and shows the deformed mesh
  through the unchanged auto-rotating flat-shaded path. The blend is
  one-bone linear blend skinning in mesh space (joint Z negated per design
  P5); a vertex moves by `p + (w/255) * (R(p - head) - (p - head))`, so
  seams stay continuous and zero-weight vertices are untouched. Without
  `--pose`, the `--mesh` path is byte-for-byte unchanged. `extra > 0` models
  still decode but `--pose` refuses them because the matrix/joint relation
  is UNKNOWN; malformed specs, out-of-range or unweighted bones, and
  `--pose` in any other mode all fail loud (stderr + exit 1, no window).
  Bone hierarchy, parenting, and `.SRA` clip playback remain missing.
  Supersedes the "bones skipped" note in the `models-3d` bullet.
- `s1-bigfile-loader` (playtest-accepted error parity, SR1 only):
  open-stage failures (`--bigfile <absent> --list`) report the
  original loader's load-failure prefix —
  `Can't load big file named "<path>" (<OS detail>)`, exit 1. Five
  post-open failure rows keep their existing messages (stage mapping
  unconfirmed — no call-edge evidence, held by regression tests);
  termination behavior unchanged. `cargo test` 86/86 green on
  synthetic bytes (see `docs/changes/s1-bigfile-loader/archive.md`).
- `s2-read-error-parity` (docs-only, SR1 only): the bounded call-edge
   trace (2 of 2 probe passes) found zero caller-to-callee edges into
   either error half, so no post-open row resolved to PROVEN and no
   production message was aligned. All 5 post-open failure rows stay
   HELD with their existing prefixes pinned by new regression tests
   (truncated count/bad count, truncated index table, past-EOF,
   seek-failure, short-read); termination unchanged; loud-stderr +
   exit-1 contract untouched. `cargo test` 88/88 green on synthetic
   bytes (see `docs/changes/s2-read-error-parity/archive.md`).
- `s3-launcher-run` (docs-only, SR1 only): launcher-to-engine handshake
   documented — SRX.exe has NO static import of `run` from `sr1.dll`
   (L1 NOT CONFIRMED); it dynamically loads the engine library via
   `LoadLibraryA` with the `1\sr1.dll` literal and resolves `run` via
   `GetProcAddress` in the same function (L2 CONFIRMED, so linkage is
   dynamic); `run` is the engine entry (also ordinal 1) and references
   the default archive name `bigfile.dat` within its first bytes (L4
   CONFIRMED). Engine startup contract beyond entry stays PARTIAL/UNKNOWN.
   `cargo test` 88/88 green (no code touched; see
   `docs/changes/s3-launcher-run/archive.md` and
   `docs/notes/s3-launcher-run-behavior.md`).
- `f4-launcher-cli` (playtest-accepted: no-op flags strip cleanly, unknown arg
   still fails loud): the launcher flag surface found in `SRX.exe` strings is
   replicated as a pure parser (`src/launcher_cli.rs`: `parse` +
   `LauncherArgs` + `Game` + `EgsEnv` + `LauncherCliError`; no IO, env, or
   process spawn) with unit tests on synthetic argv only. Every flag effect
   stays UNKNOWN (static evidence only). The viewer strips
   `--nosteam`/`-nocheck`/`-skipintro` (single- and double-dash forms) before
   dispatch as compatibility no-ops — no output, no exit-code change; the
   remaining flags are parser-only. `-nocheck`/`-skipintro` are absent from
   this build, so they exist only as compatibility no-ops. `cargo test` 102/102
   green (see `docs/changes/f4-launcher-cli/archive.md`).
- `audio-snd` (playtest-accepted: one wave decoded and heard — the tone sounded
  natural, and 44100 Hz is confirmed by ear; the scratch WAV stays in `%TEMP%`):
  the paired `.SND`+`.SMP` instrument banks under `MUSIC/<track>/` are
  readable. `cargo run -- --audio <FILE.SND> --alist` parses the 40-byte
  `DNSa` header, the program (8 B) and zone (16 B) tables and the `u32`
  wave/sequence/label offset tables, validates every structural invariant,
  and lists each wave's offset, bytes, frames, decoded samples, duration and
  loop span. `--audio <FILE.SND> --wave <n> --out <FILE.wav>` decodes exactly
  one wave from the paired `PMSa` sample bank as Sony 4-bit ADPCM (16-byte
  frame, 28 samples, filters 0-4, flag 7 stops before its samples) and writes
  a canonical 44-byte RIFF/WAVE PCM16 mono file. The container stores no
  sample rate: 44100 Hz is the community convention and every message prints
  it as `(chosen)`; the audible playtest accepted 44100, so the 22050
  alternative was not needed. The writer refuses a missing parent and any path
  inside a git working tree, so game audio cannot enter this repository; the WAV
  is a scratch playtest artifact, never an export path. Waves decode one-shot
  even when the bank marks them looped (the loop span is reported only). No
  sequence playback, no mixing, no new dependency; tests use synthetic bytes
  only (see `docs/notes/s9-audio-snd.md` and
  `docs/changes/audio-snd/archive.md`).

## What's missing

- [x] `.BIG` fixed-slot layout and slot decoding, slices `big-slots` +
      `big-decompressor` (see `docs/changes/big-slots/design.md` and
      `docs/changes/big-decompressor/design.md`): the four containers share a
      verified 512-byte header plus entry count (1287 / 29 / 268 / 178
      respectively) and `4096 + count * 131072`-byte fixed slots.
      `--tlist` lists every slot with index/offset/class and a per-class
      footer, exits 0; `--tshow` decodes one `data` slot as a raw 256 x 256
      little-endian A1R5G5B5 image and shows it, while `empty`/`zero` slots
      stay loud. Verdict: `data` slots are NOT compressed, so no decompressor
      is needed. Visual acceptance of decoded slots is a playtest, not a test.
- [x] 3D models, slice 1 `models-3d` (parse-one + render-one, see
      `docs/changes/models-3d/`): single loose SRM as a flat-shaded
      auto-rotating solid. Still unscoped: skeleton/animation, room
      meshes, multi-mesh browsing, textured meshes.
- [x] Animation, slice `animation` (one-bone pose only): `--mesh --pose
      <bone>:<degrees>` deforms one `extra = 0` model for the playtest.
      Still missing: skeleton point/chain overlay, bone hierarchy and
      parenting, animation clips (`.SRA`/`.SRL`), `extra > 0` HD matrix
      interpretation, and bone names. Next candidates are the joint
      overlay and `.SRA` + Ghidra; room meshes, multi-mesh browsing and
      textured meshes stay separate scopes.
- [ ] Audio follow-ups (unscoped, after animation): 322 of 841 waves are looped
      but rendering is one-shot (the loop span is reported only); the 155
      `QSMa` sequences are still unread and need a MIDI-like interpreter, SPU
      ADSR envelopes and pitch resampling. Multi-track playback and the other
      audio banks are separate scopes. See
      `docs/changes/audio-snd/archive.md` and `docs/notes/s9-audio-snd.md`.
- [ ] Binary loader follow-ups (UNSCOPED, TBD by user): S2 and S3 both
      landed docs-only — S2's static call-edge tracing resolved zero
      post-open rows; S3 confirmed the launcher reaches `run`
      dynamically but the engine startup contract beyond entry stays
      UNKNOWN. Next binary slice open: a DYNAMIC-trace candidate to
      resolve the still-HELD post-open rows and the still-UNKNOWN
      contract rows, or S4 `run` internals — each needs its own slice
      cycle, same slice discipline. F4 launcher CLI is done (see
      `docs/changes/f4-launcher-cli/archive.md`); its remaining
      parser-only flags get wired only if a future slice needs them.
- [ ] sRGB correctness pass, resize handling, full mipmap-chain UI —
      still deferred; `dxgiFormat` 99 (BC7 sRGB) still fails loud.
- [ ] Visual playtest confirmation against a reference DDS viewer
      (done by the user per slice; see the change PR).
- [ ] Everything after that (gameplay, UI, SR2-side data) — unscoped

## Next step

Read `AGENTS.md`, then `docs/notes/` (survey + community tools + format
notes). The `.SND`+`.SMP` audio probe and its one-wave slice are done; the
next change is the animation scoping probe for the mesh path.
