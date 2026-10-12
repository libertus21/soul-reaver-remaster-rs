# Spec: bigfile-index

`cargo run -- --bigfile <file> --list` exits 0 and prints the archive's entry index (one line per entry: index, name-hash, offset, length — nothing invented). `cargo run -- --bigfile <file> --show <entry>` reads that entry into memory, verifies it is a DDS texture, decodes it through the EXISTING pipeline, and shows it in the EXISTING window. `cargo run -- <file>` and `cargo run -- <dir>` keep their exact slice-4 behavior.

## Quick path

1. Run `cargo run -- --bigfile "<install>\1\BIGFILE.DAT" --list` (any bigfile path; runtime-only, never committed).
2. Pick a DDS entry; run `cargo run -- --bigfile "<install>\1\BIGFILE.DAT" --show <entry>`.
3. Compare the rendered texture by eye against the loose-file render of the same texture if identifiable, else against a reference viewer (user playtest note records entry + comparison + result).

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI keeps its current forms UNCHANGED: one positional arg that is a file opens single-file mode, a directory opens gallery mode, anything else is the existing loud usage error. `--bigfile <path>` is a NEW flag form: `--list` lists the index, `--show <entry>` shows one entry. Unknown/missing flag arguments are a LOUD error (usage + nonzero exit) |
| R2 | `--list` parses the index per the SR1PC layout (u32 LE count at offset 0, then 4×u32 LE entries) and prints one line per entry carrying the spec-provided fields only: decimal index, `nameHash` as 8 hex digits, `offset`, `length` (whatever the spec provides, nothing invented). Entry count N is whatever the real file declares — recorded at apply time, never hardcoded |
| R3 | `--show <entry>` accepts the decimal list index OR the 8-hex-digit name-hash printed by `--list`. Unknown selector, out-of-range index, or unmatched hash is a LOUD error + exit 1 |
| R4 | `--show` reads exactly `[offset, offset+length)` into process memory, validates the range against the real file size first (out-of-range index data is a LOUD error, never a panic or a short read presented as success), and NEVER writes extracted bytes to disk or repo |
| R5 | `--show` decodes ONLY DDS entries: the in-memory bytes must start with the `DDS ` magic and decode through the EXISTING `dds` validation + RGBA8/CPU-BC7 path (shared with single-file mode; no new decoder). A non-DDS entry (any other archived type) is a LOUD error naming the entry + exit 1 — listed, not decoded |
| R6 | Rendering reuses the EXISTING window (`show_rgba8`-family) unchanged: same title/exit contract as single-file mode (close/Esc exits 0). No new GPU format, no new upload path |
| R7 | Corrupt/truncated input fails LOUD with exit 1: bad count (unreasonable vs file size), truncated index, truncated entry data, or non-DDS `--show` target each print a clear stderr message; no guessing, no partial output presented as success |
| R8 | No game bytes, paths, hashes, or offsets committed; whitelist `.gitignore` verified per commit; tests use SYNTHETIC container bytes only (minimal fake bigfiles built in temp dirs — the real file is never touched by tests) |

## Acceptance scenarios

- [ ] **Happy path (`--list`):** `cargo run -- --bigfile <real BIGFILE.DAT>` exits 0 and prints N entries (N recorded from the real file at apply time — planning probe observed 1325, apply MUST confirm and record the authoritative N).
- [ ] **Happy path (`--show`):** `--show` on a real DDS entry renders it in the existing window; user playtest compares vs the loose-file render of the same texture if identifiable, else vs a reference viewer, and records entry + comparison + result.
- [ ] **Corrupt/truncated:** pointing `--bigfile` at a truncated/corrupt file (or `--show` past the end) prints a loud stderr error and exits 1; no window opens, no partial listing presented as success.
- [ ] **Non-DDS entry:** `--show` on a listed non-DDS entry prints a loud stderr error naming the entry and exits 1 (listed, not decoded).
- [ ] **Selector errors:** `--show` with an out-of-range index, unknown hash, missing `--list`/`--show` verb, or missing path fails loud with usage + nonzero exit.
- [ ] **Modes unchanged:** `cargo run -- <one real DDS>` renders exactly as in slice 4, and `cargo run -- <real TEX dir>` browses exactly as in slice 4 (no flag-form regressions).
- [ ] **Unit level:** `cargo test` covers index parsing (count + entries from synthetic bytes), range validation (truncated index/entry rejected), and DDS-magic gating on synthetic entries — without touching game files.
- [ ] **Legal:** `git status --short` shows only intended source/doc files; no extracted bytes, no game paths/hashes/offsets committed.

## Next step

See `design.md` for the spec source, the UNKNOWN gaps with the byte-level fallback plan, and module decisions; `tasks.md` for the ordered work units.
