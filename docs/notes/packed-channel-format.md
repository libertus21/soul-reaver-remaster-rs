# packed-channel-format - the packed frame table is a flag byte, three optional
# 3-bit planes and a palette/literal stream, and a file-resident replay matches
# the live engine exactly (2026-11-12)

This slice closes the packed-channel frame-table question left open by
`docs/notes/producer-selector.md` and `docs/notes/angle-array-source.md`. It
locates the table the channel points at, decodes its per-frame bit planes, its
palette/literal value stream and its clock, and reproduces the engine's own
per-slot output from the file-resident table. The decisive validation is an
end-to-end replay of the **same frame** from `RAZIEL.SRA` against the staging
records the live engine wrote: 82 of 82 populated lanes and 67 of 67 angle lanes
match exactly (maximum absolute error `0.0`), over 66 slots and 3 channels.

Nothing was written to the game. The capture was read-only instrumentation of a
running process; the replay reads the shipped file. No engine code changed and
no golden moved.

Evidence classes: **CONFIRMED** (byte-level or runtime observation with a
falsifiable shape), **PARTLY** (one side pinned, the other inferred), **UNKNOWN**
(not established).

## Quick path

1. Read "Verdict" for questions 1-6.
2. Read "The carrier and the producer switch" and "The frame table".
3. Read "The per-frame bit planes" and "The value stream" before implementing.
4. Read "Decoder" for the complete algorithm.
5. Read "Validation" for the live and file-resident numbers.
6. Read "The 59/7 split" and "The transient keyed frame".
7. Read "UNKNOWN" and the falsification ledger before extending coverage.

## Verdict

| # | Question | Answer | Class | Evidence |
|---|---|---|---|---|
| 1 | Where does the packed data live? | The channel record at `+0x38` points to a **standalone frame-table record**, not inline data. The table is a relocatable record whose file form uses 32-bit value offsets and whose memory form uses 64-bit pointers; the pointer width shifts the payload by `0x10`. | CONFIRMED | Runtime pointer read; file bytes and runtime bytes agree after the shift |
| 2 | Frame count, divisor, wrap | Header `+4` is a `u16` frame count, `+6` is a `s16` divisor. Frame index is `floor(frame_time / divisor)`; `frame_time` is channel `+4` (current) and `+6` (previous). The observed table is 50 frames at divisor 100. The `index+1 >= count` wrap branch was reached in code but not exercised by the validated frame. | CONFIRMED for count/divisor/index; PARTLY for the wrap | Live capture `time=2400`, index `24/50`; code path |
| 3 | Per-frame payload layout | A flag byte selects which of three planes are present: bit 0 angle, bit 1 scale, bit 2 translation. Present planes are concatenated at `align8(3 * slot_count)` bits (200 bits for 66 slots). Each plane stores **3 bits per slot**; the 3 bits are a component mask (bit 0 X, bit 1 Y, bit 2 Z). The 12-bit mask maps bit 0-2 to angles, 4-6 to scale, 8-10 to translation. | CONFIRMED for the angle plane and the mask; PARTLY for the scale plane | 66-slot live capture; file replay |
| 4 | Value encoding | The value stream is a sequence of blocks: `i32 0` + `frame_count` `f32` palette, or `i32 1` + one `f32` literal. One block is consumed per set bit, in ascending lane order. The floats are signed and already in **binary-angle units** (4096 per turn); the palette index is the frame index. | CONFIRMED | Decoded values equal the staged values exactly |
| 5 | Which slots are the 7 mode slots? | The live track chain targets **51, 52, 53, 58, 59, 60, 61** with modes `8` (six entries) and `0x4C` (one entry, slot 58). They are per-slot transform/mode overrides, not aim/root slots. The aim slots (0, 1, 31, 41) are a separate overwrite class inside the 59 channel-covered slots. | CONFIRMED (runtime track chain) | Track pool chain, 7 entries |
| 6 | What triggers the transient keyed frame? | The switch is **not** the keyed-table pointer alone: every channel carries a keyed head (frame count 300) whose frame-array pointer at `+0x10` is normally null. The transient is that pointer becoming non-null for one channel, which runs the keyed sampler for that channel's slots. The exact state/animation that sets it is UNKNOWN. | CONFIRMED for the mechanism; UNKNOWN for the trigger | Live keyed heads; transient snapshot `keyed_tables=1`, `flags=0:50,1:16` |

## The carrier and the producer switch

The channel record keeps its documented `0x50` stride. The fields needed here
are:

| Field | Offset | Role |
|---|---:|---|
| format byte | `+0` | channel format flags |
| value-stream selector | `+1` | index into the table's value-offset array |
| first slot | `+2` | first slot covered by this channel |
| slot count | `+3` | number of contiguous slots covered |
| current frame time | `+4` | `u16`, divided by the table divisor |
| previous frame time | `+6` | `u16` |
| packed table | `+0x38` | pointer to the packed frame table |
| keyed table | `+0x48` | pointer to the keyed head |

The engine chooses the producer per channel:

```text
keyed  if channel.keyed_head != 0 AND channel.keyed_head.frame_array != 0
packed otherwise
```

The keyed head is not the switch on its own. In the observed live state all three
channels had a non-null keyed head, but every head's `+0x10` frame-array pointer
was null, so all three ran the packed decoder. This corrects the earlier
shorthand "keyed table non-null => keyed producer".

## The frame table

The table is a standalone record. Its two forms differ only by pointer width:

| Field | File form | Memory form |
|---|---:|---:|
| format/count byte | `+0x00` | `+0x00` |
| per-channel bytes | `+0x01..+0x03` | `+0x01..+0x03` |
| frame count (`u16`) | `+0x04` | `+0x04` |
| divisor (`s16`) | `+0x06` | `+0x06` |
| four header words | `+0x08..+0x0F` | `+0x08..+0x0F` |
| widened link field (file offset vs memory pointer) | `+0x10..+0x13` (4 bytes) | `+0x10..+0x17` (8 bytes) |
| value-block offsets/pointers | `+0x14 + i*4` | `+0x18 + i*8` |
| bit-plane block | `+0x14 + 4*count` | `+0x18 + 8*count` |

`count` is the byte at `+0x00` (3 in the observed table). The file form stores
three 32-bit offsets; the memory form stores three 64-bit pointers. The field
at `+0x10` is widened the same way: one 32-bit offset in the file, one 64-bit
pointer in memory. The `0x10` block shift is therefore the sum of two
changes, `+4` for that field and `+4*count` for the offset array. In the file
the block starts at `+0x20`, in memory at `+0x30`.

The observed header is frame count 50, divisor 100, and four words
`50, 100, 50, 100`. The frame clock is:

```text
frame_index = floor(channel.frame_time / table.divisor)
```

The channel's current and previous times are both advanced by the engine. In the
validated frame the clock was `2400 / 100 = 24` of 50. The decoder's wrap branch
fires when `frame_index + 1 >= frame_count`; the code then wraps the next-frame
lookup to frame 0 or suppresses interpolation. The validated frame did not reach
that branch, so the exact wrap choice stays PARTLY.

## The per-frame bit planes

The bit-plane block begins with a flags byte:

| Bit | Plane | Output lanes |
|---:|---|---|
| 0 | angle | 0, 1, 2 |
| 1 | scale | 4, 5, 6 |
| 2 | translation | 8, 9, 10 |

Only the present planes are stored, concatenated in the order angle, scale,
translation. The stride of one plane is

```text
plane_stride_bits = align_up(3 * slot_count, 8)
```

which is 200 bits (25 bytes) for the 66-slot raziel body. Within a plane a slot
owns three consecutive bits at

```text
plane_base_bit = 8 + 3 * slot + plane_index * plane_stride_bits
```

The bit offset is measured from the start of the bit-plane block, so the first
`8` bits skip the flags byte. The three bits are a component mask, not an index:

- bit 0 = X (the first angle, the first scale component, or translation X)
- bit 1 = Y
- bit 2 = Z

The engine forms a 12-bit mask from the present planes:

```text
mask = angle3 << 0 | scale3 << 4 | translation3 << 8
```

and walks the mask from lane 0 upward. Lane `k` is written only when bit `k` is
set. The observed table used `flags = 0x05` (angle and translation, no scale), so
the mask had angle bits 0-2 and translation bits 8-10.

The angle bit order is X, Y, Z, which is the `(a, b, c)` order the rotation model
consumes. The calibration triple `(-595.240, -101.331, 52.838)` is exactly the
three angle lanes of one slot.

## The value stream

Each channel has a value stream selected by channel `+1` through the table's
value-offset array. The stream is a linear sequence of blocks:

```text
command 0:  i32 0, then frame_count f32 palette values
            index = frame_index; size = 4 + 4 * frame_count bytes

command 1:  i32 1, then one f32 literal
            size = 8 bytes
```

One block is consumed for each **set bit** of the 12-bit mask, in ascending lane
order. The palette's frame index is the same `floor(frame_time / divisor)` used
for the table. A slot with mask `0x007` consumes three blocks and writes angle
X, Y, Z in that order; a slot with mask `0x001` consumes one block and leaves
Y and Z at zero.

The values are signed `f32` in the binary-angle unit the rotation model already
uses: **4096 units per turn**, continuous, not scaled again. The validation
showed values such as `203.024`, `-595.240` and `52.838` written unchanged into
the angle lanes.

When the channel fraction is non-zero, the engine blends the current value with
the next-frame value:

```text
out = current + fraction * (next - current) / 4096
```

with a half-turn wrap (`±2047/±4096`) on the delta. The validated frame had
`fraction = 0` (the game was paused on the sampled frame), so the blend was not
exercised; the formula is read from the engine, not measured.

## Decoder

```text
decode_channel(table, channel, slot_count, frame_count, divisor, stride):
    flags = table[block_base]
    present = [k for k in 0..2 if flags.bit(k)]
    cursor = table.value_offset[channel.value_selector]
    for slot in channel.first .. channel.first + channel.count - 1:
        mask = 0
        for plane_index, k in enumerate(present):
            group = read3(table, block_base*8 + 8 + 3*slot
                                 + plane_index*stride)
            mask |= group << (4*k)
        lanes = [0] * 12
        for lane in 0..11:
            if not mask.bit(lane):
                continue
            cmd = read_i32(table, cursor)
            if cmd == 0:
                lanes[lane] = read_f32(table, cursor + 4 + 4*frame_index)
                cursor += 4 + 4*frame_count
            elif cmd == 1:
                lanes[lane] = read_f32(table, cursor + 4)
                cursor += 8
            else:
                stop
        emit(staging[slot], lanes)
```

The engine pre-writes the scale (`4096.0`) and the descriptor translation before
the stream runs; a set scale or translation bit overrides those lanes. For the
angle-only decode, only lanes 0-2 and the angle plane are needed.

## Now implemented in

The decoder above is implemented in `src/packed_channel.rs` (pure, no I/O, not
wired into canonical frame state):

- `parse_header` parses and bounds-checks `table_count`, `frame_count` and
  `divisor`.
- `select_frame_index` applies an explicit `FrameIndexPolicy`. The default
  `FRAME_INDEX_POLICY` is `Saturate`; the UNKNOWN wrap branch is available as
  `Wrap` but is never implicit.
- `PackedFrameTable::parse` / `BitPlanes::parse` / `BitPlanes::slot_mask` parse
  the offset array, the flags byte and the per-slot 3-bit plane groups.
- `ValueStream::next_value` walks the palette/literal blocks in ascending lane
  order.
- `decode_slot_angles` returns one slot's angle triple;
  `DecodedChannel` / `decode_channel_range` / `decode_channel` return the angles
  and their `rotation_from_binary_angles` matrix for a whole channel window.

## Now consumed by

The decoder is wired into the shipped `.SRA`:

- `--animate --sra <FILE> --bigfile <DAT> --entry <INDEX> [--clip <N>]
  [--frame <N>] [--time <T>] [--list-clips]` locates every structurally valid
  table, decodes one frame of the selected clip and prints slot count, channel
  count, clip index, frame index, the first populated angle triples and the first
  three skinned vertices. `--list-clips` prints the enumeration instead. Slot
  count is the model's bone count, not an `.SRA` field.
- `--play ... --sra <FILE>` decodes one frame per canonical tick and skins the
  player mesh through `pose::skin_positions_hierarchical`. It is presentation
  only: no animation state enters `FrameState` and no golden digest moves.

Two file-form facts are CONFIRMED by that replay: the inline stream begins at
`align4(bit_plane_base + 1 + present * stride_bytes)` (not at the `+0x14`
offsets, which still point elsewhere), and one contiguous stream carries every
slot in slot order. At frame 24, slot 6 decodes to exactly
`(-595.240, -101.331, 52.838)`.

### Playback clock

The capture above fixes the **source-side** frame unit (`divisor` and
`frame_index = floor(frame_time / divisor)`), but it does **not** establish how
many source time units the game advances per second. The authored wall-clock
rate is therefore **UNKNOWN**. The renderer models the presentation clock as a
constant rate in animation time units per second and lets each clip's `divisor`
convert that rate into a palette frame index in `src/sra.rs`:

```text
units_elapsed = tick * units_per_second / 60
frame_index   = units_elapsed / divisor
index         = frame_index % frame_count           (PlaybackLoop::Loop)
index         = min(frame_index, frame_count - 1)   (PlaybackLoop::Once)
```

The divisor is therefore load-bearing. At the same clock rate, a clip with
divisor `544` advances `100 / 544` as fast as the validated divisor-100
reference. The measured `RAZIEL.SRA` clips include divisors `100`, `177`, `209`,
`233` and `544`, so they keep their relative authored speeds instead of all
playing at one rate.

The arithmetic is integer-only (`u64`, no float, no wall clock) and narrows only
at the end, so a large tick count cannot hit the old `u16` frame-time ceiling
that froze playback. `ANIMATION_UNITS_PER_SECOND` defaults to **5000**. That
value is derived from the validated reference clip: it has divisor `100` and 50
frames, and the earlier presentation choice played it at 50 palette frames per
second, so `50 frames/s * 100 units/frame = 5000 units/s`. One full 50-frame
pass therefore still lasts one second of simulation, and one reference frame
lasts about 1.2 ticks at the fixed 60 Hz rate. This is a presentation default,
not a measured engine constant.

`--anim-fps <N>` (accepted range `1..=240`) keeps its shipped name but is
defined as frames per second for a divisor-100 clip; it converts internally to
`N * 100` time units per second. A clip with another divisor plays at
`N * 100 / divisor` frames per second. Add `--anim-once` for a single pass that
holds the last frame instead of wrapping. In `--animate`, an explicit
`--frame <N>` overrides the playback clock. The clock is presentation-only: it
never enters `FrameState` and no determinism golden moves.

## Clip enumeration and state selection

A `.SRA` payload is a record container. The **base** enumeration keeps the first
structurally valid packed frame table in each payload record, in file order, so
indices `0..base` are stable. A record may in principle hold more than one
table, so the loader continues past the base table's inline value stream and
appends every further structurally valid table after the base indices (record
order, then order within the record). Additional tables are deduplicated by the
form-invariant content fingerprint the labelling probe uses: frame count,
divisor, and FNV-1a 32-bit over `+0x00..+0x10` plus the complete bit-plane
block. That view is literal content, so a relocated memory copy agrees with the
file form; the definition and its evidence are in "Clip fingerprint" below.

The `+0x04`/`+0x08` duplicate header word `(divisor << 16) | frame_count` parses
as a one-stream table with the same frame count and divisor. A candidate nested
in the base table's 20-byte header is therefore rejected and reported, never
indexed. A candidate past a table's extent that parses as a header but fails the
whole-slot inline decode is counted as structurally rejected.

Measured read-only at the player's 66-slot stride:

| File | Payload records | Base clips | Additional distinct | Duplicates | Header aliases rejected |
|---|---:|---:|---:|---:|---:|
| `RAZIEL.SRA` | 230 | **230** | **0** | 0 | 380 |

The same scan over all 227 `OBJECT/*.SRA` found **0 genuine additional tables**:
every extra structural match at a non-base offset was a `+0x04`/`+0x08` header
alias, not a second table. A record therefore holds one table in the measured
corpus, and the original one-table-per-record enumeration was already complete.

`RAZIEL.SRA` clip 0 is the validated 50-frame body table: divisor 100, flags
`0x05` (angle + translation), 3 streams. No table carries a name, id, state
byte or clip index:

- `+0x00` table/stream count, `+0x04` frame count, `+0x06` divisor.
- `+0x08` and `+0x0C` are the same packed word, `(divisor << 16) | frame_count`.
- `+0x14` starts the value-offset array; there is no id word before it.
- The three bytes at `+0x01..+0x03` are equal within a table (`100` in most,
  `150` in a few) and do not track record order, so they are not an index.
- The runtime samples a table through the channel pointer at `+0x38`; it never
  reads a table id. Which channel set is active is an engine state question,
  not a field in the file.

## Clip fingerprint: form-invariant content digest (2026-11-14)

A runtime observation names the packed table the engine selected; the file
enumeration names the same table in a payload record. Mapping one to the other
needs a key that survives relocation, so the fingerprint is defined only over
bytes that are identical in the file and memory forms.

### Definition

```text
invariant_fp = FNV-1a 32-bit over, in this order:
  1. table bytes +0x00..+0x10   (16 bytes)
  2. the complete bit-plane block: flags byte then the present planes,
     each align8(3 * slot_count) bits

key = "<frame_count>:<divisor>:<invariant_fp as 8 lowercase hex digits>"
```

`slot_count` is the **observed body's** covered slot count, not a global player
value; see "Small bodies" below for the byte-level difference and the proof.

Included, and why each is form-invariant:

- `+0x00` table/stream count, `+0x01..+0x03` the three per-channel bytes,
  `+0x04` `u16` frame count, `+0x06` `s16` divisor, `+0x08..+0x0F` the two
  duplicated `(divisor << 16) | frame_count` words. These are literal content;
  relocation copies them unchanged.
- the flags byte and the per-slot 3-bit component masks. These are static
  animation content, also copied unchanged.

Excluded, with the reason:

- `+0x10..+0x13`. The field is 4 bytes wide in the file and 8 bytes wide in
  memory: a file offset versus an absolute pointer. Hashing its low half makes
  the key form-dependent and session-dependent (ASLR). This is the defect that
  produced `unmapped` walk keys.
- the value-offset/pointer array (`+0x14 + i*4` vs `+0x18 + i*8`).
- the value streams. The file's `+0x14` offsets do not point at the inline
  palette the replay decodes (UNKNOWN 3), so a stream location shared by both
  forms is not established. The header and the masks already identify the
  table.

The pre-fix digest, which hashed `+0x00..+0x14` instead of `+0x00..+0x10`, is
still emitted as `legacy=`. It is a diagnostic only and must never be used for
mapping.

### Evidence

A captured memory-form table and the shipped file table were compared for two
clips. In both clips `+0x00..+0x10` and the whole bit-plane block are
byte-identical; only the widened field and the pointer array differ.

| Clip | Pair | File legacy | Runtime legacy | Invariant (both forms) |
|---|---:|---:|---:|---:|
| 0 (idle) | `50:100` | `b1db0234` | `b1db0234` | `a1f48a74` |
| 2 | `12:209` | `b71d0a05` | `59559fbd` | `3acaf951` |

- Clip 0's widened field is null in both forms, so the legacy digest coincides
  by accident. That is the observed idle key, which is why idle mapped before
  the fix.
- Clip 2's widened field is a non-null file offset and a relocated pointer. Its
  legacy digest therefore has three different values: file form, captured
  session and observed session. Its invariant digest is one value.
- The observed walk key `12:209:59559fbd` is a legacy key. It cannot be
  reproduced offline: its 4 bytes at `+0x10..+0x13` are a pointer that exists
  only in that process. The pair `12:209` plus the byte-identical header and
  block identify clip 2; under the invariant digest the walk observation emits
  `12:209:3acaf951`. This is **PARTLY** (fingerprint-implied), pending one
  runtime run with the fixed probe.

Uniqueness over the 230 `RAZIEL.SRA` clips at 66 slots: the invariant view gives
**225 distinct keys**. Five pairs collide; the previous 20-byte view had one
collision, and the four extra collisions are the price of dropping the
form-dependent word. The mapping policy is unchanged: the index keeps the
lowest clip and reports `ambiguous`.

### Small bodies: the stride belongs to the body, not to the player (2026-11-15)

The definition above is form-invariant, but it was under-specified about **whose**
slot count supplies `align8(3 * slot_count)`. The probe answered that question
with the global `--slots` value (the player's 66) for every observed body. That
is correct for `RAZIEL.SRA` and wrong for every smaller body, which is why a
real directory run matched `slots=66` and returned `file=none` for `slots=24`
and `slots=3`.

**The byte-level difference.** Take `SLUAGH.SRA` record 0 and `RAZIEL.SRA`
record 0. Both tables use `flags=0x05` (angle + translation), so both hash a
flags byte plus two planes. Everything else differs:

| | `RAZIEL.SRA` record 0 | `SLUAGH.SRA` record 0 |
|---|---|---|
| body slots (observed) | 66 | 24 |
| table/stream count (`+0x00`) | 3 | 2 |
| plane stride `ceil(3*slots/8)` | 25 bytes | 9 bytes |
| planes present (`flags=0x05`) | 2 | 2 |
| bit-plane block length | `1 + 2*25 = 51` | `1 + 2*9 = 19` |
| file block base | `+0x14 + 3*4 = +0x20` | `+0x14 + 2*4 = +0x1c` |
| memory block base | `+0x18 + 3*8 = +0x30` | `+0x18 + 2*8 = +0x28` |
| file-to-memory block shift | `+0x10` | `+0x0C` |

Two independent consequences follow, and only the first was already handled:

1. **The block shift is `4 + 4 * table_count`, not a constant `0x10`.** A
   two-stream table shifts by `0x0C`, a three-stream table by `0x10`. The file
   side (`+0x14 + 4*count`) and the memory side (`+0x18 + 8*count`) both already
   used the count, so the base was never the defect.
2. **The hashed window is `1 + present * ceil(3 * slots / 8)`, and `slots` is the
   observed body's.** Hashing a 24-slot table with the player's 25-byte planes
   reads `51 - 19 = 32` bytes past the block. Those bytes are the `align4`
   padding and the inline value stream (the command word and its palette), so
   the digest is stable per form but describes the wrong window. Measured for
   `SLUAGH.SRA` record 0: the file form and a relocated memory form agree at
   `13:333:f60344d6` when both use 9-byte planes, and the memory form hashes to
   `13:333:4a378f68` at 25-byte planes. One clip, two keys, and the file index
   only holds the first.

**The corrected invariant.** The key is a function of the table's own content
and of the body's slot count:

```text
invariant_fp(slots) = FNV-1a 32-bit over, in this order:
  1. table bytes +0x00..+0x10   (16 bytes, includes the stream count)
  2. the bit-plane block at +0x14 + 4*count (file)
     or +0x18 + 8*count (memory): flags byte then `present` planes of
     ceil(3 * slots / 8) bytes each

key = "<frame_count>:<divisor>:<invariant_fp as 8 lowercase hex digits>"
slots = the observed body's covered slot count, i.e. the sum of its channel
        slot counts (66 for the player body, 24 for `SLUAGH.SRA`)
```

Why that is invariant across forms for any slot count and any stream count:

- the 16-byte header and the flags byte are literal content, copied unchanged by
  relocation;
- the stream count is inside those 16 bytes, and it only moves the block, which
  both sides derive from it;
- the plane stride is a property of the model, so the file's inferred stride and
  the live body's `ceil(3*slots/8)` are the same number for the same body. The
  file side infers it from the records; the runtime derives it from the channel
  coverage. The existing attribution rule already compares exactly this stride,
  so the fingerprint and the `slot-mismatch` check now agree by construction;
- the key text does not change for the player body: its stride was already 25,
  so `a1f48a74` (clip 0) and `3acaf951` (clip 2) are unchanged.

The fingerprint is therefore not a single global constant: it is
`fingerprint(table content, slots)`. Two bodies with different slot counts give
the same table two different keys, which is intended — the key names a clip of a
specific body, and the directory index already stores `(file, per-file index)`.

**Proof.** Both forms are constructed for the same small clip and hashed:

| Case | Form | slots | streams | key |
|---|---|---:|---:|---|
| synthetic 24-slot clip | file | 24 | 2 | `13:333:31e138ac` |
| synthetic 24-slot clip | memory | 24 | 2 | `13:333:31e138ac` |
| same memory bytes | memory | 66 | 2 | differs |
| `SLUAGH.SRA` record 0 | file | 24 | 2 | `13:333:f60344d6` |
| `SLUAGH.SRA` record 0 | memory | 24 | 2 | `13:333:f60344d6` |
| `RAZIEL.SRA` record 0 | file | 66 | 3 | `50:100:a1f48a74` |
| `RAZIEL.SRA` record 0 | memory | 66 | 3 | `50:100:a1f48a74` |

The synthetic pair is the regression test: `--selftest` case
`fingerprint-small-body-form-invariant` builds a 24-slot, 2-stream record,
relocates it the way the engine does, and asserts one key for both forms and a
different key at the player's stride. `--live-selftest` case
`fingerprint-small-body-cross-form` does the same across the boundary: the host
builds the file form and the real agent builds the memory form through the same
hook the capture uses, then compares the two keys.

**Re-measured index (227 files).** The file side is unchanged, so the directory
totals are identical; what changed is how many files can be reached by a live
body at all:

```text
CLIPMAP files=227 clips=1001 distinct=910 ambiguous=55 collisions=32 elapsed=2.57s
files_with_clips=143 files_without_a_table=84
files_with_a_matching_key:        before=34  after=143
files_usable_after_attribution:   before=30  after=143
```

All 143 files that index at least one clip now resolve at least one
actor-reachable fingerprint; the other 84 files hold no structurally valid
table. Before the fix, 30 files could be used: the 30 files at the player's
66-slot stride. The raw count was 34, because four smaller files (strides 13, 56,
57 and 63) hashed at 25-byte planes into keys that exist in the map — but every
one of those keys belongs to a 66-slot file, so the attribution rule refuses the
small actor as `slot-mismatch`. They were matches, not attributions.

The stride distribution shows why the defect was not a corner case: 30 files are
66-slot bodies, 3 are 24-slot, and the remaining 110 are other sizes (strides
3..64), so most of the corpus was unreachable from a live body of its own size.

The player's confirmed keys are unchanged: `RAZIEL[0]=50:100:a1f48a74`,
`RAZIEL[2]=12:209:3acaf951`, `RAZIEL[35]=2:700:c003c0fc`,
`RAZIEL[36]=1:100:d4224b95`, `RAZIEL[37]=2:400:876bcf9b`,
`RAZIEL[55]=6:580:e5bb630f`.

**Limit.** `slots` is the sum of the observed channel slot counts, so the
invariant holds while the channels tile the model's slot space. A body whose
channels cover a strict subset would hash a shorter window than its file's
stride; that body is reported `file=none`, exactly as before, and is not
silently mapped to a neighbouring stride. No measured file has that shape.

The extended scan still finds **0 genuine additional tables** in the measured
corpus: every extra structural match at a non-base offset is a `+0x04`/`+0x08`
header alias. So the earlier conclusion that walk could not be in a second table
of `RAZIEL.SRA` stands; the reason walk was unmapped was the fingerprint, not a
missing table.

### The short jump clips (`2:700`, `2:400`, `4:433`)

These are not misparsed keyed or nested structures. Each pair is a real located
clip:

| Pair | Clips |
|---|---:|
| `2:700` | 35 |
| `2:400` | 37, 47 |
| `4:433` | 25, 32 |

The captured runtime arena contains memory-form tables whose `+0x00..+0x10`
and bit-plane block are byte-identical to those file clips, so the engine
really selected packed tables with those shapes during the jump window. They
are short (2-4 frame), slow-divisor tables. The five-state aligned capture now
labels `35` (`2:700`) as jump-up and `37` (`2:400:876bcf9b`) as land; `25`,
`32` and `47` remain unlabelled. The pair alone cannot separate 37 from 47 or
25 from 32, but the invariant digest can: 37 and 47 share `2:400` yet hash
differently (`876bcf9b` vs `42223778`), so the runtime sample resolves to 37.

`--match-unmapped <FILE> --sra-dir <DIR> [--slots <N>]` still searches every
`.SRA` for a probe key and reports the file, record and index. It now compares
invariant keys; an old probe key that contains the pointer half is expected to
be `absent`.

### State defaults: runtime-labelled clips (2026-11-14, five-state map 2026-11-15)

The shipped tables still carry no state identity, so the default map is a
runtime observation of the table the engine selected, not a decoded field
(`docs/changes/runtime-capture/clip-labelling.md`).

| Index | State | Evidence class | How it was measured |
|---:|---|---|---|
| 55 | idle | **CONFIRMED** | Clip `55` (`6:580`) held coverage `1.00` in 3 of 4 runs of the still window. |
| 2 | walk | **CONFIRMED** | The moving window selected clip `2` (`12:209:3acaf951`) with coverage `0.91`; the fingerprint was predicted from this file before the run, then matched. |
| 35 | jump_up | **CONFIRMED** | Two runs of the aligned per-jump template made clip `35` (`2:700`) the first phase; template agreement `0.83 -> 1.00`. |
| 36 | fall | **CONFIRMED** | The middle phase of the same two-run sequence was clip `36` (`1:100`); agreement `0.83 -> 1.00`. |
| 37 | land | **CONFIRMED** | The last phase was clip `37` (`2:400`); agreement `0.83 -> 1.00`. |

**The jump is an ordered sequence, not one clip.** The aligned capture recorded
a jump as `35 -> (keyed) -> 36 -> (keyed) -> 37`, 24 samples over about
`0.76 s`; all six jumps produced this order. The `keyed` buckets are transitions,
not clips, so they never become an index. This is why the earlier single jump
window split across four clips (`27:0.16`, `16:0.15`, `2:0.12`, `37:0.07`,
keyed `0.21`): it averaged three phases plus their transitions.

Two caveats are part of the record. **Idle has a variant**: one earlier run
observed clip `0` (`50:100`) at coverage `1.00` instead of `55`, so idle likely
selects per section or variant; `55` is the most frequent and is the default.
**The map is a phase model, not five semantic names**: each label means "the
engine selected this table in this phase", the `.SRA` still has no state byte,
and a different cadence or section can select another table.

#### The landing window is derived from the `land` clip duration

`Landing` is the only state with a duration, so its window is not hand-picked.
It is one pass of the configured `land` clip at the presentation clock:

```text
seconds = frames * divisor / ANIMATION_UNITS_PER_SECOND
ticks   = floor(seconds * 60)            (the fixed 60 Hz frame clock)
```

The result is clamped to **4..=30 ticks** (`66.7 ms ..= 0.5 s`). The floor keeps
a one-frame, low-divisor clip visible instead of a zero- or one-tick flicker;
the ceiling stops a pathologically long `land` clip from holding the state for
seconds. The shipped `land` clip is `37` (`2:400`), so the default window is
`floor(2 * 400 * 60 / 5000) = 9` ticks (`0.15 s`). The derivation reads the
`land` clip actually in the resolved map, so a `--clip-land` override moves the
window with it. `ClipMap::for_clip_count` still clamps every index to the last
decoded clip, so a short animation falls back instead of erroring.

Selection reads only existing deterministic signals: `player_grounded()`,
whether stage 9 committed a non-zero horizontal delta (`player_moving()`), and
the signed `player_vertical_velocity()`:

| State | Rule |
|---|---|
| `Ascending` | not grounded and velocity `> 0` |
| `Falling` | not grounded and velocity `<= 0` (exactly zero counts as falling) |
| `Landing` | grounded inside the window that opens on the touchdown tick |
| `Moving` | grounded, outside the window, non-zero committed delta |
| `Idle` | grounded, outside the window, zero committed delta |

A touchdown is `grounded` now with a previous observation of `false`; a
first observation has no previous sample, so a player that spawns resting on
the floor does not play a landing. The window is `[touchdown, touchdown +
ticks)`: the touchdown tick is `Landing`, and the exclusive bound releases to
`Idle`/`Moving`. Because `Landing` holds for the whole window regardless of the
committed delta, a touchdown cannot oscillate `Landing -> Idle -> Landing`.

The switching rule is unchanged: evaluate once per rendered frame, switch only
on a state change, and restart the switched clip at local tick 0. A fixed
`--clip <N>` ignores the state and uses the global tick. Playback stays
presentation-only; no animation state enters `FrameState` and no determinism
golden moves.

CLI:

```text
--animate ... --list-clips            one line per located clip + scan stats,
                                       including fp=<invariant> legacy=<old>
--animate ... --clip <N>              decode clip N (default 0)
--animate ... --dump-unmapped <FILE>  write appended and rejected table keys
--match-unmapped <FILE> --sra-dir <DIR> [--slots <N>]
                                      search every .SRA for probe fingerprints
--play ... --sra <FILE> \
    [--clip <N>]                      freeze one clip
    [--clip-idle <N>] [--clip-walk <N>]
    [--clip-jump <N>]                 shortcut: sets jump-up, fall and land
    [--clip-jump-up <N>] [--clip-fall <N>] [--clip-land <N>]
```

`--clip-jump <N>` sets all three airborne states together as a testing
shortcut; an explicit per-state flag wins for that state. Every index is
validated strictly against the decoded clip count (`ClipError::OutOfRange`).

## Offline clip ranking (`--animate --rank-clips`)

The tables have no name, id or state byte, so a human mapping idle / walk / jump
has to try clips one by one. `--animate --sra <FILE> --rank-clips` decodes
**every** located clip and prints a bounded, parseable shortlist plus a
`suggest --clip-idle/--clip-walk/--clip-jump` line ready for `--play`.

The slot count is not an `.SRA` field (it is the model bone count), so ranking
takes it from `--slots <N>` or from `--bigfile <DAT> --entry <INDEX>`:

```text
--animate --sra <FILE> --rank-clips --slots <N> [--top <N>]
```

Default `--top` is 8, capped at 64. Output is bounded to
`10 + 4 * top` lines: one summary, one threshold line, four bucket headers, at
most `top` rows per bucket, one suggestion and three justification lines.

All per-axis differences are **shortest-arc** binary-angle differences wrapped
into `[-2048, +2048]` units. Stored angles are not normalized, so a raw
subtraction across the 0/4096 turn boundary would turn a small rotation into a
full-turn spike. Reported degrees still use `value * 360 / 4096`.

| Metric | Definition |
|---|---|
| `duration_ms` | `frames * divisor * 1000 / 5000` (integer math) |
| `mean_delta_deg` | mean absolute per-axis consecutive-frame change over all frames/slots |
| `max_delta_deg` | largest single consecutive-frame change |
| `loop_closure_deg` | mean `|frame[0] - frame[last]|` per axis; near 0 loops cleanly |
| `moving_slots` | slots whose largest consecutive-frame change exceeds `0.50deg` |
| `period` | `p` in `1..frames/2` minimising mean `|frame[k] - frame[k+p]|`, or `none`; residual reported in degrees |

Heuristic thresholds (also printed verbatim by the command):

| Rule | Threshold |
|---|---|
| `STILL` | `mean_delta_deg <= 0.50` |
| `IMPACT` | `max_delta_deg >= 5.00`, `max/mean >= 4.00`, no real periodicity |
| `LOCOMOTION` | `0.50 < mean_delta_deg <= 8.00`, real period `p >= 2`, residual `<= 1.00deg` |
| `OTHER` | everything else |

A period of `1` is the trivial adjacent-frame shift: for a smooth trajectory it
always wins the residual search and says nothing about repetition. It is
reported as `period=1`, but it is **not** periodicity evidence for `LOCOMOTION`
or `IMPACT`. Bucket rows are sorted by the bucket's own key (`STILL` ascending
motion, `LOCOMOTION` ascending residual, `IMPACT` descending spike ratio,
`OTHER` descending maximum change).

**UNKNOWN, not a failure**: this is a heuristic shortlist, never ground truth.
Measured read-only on the shipped player body, the thresholds put the still
clips in `STILL` and every remaining clip in `IMPACT`; no clip passed the real
periodicity rule, so `LOCOMOTION` is empty. The walk suggestion then falls back
to the lowest non-still motion and says so in its justification line. A bucket
can be empty, and an empty bucket is printed with `note=empty` instead of being
silently guessed. The trio is a starting point for a human, not a recovered
identity mapping.

## Validation

### Live capture

One read-only attach to the running game captured one 66-slot body node whose
three channels cover `0..13`, `14..49` and `50..65`. The captured state was:

| Field | Value |
|---|---:|
| frame time | 2400 |
| divisor | 100 |
| frame index | 24 |
| frame count | 50 |
| fraction | 0 |
| keyed frame array | null on all three channels |

The same call wrote the staging records for the 66 slots. Decoding the runtime
table and comparing every set lane to the staging record gave:

```text
PACKED DECODE-CAPTURE comparisons=82 exact=82 maxerr=0.000000 frame=24/50
```

That is 66 slots, 3 channels, 82 populated lanes, 67 of them angle lanes, at a
`1e-3` tolerance, with a measured maximum error of `0.0`.

### File-resident replay

The same table is present in the shipped `RAZIEL.SRA` animation resource. The
file form stores 32-bit value offsets where memory stores 64-bit pointers, which
shifts the bit-plane block and the value stream by `0x10`. The validated replay
reads the inline value stream at those shifted offsets; the file's own offset
array is a separate question (UNKNOWN 3). Reading the file table, decoding the
same frame 24 and comparing to the captured staging gave:

```text
PACKED DECODE-FILE comparisons=82 exact=82 maxerr=0.000000 frame=24/50
```

Angle lanes: **67 of 67 exact**. This is the required file-resident replay: the
same frame decoded from the shipped data matches the triple the live engine
wrote.

The scratch probe `sr1_packed_frame_probe.py` (outside the repository) implements
the decoder and the three validation paths. It passes:

```text
python sr1_packed_frame_probe.py --selftest       PASS cases=8
python sr1_packed_frame_probe.py --live-selftest  PASS cases=4
python sr1_packed_frame_probe.py --dry-run        prints the plan
```

### Extended enumeration

Read-only against the shipped player body at its 66-slot stride:

```text
sra clips=230 slots=66 base=230 additional=0 duplicates=0 distinct=229
    rejected_aliases=380 rejected_invalid=0 (RAZIEL.SRA)
clip index=55 record=55 offset=0 frames=6 divisor=580 channels=3 flags=0x05
```

The base indices `0..=229` are unchanged: clipping at index 55 is still the
same six-frame, divisor-580 table. The 380 rejected candidates are the
`+0x04`/`+0x08` duplicate header word in each record, not extra tables. A
directory-wide scan at the same stride found no genuine additional table in any
of the 227 `OBJECT/*.SRA` files.

### The older atomic capture

`angle_atomic.log` was recorded in a different process session and a different
animation phase. Its closest `ANGLE PAIR3` triple to the captured body pose is `(-783.884, 21.435, 39.071)` against
`(-785.704, 20.866, 39.640)`: a maximum component difference of `1.82`, with no
triple within `0.05`. The capture and the file table therefore **cannot be
phase-aligned**: there is no shared clock or animation id in the old log. The
needed alignment capture is a same-session one that records, per packed
invocation, the channel pointer, the channel frame time/divisor, the table bytes
and the produced staging triple. That is exactly what `--capture` in the scratch
probe records, and it is what the live validation above used.

## The 59/7 split

The live track chain is the direct answer to the 7 mode slots. It walked:
`51 -> 52 -> 53 -> 58 -> 59 -> 60 -> 61`, modes `8` (six entries) and `0x4C`
(one entry, slot 58), flags `0x4015` and `0x0015`. These are per-slot transform
overrides applied after the channel pass. They are not the object root and not
the authored aim joints.

The census's aim class is separate: slots 0, 1, 31 and 41 carry aim offsets and
are overwritten by the aim path. They lie inside the channel coverage, so the
split's `packed=59` counts them as channel-covered, but their final angle source
is the aim path, not the packed stream. The precise per-slot picture is:

```text
66 slots = 7 mode-applied track targets
         + 4 aim/root overwrites (0, 1, 31, 41)
         + 55 packed-channel angle triples
```

The split's `packed=59` is the channel-covered count, not the count of slots
whose final triple came from the packed stream.

## The transient keyed frame

Every channel in the captured node carried a keyed head (`+0x48` non-null) with
frame count 300 and a null frame-array pointer at `+0x10`. The live switch is:

```text
keyed  if keyed_head != 0 AND keyed_head.frame_array != 0
packed otherwise
```

The transient snapshot in the stored run showed `keyed_tables=1` and
`flags=0:50,1:16`: one channel's frame array became non-null, the keyed sampler
ran for that channel, and 16 slots received the quaternion kind word. That is
consistent with the 16-slot channel (slots 50-65). The snapshot then reverted.

What is established: the head alone is permanent and is **not** the producer
change; the frame-array pointer is. What is not established: the state or
animation that sets that pointer for one channel. It is not a permanent
producer change, it is not all three channels, and it is not the aim/root path.
A capture that logs `keyed_head.frame_array` with the active animation id at the
transition is the missing observation.

## UNKNOWN

1. **Wrap semantics** when `frame_index + 1 >= frame_count`: clamp, wrap to 0, or
   suppress interpolation. The branch was not exercised by the validated frame.
2. **The table header words** at `+0x08..+0x0F` (`50, 100, 50, 100` in the
   observed table). The validated decoder reads only `+0x04` and `+0x06`.
3. **The file-form value-offset array** at `+0x14..+0x1F`: the offsets in the
   observed file point to float arrays that are not the inline palette the
   validated replay used. The inline stream is confirmed; the selection rule for
   the offset array is not.
4. **The scale plane** (flags bit 1) was absent in the observed table. Its
   component mapping is inferred from the lane layout, not measured.
5. **The transient keyed trigger**: which state or animation sets the frame-array
   pointer for one channel.
6. **Coverage**: one shipped build, one actor, one animation state, one
   non-wrapping frame. Other actors, clips and wrap frames are unobserved.
7. **The widened +0x10 field meaning.** It is an offset in the file and a
   pointer in memory, but what it points at, and whether it is null for every
   unloaded clip, is not established. Its non-null-ness is why the legacy
   fingerprint could never be form-invariant.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| The channel `+0x38` points to a standalone table, not inline data | a channel whose packed data is reachable as `channel + constant` without a pointer | Holds: the pointer was read and the file/runtime tables agree |
| The producer switch is the keyed frame-array pointer | a channel with a non-null frame array that still runs the packed decoder, or a null array that runs the keyed decoder | Holds: all three channels had null arrays and ran packed |
| The angle plane is 3 bits per slot, X/Y/Z in bits 0/1/2 | a decoded angle triple that does not match the staging record at those bits | Holds: 67/67 angle lanes exact |
| Present planes are concatenated at `align8(3*slot_count)` | a table where the translation plane is at the same offset regardless of the angle plane | Holds: the translation bits of slot 1 land on the translation lanes at the second plane offset |
| Values are binary-angle `f32` with no extra scale | a palette value that needs a scale to match the staged angle | Holds: decoded values equal staged values with no scale |
| The 7 mode slots are 51, 52, 53, 58, 59, 60, 61 | a live track chain with a different target set | Holds for the captured state |
| The transient is the frame-array pointer | a transient where the frame array stays null but the kind word changes | Not observed; the frame array was null in the normal state |
| The old atomic capture can be aligned to the file table | a same-session clock/id in the old log | Refuted: different session and phase; closest triple differs by 1.82 |

## Method and legal

- Read-only instrumentation of one running process: a bounded hook on the
  packed core read one invocation, its channel array, its table bytes and the
  staging records that same call wrote. No game memory was written and no game
  code was called.
- Offline decode of the shipped `RAZIEL.SRA` table, read-only, using the
  documented field offsets. The scratch probe and the capture raw bytes lived
  only in the session temp directory.
- The scratch probe carries module-relative offsets and therefore stays outside
  the repository. It passes `--selftest`, `--live-selftest` and `--dry-run`.
- This note records field offsets, table-relative layouts, format constants,
  value sets, floats and counts only. It contains no addresses, no
  module-relative numbers, no disassembly listings, no raw game bytes and no
  absolute install paths.
