# Spec: textures-big

`cargo run -- --textures <file> --tlist` exits 0 and prints the archive's entry index when an entry table is identified (one line per entry carrying the identified layout's fields only — nothing invented), else exits 1 loud with the observed layout summary. `cargo run -- --textures <file> --tshow <entry>` reads that entry into memory, verifies it is a DDS texture, decodes it through the EXISTING pipeline, and shows it in the EXISTING window. `cargo run -- <file>`, `cargo run -- <dir>`, and both `--bigfile` forms keep their exact prior behavior.

## Quick path

1. Run `cargo run -- --textures "<install>\1\TEXTURES.BIG" --tlist` (any textures path; runtime-only, never committed).
2. When entries are listed: pick a DDS entry; run `cargo run -- --textures "<install>\1\TEXTURES.BIG" --tshow <entry>`.
3. Compare the rendered texture by eye against the loose-file render of the same texture if identifiable, else against a reference viewer (user playtest note records entry + comparison + result).

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI keeps its current forms UNCHANGED: one positional arg (file/dir), `--bigfile <path> --list/--show`, and the loud usage error. `--textures <path>` is a NEW flag form: `--tlist` lists, `--tshow <entry>` shows one entry. Unknown/missing flag arguments are a LOUD error (usage + nonzero exit) |
| R2 | `--tlist` prints one line per entry carrying the identified layout's fields only (decimal index plus whatever the layout provides — nothing invented). When no entry table is identified, `--tlist` prints the observed layout summary (size class, region classes, table status — counts/codes/field-shapes only) to stdout and exits 1 with a loud stderr message naming the file and the next container candidate |
| R3 | `--tshow <entry>` accepts the decimal list index OR the entry's stable identifier as printed by `--tlist` (forms fixed at apply time to match the identified layout). Unknown selector, out-of-range index, or unmatched identifier is a LOUD error + exit 1. When no table is identified, `--tshow` is a LOUD error + exit 1 naming the file and the next candidate |
| R4 | `--tshow` reads exactly the entry's byte range into process memory, validates the range against the real file size first (out-of-range data is a LOUD error, never a panic or a short read presented as success), and NEVER writes extracted bytes to disk or repo |
| R5 | `--tshow` decodes ONLY DDS entries: the in-memory bytes must start with the `DDS ` magic and decode through the EXISTING `dds` validation + RGBA8/CPU-BC7 path (shared with single-file and `--bigfile --show` modes; no new decoder). A non-DDS entry (any other archived type) is a LOUD error naming the entry + exit 1 — listed, not decoded |
| R6 | Rendering reuses the EXISTING window (`show_texture` tail: `LoadedTexture` → CPU BC7 or RGBA8 → `show_rgba8`) unchanged: same title/exit contract as single-file mode (close/Esc exits 0). No new GPU format, no new upload path |
| R7 | Corrupt/truncated input fails LOUD with exit 1: unidentified table, truncated entry data, or non-DDS `--tshow` target each print a clear stderr message; no guessing, no partial output presented as success |
| R8 | No game bytes, paths, hashes, offsets, or content dumps committed; whitelist `.gitignore` verified per commit; tests use SYNTHETIC bytes only (hand-built fake headers/regions in temp dirs — the real file is never touched by tests) |

## Acceptance scenarios

- [ ] **Happy path (`--tlist`, table found):** `cargo run -- --textures <real TEXTURES.BIG> --tlist` exits 0 and prints N entries (N recorded from the real file at apply time — planning probe found NO table, so apply MUST record either the authoritative N or the confirmed no-table outcome).
- [ ] **No-table path (`--tlist`, table not found):** exits 1, stdout carries the layout summary (counts/codes only), stderr names the file + the next container candidate. This is specified behavior, not a failure.
- [ ] **Happy path (`--tshow`):** `--tshow` on a real DDS entry renders it in the existing window; user playtest compares vs the loose-file render of the same texture if identifiable, else vs a reference viewer, and records entry + comparison + result. (Skipped with reason when no DDS entry exists.)
- [ ] **Corrupt/truncated:** pointing `--textures` at a truncated/corrupt file (or `--tshow` past the end) prints a loud stderr error and exits 1; no window opens, no partial listing presented as success.
- [ ] **Non-DDS entry:** `--tshow` on a listed non-DDS entry prints a loud stderr error naming the entry and exits 1 (listed, not decoded).
- [ ] **Selector errors:** `--tshow` with an out-of-range index, unknown identifier, missing `--tlist`/`--tshow` verb, or missing path fails loud with usage + nonzero exit.
- [ ] **Modes unchanged:** `cargo run -- <one real DDS>` renders exactly as before, `cargo run -- <real TEX dir>` browses exactly as before, and both `--bigfile` forms behave exactly as archived (no flag-form regressions).
- [ ] **Unit level:** `cargo test` covers region classification (zero-fill / `0xFF`-fill / data / magic windows from synthetic bytes), layout-summary output, range validation, and the DDS-magic gate — without touching game files.
- [ ] **Legal:** `git status --short` shows only intended source/doc files; no extracted bytes, no game paths/hashes/offsets/content dumps committed.

## Next step

See `design.md` for the probe ledger, the UNKNOWN gaps with the byte-level fallback plan, and module decisions; `tasks.md` for the ordered work units.
