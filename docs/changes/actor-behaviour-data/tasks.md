# Tasks: actor-behaviour-data (census reader)

## T1 - Actor block parser

- Add `ActorBlock`, `BehaviourEntry`, `ActorKind`, `EntryArrayKind` and
  `parse_actor_block` in `src/object.rs`.
- Reuse `level::data_base`; read the object header, the `+0x1C` pointer and the
  block signature. Return `Ok(None)` for a zero/out-of-range pointer or a
  non-actor signature.
- Acceptance: a synthetic actor container yields `Monster`; a synthetic
  non-actor yields `Ok(None)`; a truncated block yields a loud error.

## T2 - Trio table

- Read the 24 bytes at block `+0x18` into `[[u8; 3]; 8]`.
- Print rows in stored order; never name them idle/attack/etc.
- Acceptance: a synthetic trio table round-trips byte-for-byte.

## T3 - Behaviour-entry detector

- Implement the pointer-candidate detector from `design.md` (0x20 stride,
  `+0x1B` id, `+0x1C` sentinel, reject the `0x00,0x10` sub-table).
- Return `Clean` with ids, `Variant` for a different record shape, or `None`.
- Acceptance: a 0x10-stride sub-table is not accepted; a 0x20 array with a
  non-`0xFFFFFFFF` trailing sentinel is accepted; a 0x20 array with a missing
  id is rejected.

## T4 - `--actor-census`

- Add the CLI arm in `src/main.rs`: exactly `--bigfile <path>` (PR B).
- For each archive entry, parse the object container and the actor block;
  collect actor rows, sort by entry index, print the per-actor line and the
  summary line from `spec.md`.
- Acceptance: `skinner` -> 9 entries / 5 distinct ids; `morlock` -> 8 / 4;
  `aluka` -> 10 / 4; non-actors produce no row.

## T5 - `--object-info --actors`

- Add an optional `--actors` flag to the existing arm.
- Without it, output is byte-for-byte unchanged.
- With it, append the actor block lines or `actor block: none`.
- Acceptance: the existing `info_lines` assertions still pass; a synthetic
  actor entry gains the two actor lines.

## T6 - Loose `.PCM` coverage (PR C)

- Add `--objects-dir <path>` to `--actor-census`; walk `*.pcm` recursively in
  sorted order and parse each in memory.
- Acceptance: `morboss` and `priests` appear; a missing directory is exit 3; no
  file is written.

## T7 - Tests (synthetic bytes only)

- Extend the synthetic container builder with an actor-block writer.
- Cases: actor with 0/1/N entries; non-actor signature; 0x10 sub-table;
  out-of-range pointer; truncated block; exact line shape; unchanged
  `--object-info`.
- Acceptance: new tests pass; no test reads a game file; no game bytes in the
  repository.

## T8 - Docs and regression

- Add the reader section to `docs/notes/actor-behaviour-data.md`, the README
  line, and the change archive entry.
- Run `cargo test`; confirm the existing level, mesh, texture, audio and object
  arms are unchanged; `git status` shows only `*.rs` and `*.md`.
- Acceptance: 321+ tests green, no game data staged.
