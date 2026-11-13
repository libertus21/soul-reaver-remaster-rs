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
| reserved | `+0x10..+0x13` | `+0x10..+0x17` |
| value-block offsets/pointers | `+0x14 + i*4` | `+0x18 + i*8` |
| bit-plane block | `+0x14 + 4*count` | `+0x18 + 8*count` |

`count` is the byte at `+0x00` (3 in the observed table). The file form stores
three 32-bit offsets; the memory form stores three 64-bit pointers. That is the
whole `0x10` shift: the file block starts at `+0x20`, the memory block at `+0x30`.

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

- `--animate --sra <FILE> --bigfile <DAT> --entry <INDEX> [--frame <N>]
  [--time <T>]` locates the first structurally valid table in a payload record,
  decodes one frame and prints slot count, channel count, frame index, the first
  populated angle triples and the first three skinned vertices. Slot count is
  the model's bone count, not an `.SRA` field.
- `--play ... --sra <FILE>` decodes one frame per canonical tick and skins the
  player mesh through `pose::skin_positions_hierarchical`. It is presentation
  only: no animation state enters `FrameState` and no golden digest moves.

Two file-form facts are CONFIRMED by that replay: the inline stream begins at
`align4(bit_plane_base + 1 + present * stride_bytes)` (not at the `+0x14`
offsets, which still point elsewhere), and one contiguous stream carries every
slot in slot order. At frame 24, slot 6 decodes to exactly
`(-595.240, -101.331, 52.838)`.

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
