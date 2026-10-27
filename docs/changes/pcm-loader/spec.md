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
6. The first-dword conflict (L13), stated as UNRESOLVED.

## R5 - No-go bar

The verdict may be "documented" only if L13, L15 and L16 are CONFIRMED. The
current verdict is PARTIAL because they are not. A "no-go" wording is reserved
for the case where the pipeline itself is disproved.

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
