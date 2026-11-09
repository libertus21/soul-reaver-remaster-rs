# actor-parameter-program - where the activation program lives and how it is encoded

Bounded, read-only static pass on the shipped SR1 module (Ghidra 12.1.4) plus an
exhaustive read-only scan of the shipped actor containers. Structure and counts
only: roles, record shapes, opcode numbers, counts and small values. No addresses,
no field offsets, no disassembly listings, no raw game bytes, no absolute install
paths. Scratch parsers and decompiled text stayed in `%TEMP%`; the game binary and
the archives were opened read-only and never copied into the repository. No game
process was launched or instrumented.

Date: 2026-11-09. Evidence class per claim: **CONFIRMED** (code and data agree, or
two independent observations agree), **PARTLY KNOWN** (one side pinned), **UNKNOWN**
(explicitly not established).

This note continues the state-writer chain of `actor-transition-table`
(activation writer W1 and the behaviour-entry applier W2) and answers the four
questions that note left open for the activation program: where it lives, how it
is encoded, whether the containers carry literal opcodes, and which model byte
feeds the selector. It does not change production code, does not wire `refresh`
or `advance`, and does not move a golden.

## Verdict first

| Question | Answer | Class |
|---|---|---|
| Where does the parameter program live in the object asset? | **Not as a literal record list.** The applier reads a record-list pointer from a per-object slot populated at load. No shipped actor container (and not the module image) contains a decodable program under the confirmed encoding. The file-side source of the list is **UNKNOWN**. | CONFIRMED negative for literal storage; source UNKNOWN |
| What is the opcode encoding? | Each record is `u16 opcode`, `u16 count`, then `count` × `u32` argument; record size is `4 + 4*count` bytes; a **word** opcode `0` terminates. Opcodes are words, not bytes, at the point of application. | CONFIRMED (code + assembly) |
| Do the 21 actor containers contain opcodes 1, 0x14, 2? | **No.** An exhaustive scan of every actor entry found zero decodable programs, so none carries a literal `1`, `0x14` or `2`. | CONFIRMED negative |
| What is the selector byte? | The applier seeds the AI block's selector/facing byte from a **model byte**; the behaviour-entry applier clamps it to the entry count and indexes the expanded entry array with it. The file-side source is the model block's small selector word (observed `0..15`), whose loader transform is UNKNOWN. | CONFIRMED (code); file mapping INFERRED |
| What do the non-state opcodes do? | The switch handles **19** non-state opcodes (not nine): flags, 16-bit scalars, short-array copies, byte fields and a 32-bit field. Names of several fields stay UNKNOWN. | CONFIRMED (code + assembly) |
| Should a read-only decoder be added? | **No.** The compact stream is not present in any shipped container, so the file layout is not confirmed with zero contradictions; a `--actor-program` reader would have nothing to print. | CONFIRMED |

The headline correction to the task framing: the shipped object assets do **not**
carry the compact opcode stream. They carry compact source tables; the runtime
record list is built in memory at load and the applier walks that in-memory list.
A decoder over the containers cannot reproduce it, so none was added.

## 1. Where the program lives, and how its length is bounded

### 1.1 What the applier reads

The activation applier (writer W1 of `actor-transition-table`) receives one object
pointer and does, in order:

1. It reads the AI block pointer from the object.
2. It reads the **program pointer** from a per-object slot. If that pointer is
   null, or if a per-object flag (the second bit of a flag byte) is set, the
   record list is skipped entirely.
3. Otherwise it walks the list record by record, staging the state and non-state
   writes, then calls the behaviour-entry applier (W2) once, unconditionally, and
   only afterwards commits the staged state overrides.

The program pointer is installed **after load**, not authored as a pointer in the
container. Per the authority (`actor-state-writer` §2.4) this is the same
per-object slot the classifier read for the class-flags word at load-complete,
which is why class resolution must finish before actor initialisation overwrites
the slot. That relationship is taken as given here; it is not re-derived.

### 1.2 Length bounding

There is **no length or count header** on the list. The list is bounded only by
its own terminator: a record whose opcode word is zero. Each record is
self-describing through its `count` field, so the walk advances `4*(count+1)`
bytes per record until it reads a zero opcode. The applier therefore depends on
the list being well formed and zero-terminated; a malformed list would overrun.

### 1.3 The stream is an in-memory product, not a file block

This is the central data finding. A parser for the confirmed encoding was run
over the **entire** bytes of every actor-family container — the 19 archive actor
containers, the two loose `morboss`/`priests` containers, and the player
container — not just the actor block. It found **zero** decodable programs. The
same parser over the full module image also found zero.

Two facts explain the absence:

- Each container carries exactly **one** actor-family signature (the player
  container carries only the player signature). There is no second,
  runtime-shaped blob in the file that a relocator could simply finger into.
- The behaviour-entry applier expects a runtime actor block with an entry-count
  byte and an entry-pointer array, i.e. an **expanded** structure. The file actor
  block instead holds compact records (the trio table and fixed-stride entries)
  and has no entry-pointer array.

So the runtime actor block, and with it the program list, are built in memory at
load from compact file tables. The file-side table that seeds the program list
was **not located**; see the UNKNOWN list. This is an honest negative, not a
misread: the encoding was read from the machine code and then used to reject the
data, and the rejection is exhaustive.

## 2. The confirmed opcode encoding

### 2.1 Record layout

Read from the applier's machine code, not inferred:

| Part | Width | Meaning |
|---|---|---|
| opcode | 16-bit word | dispatch selector; `0` terminates the list |
| count | 16-bit word | number of 4-byte arguments in this record |
| arguments | `count` × 32-bit | per-opcode payload; most opcodes use the low byte or low word of the first argument |

The next record begins immediately after the arguments: the step is
`4 + 4*count` bytes. The terminator contributes only its opcode word; the walker
reads the next opcode first and stops before it would consume a count. Opcodes are
**words** in this form. Whether any earlier, denser file encoding exists and is
expanded into this word form is UNKNOWN (and, given §1.3, likely: the loader
builds the word stream).

The dispatch handles opcodes `1` through `0x11`, then `0x14`, `0x16`, `0x17`,
`0x18` and `0x19`. The values `0x12`, `0x13` and `0x15` are holes: they fall to
the default arm and advance without writing anything.

### 2.2 Worked example (structure only)

Consider a two-record program: the first record has opcode `1`, argument count
`1`, and argument `5`; the second record is the terminator (opcode `0`). The
applier reads record one, stages the value `5` for both current and queued, reads
the zero terminator, calls the behaviour-entry applier, and finally commits
current and queued to `5`. The behaviour entry's own state bytes are overwritten
by that commit because the program is applied after W2. No program bytes are
reproduced here, per the artifact rule.

### 2.3 Post-application remap

After all opcodes are staged and committed, the applier re-applies the
post-action remap already recorded in `actor-transition-table`: if the
entry/action byte is the post-action value `8` and the queued byte is neither `4`
nor `0x10`, then current and queued are forced to `4`. This confirms that the
`4`/`0x10` exclusion in the earlier note is part of the same routine.

## 3. Per-model literal-opcode presence

Every actor entry was scanned in full. "None" means no decodable program under
the confirmed encoding anywhere in the container (the honest result, not zero
records in a located program). The selector column is the model's small selector
word (see §4) and the entry column is the clean behaviour-entry count from
`actor-behaviour-data` (`soul` and `raziel` are degenerate/player and have no
comparable entry count).

| Model | Family | Entries | Selector word | Opcode 1? | Opcode 0x14? | Opcode 2? | Observed state values |
|---|---|---:|---:|---|---|---|---|
| `skinner` | monster | 9 | 2 | none | none | none | none (no literal stream) |
| `morlock` | monster | 8 | 0 | none | none | none | none |
| `sluagh` | monster | 3 | 0 | none | none | none | none |
| `ronin` | monster | 7 | 0 | none | none | none | none |
| `wallcr` | monster | 8 | 12 | none | none | none | none |
| `hunter` | monster | 4 | 2 | none | none | none | none |
| `vwraith` | monster | 3 | 2 | none | none | none | none |
| `vlgra` | monster | (no clean array) | 3 | none | none | none | none |
| `vlgrb` | monster | (no clean array) | 3 | none | none | none | none |
| `vlgrc` | monster | (no clean array) | 3 | none | none | none | none |
| `aluka` | monster | 10 | 15 | none | none | none | none |
| `walbosb` | monster | (variant) | 1 | none | none | none | none |
| `walboss` | monster | 7 | 6 | none | none | none | none |
| `kain` | monster | (variant) | 10 | none | none | none | none |
| `wrshp` | monster | 8 | 0 | none | none | none | none |
| `alukabss` | monster | 1 | 4 | none | none | none | none |
| `roninbss` | monster | 1 | 9 | none | none | none | none |
| `skinbos` | monster | 5 | 13 | none | none | none | none |
| `morboss` | monster | 1 | 2 | none | none | none | none |
| `priests` | monster | (no clean array) | 13 | none | none | none | none |
| `soul` | monster (degenerate) | (none) | 0 | none | none | none | none |
| `raziel` | player | (player block) | n/a | none | none | none | none |

The selector word exceeds the entry count for several models (`aluka` 15 vs 10,
`skinbos` 13 vs 5). That is consistent with W2 clamping the selector to
`entries - 1`, which is exactly what the code does; it is further support that
this word is the selector and not an index into anything pre-clamped.

## 4. The selector byte and the model field that feeds it

The code path is unambiguous:

1. **Seed.** The applier reads one byte from the loaded actor block and writes it
   into the AI block's selector/facing byte. Opcode `6` can overwrite that same AI
   byte later in the same activation.
2. **Clamp.** The behaviour-entry applier (W2) reads the AI selector byte, and if
   it is greater than or equal to the actor block's entry count, it replaces it
   with `entry_count - 1`.
3. **Select.** W2 then indexes the expanded behaviour-entry pointer array with the
   clamped selector (fixed 8-byte pointer stride) and reads the selected entry's
   state byte, action byte and parameter fields into the runtime.

The **file-side source**: the model block's small selector word. It was observed
across the 21 monster containers at values `0..15`, and its over-range cases
(`aluka`, `skinbos`) are exactly the cases the runtime clamp must absorb. The file
layout's constant byte that sits in the trio table at row 4, column 2 is `11` for
every monster and therefore **cannot** be the selector; that rules out the naive
"the applier reads a trio byte" reading.

What is **not** proven: the loader transform that turns the file-side selector
word into the runtime actor-block selector byte. The runtime block is an expanded
structure (§1.3), so the two byte positions are not the same field; the mapping is
inferred from value range and clamp consistency, not traced.

## 5. The non-state opcodes

The switch has **19** non-state arms, not the nine the earlier framing assumed.
Grouped by destination kind (no field names are invented where the evidence does
not establish one):

| Opcode | Argument use | Destination kind | Established role / class |
|---|---|---|---|
| `3` | none | boolean object flag | toggles one high object flag bit (also set by W2 when an entry field is non-zero) — CONFIRMED write, name UNKNOWN |
| `4` | low 16 bits | 16-bit AI scalar | same field W2 seeds from a behaviour-entry word; reach/range family — Partly known |
| `5` | low 16 bits | 16-bit AI scalar | same field W2 seeds from a behaviour-entry word — Partly known |
| `6` | low byte | AI selector/facing byte | overwrites the selector seeded in step 1 of §4 — CONFIRMED |
| `7` | none | AI flag mask bit | ORs a mask bit later XORed into the AI flag word — CONFIRMED write, role UNKNOWN |
| `8` | low 16 bits | 16-bit AI scalar | stored as `value * 256` — CONFIRMED write, name UNKNOWN |
| `9` | count + low 16 bits | object flag + AI flag + 16-bit scalar | sets object and AI flags; writes a scalar only when the count is non-zero — CONFIRMED writes |
| `0xA` | count shorts | short-array copy | copies one low short per argument into an inline AI array and stores the array pointer — CONFIRMED |
| `0xB` | low byte | AI byte field | CONFIRMED write, name UNKNOWN |
| `0xC` | low byte | AI byte field | CONFIRMED write, name UNKNOWN |
| `0xD` | low 16 bits | 16-bit AI scalar | CONFIRMED write, name UNKNOWN |
| `0xE` | count shorts | short-array copy | copies one low short per argument into another inline AI array — CONFIRMED |
| `0xF` | 32 bits | 32-bit AI field | CONFIRMED write, name UNKNOWN |
| `0x10` | low byte | AI byte field + 32-bit default | writes a byte, and if the `0xF` field is still zero sets it to a sentinel maximum — CONFIRMED |
| `0x11` | none | AI flag mask bit | clears the top bit of the AI flag word — CONFIRMED |
| `0x16` | low 16 bits | 16-bit AI scalar | CONFIRMED write, name UNKNOWN |
| `0x17` | 32 bits | 16-bit AI scalar | converts `value * 4096` through a fixed-point divide into a day-phase/turn quantity — CONFIRMED conversion, units UNKNOWN |
| `0x18` | 32 bits | 16-bit AI scalar | same conversion as `0x17`, different destination scalar — CONFIRMED |
| `0x19` | low byte | AI byte field | CONFIRMED write, name UNKNOWN |

The three state opcodes are listed here for completeness: `1` sets current and
queued, `2` sets entry/action, and `0x14` sets alternate, all to the first
argument (low byte). Those remain exactly as `actor-transition-table` records.

## 6. Falsification ledger

| Claim | Observation that would refute it | Tested? | Result |
|---|---|---|---|
| The record is `u16 opcode, u16 count, count×u32` and steps `4*(count+1)` | a different read width for opcode or count, or a different step | Disassembly of the applier's loop and dispatch | CONFIRMED |
| Opcode `0` terminates and there is no length header | a separate length/count read before the loop | Disassembly of the loop entry and termination test | CONFIRMED |
| The program is not a literal stream in the containers | one decodable program in any actor entry | Exhaustive scan of all 22 actor-family entries | CONFIRMED negative |
| The program is not a literal stream in the module | one decodable program in the image | Exhaustive scan of the module image | CONFIRMED negative |
| The runtime actor block is an expansion, not a file blob | a second signature-bearing block in a container, or W2 reading fields the file block actually has | Signature census (exactly one per container) + W2 field shape | CONFIRMED |
| The selector is a model byte, clamped by the entry count | W2 not clamping, or selecting without the count | Decompiled W2 | CONFIRMED |
| The selector is the model's small selector word | a constant-invalid or clamped-inconsistent value | Corpus values `0..15` vs entry counts | INFERRED (consistent) |
| The trio-table constant byte is the selector | that byte varying with the selector | Corpus: constant `11` for every monster | REFUTED |
| Nineteen non-state opcodes, not nine | a non-state arm beyond the enumerated set, or a listed arm that writes state | Full switch enumeration | CONFIRMED |
| The post-action `4`/`0x10` exclusion is in the applier | the remap lacking the exclusion | Decompiled the tail of the applier | CONFIRMED |

## 7. Explicit UNKNOWN list

1. The file-side table that seeds the runtime program list, and the loader
   transform that expands it into the word-based record stream. This is the
   primary gap.
2. Whether the file encodes opcodes as bytes (and the loader widens them) or the
   list is built entirely from non-program fields.
3. The semantic names of the 16-bit scalars (opcodes `4`, `5`, `8`, `0xD`, `0x16`,
   `0x17`, `0x18`) and the byte fields (`0xB`, `0xC`, `0x10`, `0x19`).
4. The exact units of the `0x17`/`0x18` fixed-point conversion.
5. The exact file-to-runtime byte mapping for the selector word.
6. The concrete bit identities behind the boolean/flag opcodes (`3`, `7`, `9`,
   `0x11`) beyond their write sites.
7. Any runtime observation of a real activation list being applied.

## 8. Method

- **Static.** Ghidra 12.1.4 on the shipped SR1 module, read-only. Decompiled and
  read the assembly of the activation applier (staging loop, dispatch, record
  step, terminator, post-application remap) and the behaviour-entry applier it
  calls. The opcode encoding in §2 comes from the machine code, not from a data
  guess.
- **Data.** A read-only parser for the confirmed encoding was run over the entire
  bytes of all 22 actor-family containers (archive and loose) and over the module
  image. It found zero programs. A signature census found exactly one
  actor-family signature per container.
- **Cross-check.** The runtime entry applier's field shape (entry-count byte,
  entry-pointer array, 8-byte stride) against the compact file actor block
  established that the runtime block is an expansion.
- **No execution.** No game process was launched or instrumented.

`cargo test` is 486 green (the `main` baseline at head `b3a3ab7`, PR #115
merged). This is a docs-only change: no production code, no golden moved, and no
decoder added (see §9).

## 9. Why no decoder was added

The optional `--actor-program --bigfile <DAT> --entry <N>` reader was gated on a
layout confirmed with zero contradictions. There is a contradiction with the
premise itself: the compact program is absent from every shipped actor container
(§1.3, §6). A reader over the container would therefore never print a program,
and writing one would imply a file layout that the data refutes. The decoder is
deliberately omitted. If a future pass locates the loader transform, the natural
next capability is to decode the **synthesised runtime list** at load time in a
controlled run, not to scan the container.
