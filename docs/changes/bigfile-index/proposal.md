# Proposal: bigfile-index

Index the remaster's `BIGFILE.DAT` and show ONE archived texture in the existing window: `cargo run -- --bigfile <BIGFILE.DAT> --list` prints the entry index (hash, offset, length per entry — whatever the spec provides, nothing invented); `cargo run -- --bigfile <BIGFILE.DAT> --show <entry>` decodes that entry IN MEMORY (never writes extracted bytes to disk or repo) and shows it through the proven RGBA8 path. This is the smallest change that proves we can read the Gex-family archive without an extraction step.

## Quick path

1. `cargo run -- --bigfile <BIGFILE.DAT> --list` exits 0 and prints N entries (N recorded from the real file at apply time).
2. `cargo run -- --bigfile <BIGFILE.DAT> --show <entry>` renders one DDS entry in the existing window (user playtest).
3. Verified with `cargo test` (synthetic container bytes only), a user playtest on the real file, and a `git status` whitelist check.

## Details

| Topic | Decision |
|---|---|
| Intent | Prove archive readability end to end (index parse + in-memory entry read + existing decode + existing render) with zero bytes written to disk, so later slices (browser UI, more types) build on a proven reader |
| Input contract | The bigfile path arrives via `--bigfile <path>` at runtime only; absolute game paths never committed; no game bytes in repo or on disk outside process memory (same legal rule as slices 1–4) |
| Spec source | Soul Spiral's `BenLincoln.TheLostWorlds.CDBigFile` (mirror `github.com/TheSerioliOfNosgoth/SoulSpiral-Official`, GPL-3.0-only per file headers) + corroboration from `cdcengine.re/docs/files/bigfile`. Facts only, clean-room reimplementation — NO Soul Spiral code copied (see `design.md` for exact URLs, license, and field coverage) |
| Index layout (verified) | SR1PC type: no global magic; u32 LE entry count at offset 0; entries of 4×u32 LE `[nameHash, length, offset, unknown]`; raw byte offsets (multiplier 1); little-endian; names are hashes resolved only via Soul Spiral's external lookup table (we print raw hashes) |
| Scope | DDS-texture entries only for `--show`; every other entry type is listed, never decoded. Single-file and gallery modes byte-for-byte unchanged |
| Failure policy | LOUD everywhere: truncated/corrupt container, out-of-range entry, or non-DDS `--show` target each print a clear stderr error + exit 1 (mirrors the single-file die-loud policy; no gallery-style skipping — an archive index must be exact, not best-effort) |
| Non-goals | Archive browser UI, extraction-to-disk, `TEXTURES.BIG` (magic UNKNOWN), `E3.BIG`/`BONUS.BIG`/`MOVIES.BIG`, audio, models, hash-name reverse lookup table, Ghidra |
| Slice boundary | `--list` + `--show`-one-DDS + docs; everything else is a follow-up slice |

## Checklist

- [ ] Reviewer can state the slice's one deliverable without re-reading (list the index, render one archived DDS from memory).
- [ ] Reviewer can state what is out of scope (browser UI, extraction, TEXTURES.BIG, other archives, non-DDS decode).
- [ ] Reviewer can state the failure policy and why it differs from gallery mode (die-loud: an index must be exact).
- [ ] Legal guardrails hold: whitelist `.gitignore` intact, no game bytes committed or extracted, game paths via CLI args only, spec source + license credited in `design.md`.

## Next step

Read `spec.md` for acceptance scenarios, then `design.md` and `tasks.md`. Apply phase follows with a SINGLE PR (forecast in `tasks.md`).
