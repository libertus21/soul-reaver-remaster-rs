# Spec: pcm-loader

Docs-only analysis. Requirements are about what the note must establish, not
about code.

## R1 - Scope

The analysis covers only how the SR1 engine reads `.PCM` level units and
object containers from `1\sr1.dll`. It does not cover SR2, the playback
runtime, writing, or any other format.

## R2 - Method

- Headless Ghidra 12.1.4 on the SR1 binary, project under `%TEMP%`.
- Entry points are string literals and their xrefs. Function names are
  re-pinned by those anchors every run, never hardcoded.
- Arithmetic is confirmed against real units read-only: file size, `4 + 4N`
  and the drain's rounded value are reported side by side, and the corpus
  check is recorded (counts and offsets only).
- Budget: about two passes. Anything past the budget is recorded as UNKNOWN,
  not guessed.

## R3 - Evidence classes

Every claim is tagged:

- **CONFIRMED**: seen directly (string, xref, constant, instruction-level
  shape).
- **INFERRED**: a reasoned step (arithmetic, analogy between two sites).
- **UNKNOWN**: not established; name the probe that would settle it.
- **UNRESOLVED**: two observations conflict and are not reconciled.

## R4 - Required content

The note must establish, with classes:

1. The path roots and which function formats them (L1).
2. The queue, the drain, and the dispatch by type tag (L3-L5).
3. The object instance size and its sub-model structure (L6).
4. The level struct, the header tables, and the 36-to-40-byte record relation
   (L7-L9).
5. The meaning of the 48 in the object tracker (L10).
6. The first-dword rule and the data base (L13), stated as CONFIRMED: the
   first dword is N; the drain strips `4 * roundup512(N)` bytes; directory
   and header offsets are relative to that data base.
7. The name resolver's behavior on the 36-byte record's first field (L15),
   stated as CONFIRMED behavior with UNKNOWN interior.

## R5 - No-go bar

The verdict may be "documented" only if L13, L15 and L16 are CONFIRMED. L13
and L15 (behavior) are now CONFIRMED; L16 and the tagged-record interior are
not, so the verdict stays PARTIAL. A "no-go" wording is reserved for the case
where the pipeline itself is disproved.

## R5a - Reader safety

A reader may implement the container and the level-header shape (count,
directory offsets, data base, tag dispatch) because L13 is CONFIRMED. It must
not name the 36-byte record's fields or treat the first field as a string.
Any note that still uses the `4 + 4N` base is wrong by the padding amount.

## R6 - Legal

No addresses, no disassembly, no decompiled text, no game bytes, no absolute
install paths in any artifact. Scratch stays in `%TEMP%`. A protection or
packing finding stops that line and is reported.

## R7 - Verification

- `git status --short` shows only the planning bundle and the note.
- Grep the bundle for absolute paths, hex dumps and decompiled fragments;
  expect zero hits.
- `cargo test` runs as a no-op baseline only.

## R8 - Out of scope

No `src/` edits, no format change, no CLI change, no `.PCM` writer, no
decompressor implementation, no dynamic tracing in this change.
