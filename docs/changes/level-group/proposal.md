# Proposal: level-group (render one level family)

## Intent

Add a family selector to the archive-backed level arms so every room of one
level family is discovered and shown together:

```text
--level-geometry --bigfile <BIGFILE.DAT> --group <PREFIX>
--level-render   --bigfile <BIGFILE.DAT> --group <PREFIX>
```

`<PREFIX>` is the shared self-name stem (`under` matches `under1..underN`).
The existing loose-path, `--entry`, and `--bigfile --list/--show` forms keep
their exact behavior.

## Discovery rule

1. Load the archive index once.
2. Read each entry into memory and try the in-memory `.PCM` reader.
3. Keep only entries that parse and whose self-name (header word `0x26`) is
   exactly `<PREFIX>` followed by one or more ASCII digits.
4. Sort matched rooms by their numeric suffix, then report name + `V`/`F`
   each, followed by one totals line.

A non-`.PCM` entry is skipped. Empty prefix, no match, and a matched room
whose geometry fails to parse are distinct loud exit-1 errors with no window.
`--group` is mutually exclusive with `--entry` (its own message).

## World-space merge

Room vertices are already world coordinates, so the merge is a plain
concatenation: append each room's `[f32; 3]` positions and add that room's
running vertex base to every one of its `u16` face indices. No transform,
rotation, or per-room translation is applied. The running base must stay
within the 16-bit index range; exceeding it is a loud error rather than a
wrapped index.

## Real proof (read-only, in memory)

`--group under` on the SR1 archive discovers four rooms:

```text
under1 1468/2922  under2 252/472  under3 657/1174  under4 269/508
group under rooms=4 vertices=2646 faces=5076
```

Each room reports `out_of_range=0`. The bounding boxes chain contiguously on
`x`, confirming the shared world space. `--group nosuch` and an empty prefix
both exit 1 with their own message. `--level-render --group under` prints the
total line before the window opens.

## Non-goals

Textures, objects, BSP, per-room transforms, on-disk extraction, and new
dependencies. Tests use synthetic archive bytes only; no game bytes or install
paths are committed.
