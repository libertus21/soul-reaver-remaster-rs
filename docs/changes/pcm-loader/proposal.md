# Proposal: pcm-loader

**NO-GO for implementing a `.PCM` reader now. PARTIAL for the binary
analysis.** A bounded Ghidra survey of `1\sr1.dll` documents the loader
pipeline end to end and the level-record runtime layout. It does not document
the on-disk field semantics, and it leaves one unresolved conflict about the
first dword of the file. A reader written today would guess at both.

## Quick path

1. Read `docs/notes/pcm-loader.md`: the evidence ledger (L1-L16), the
   pipeline, and the ordered next probes.
2. Read `design.md` for the method and the decision record.
3. Read `tasks.md` for the docs-only forecast and the next slice.

No code is produced. No viewer behavior changes.

## Details

| Topic | Decision |
|---|---|
| Intent | Learn how the engine reads `.PCM` level units: record layout, block typing and consumption, and what the 40/48-byte blocks are |
| Verdict | **Partial.** Pipeline and runtime record documented with evidence; on-disk field meaning and decompressor not established |
| 40-byte block | Runtime record (40 bytes) built from a 36-byte source record in the level builder. Source field meanings UNKNOWN |
| 48-byte block | **Not a block.** The `48` is the object tracker's slot count (`MAX_OBJECTS`). No 48-byte record found |
| Blocking conflict | The drain treats the first dword as a 512-rounded byte count; the `.PCM` header is documented as a count N. UNRESOLVED |
| Scope | Docs only. No `src/` change, no format change, no CLI change |
| Method | Ghidra headless, project in `%TEMP%`, re-pinned by string and xref; no hardcoded function names or addresses in artifacts |
| Non-goals | Playback, `.PCM` writing, object decompressor, SR2, `.SRL`, dynamic tracing (queued as a probe) |
| Legal | Behavior notes only. No addresses, listings, decompiled text, game bytes or absolute paths |

## Explicit no-go condition (the implementation reader)

Do not start a reader until **both** hold:

1. L13 is resolved by an observed buffer (dynamic or static) showing what the
   first dword is for a real area `.PCM`.
2. The name resolver (L15) and the 36-byte field meanings (L16) are documented
   with CONFIRMED or derived evidence.

If (1) shows the file really is a size-prefixed wrapper, the header table in
the note is wrong and must be rewritten before any reader is planned.

## Checklist

- [ ] Reviewer can state the verdict without re-reading the note.
- [ ] Reviewer can state the 40-byte source/runtime relation and why 48 is a
      count, not a block.
- [ ] Reviewer can name the unresolved conflict (L13) and the cheapest probe.
- [ ] No address, listing or game byte appears anywhere in the bundle.

## Next step

`tasks.md` for the forecast and the next slice (the L13 probe).
