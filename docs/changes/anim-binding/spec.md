# Spec: anim-binding

Documentation slice. Requirements are about evidence quality and
reproducibility, not behavior, because the change adds no behavior.

## R1 - Scope

The change MUST NOT modify `src/`, `Cargo.toml`, the CLI contract, or any
rendered output. The only artifacts are:

- `docs/notes/anim-binding.md`
- `docs/changes/anim-binding/{proposal,spec,design,tasks}.md`

## R2 - Question and verdict

The note MUST state the question (is there a channel->joint binding in
code?), the verdict (no), and the reason in one paragraph a reader can quote
without reading the ledger.

## R3 - Evidence classes

Every factual claim MUST carry one class:

- **CONFIRMED** - read directly from the binary at byte or instruction
  level, or from the complete string inventory.
- **Observed** - a measurement on real files with a stated sample size.
- **Derived (strong)** - follows from confirmed facts plus constants, with
  no competing reading.
- **INFERRED** - a semantic reading the evidence allows but does not prove.
- **UNKNOWN** - not established, listed as such, never guessed.

Absence claims MUST name the search that produced them (e.g. "whole defined
string inventory", "sampled 64 entries of 3 files").

## R4 - Reproducible method

The note MUST record:

- the project location class (outside repo and game directories),
- that later runs use `-noanalysis` plus post-scripts,
- the re-pin rule: anchor on a string literal or import, never on a
  `FUN_` name, because names shift between analyses,
- that RVAs are file-relative and stable for this PE while names are not.

A reader with the same binary and the same Ghidra version MUST be able to
reproduce the chain in under an hour by following the anchors.

## R5 - No-go quality bar

A no-go MUST satisfy all of:

1. It names the exact consumer that is missing, not just "not found".
2. It states the negative test that rules out the generic mis-parse
   (no `0x0FACE0FF` tags in sampled `.SRA` entries; `4,8,12,...` offsets).
3. It lists what would change the verdict (a non-empty callback, a reader of
   the stored field, or an SR2 parser).
4. It names the cheapest next probe.

## R6 - Legal and hygiene

- No disassembly, decompiled body, hex dump, or game byte in any artifact.
- No absolute install path; game is referenced as `1\sr1.dll` and relative
  object paths only.
- No function address presented as a symbol name; RVAs appear only where
  they are load-bearing and are labelled as file-relative.
- Ghidra project and decompiler scratch stay under `%TEMP%`.
- `git status --short` shows only the five new Markdown files.

## R7 - Verification

- `cargo test` MUST be green and MUST be reported as a no-op baseline
  (`208 passed, 0 failed` at plan time).
- The commit MUST contain no generated file, no project database, and no
  listing.

## R8 - Acceptance scenarios

1. A reviewer who has never opened Ghidra can restate: single `.sra` owner ->
   async raw load -> empty callback -> unused buffer.
2. A reviewer can name the three structures the binary *does* have (object
   slots, sub-model array, spline keys) and why none of them binds curves.
3. A reviewer can name the cheapest next probe and what result would
   overturn the no-go.

## Non-requirements

- No runtime confirmation (that is a separate slice; see the note's probe 2).
- No `.SRL` decode.
- No SR2 binary work.
- No edit to the existing data-side notes.
