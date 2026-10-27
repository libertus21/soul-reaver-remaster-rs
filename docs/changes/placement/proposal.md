# Proposal: placement

## Intent

The `room-geometry` census closed geometry as NO-GO and recorded one later,
single-unit observation: 40/48-byte blocks looked like 12-byte
`[u32 id][f32 value][u32 flags]` groups. That observation is the only lead
toward object placement or transforms. This change tests it across the whole
corpus and plans the smallest verifiable step. **A documented no-go is an
acceptable outcome**, and it is the outcome the probe supports.

## Verdict up front

- The 12-byte `[id][f32][flags]` reading is **refuted as a global `.PCM`
  layout**. It is unit-local at best: in the strongest unit the full triple
  co-occurs in 4.9% of groups, and the highest-marker unit has 0.0%.
- The universal structure is a **4-byte word grid**: every non-zero block is a
  multiple of 4 bytes, and 40 B / 48 B are simply 10-word and 12-word arrays.
- The `0x34` byte is a **rare marker** (0.39% of words), not a constant tag.
- Object placement and transforms: **NO-GO** on this evidence.

## Scope

### In Scope

- Settle the stride objectively across all 89 units (4/8/12/16/20/24).
- Test the f32 role as global vs unit-local.
- Test the `0x34` marker as global vs unit-local, and its low-byte structure.
- Cross-check block ordinals against the 2,106 extracted names.
- Ship a counts-only **stride/group census** that makes the refutation
  reproducible per unit.

### Out of Scope

- Coordinate or transform decode; placement instantiation.
- Mesh, faces, indices, normals, render.
- Semantic type labels for any block or word.
- A name-to-block ordinal mapping.
- `.PCM` <-> PS1 `.drm` equivalence.

## Capabilities

### New Capabilities

- `level-group-census`: a read-only, per-unit count of the 4-byte word grid, the
  12-byte partition (whole groups plus tail words), the `0x34` marker by word
  phase, and a bounded f32-plausibility count by phase. It makes no semantic
  claim.

### Modified Capabilities

- None.

## Approach

Probe first, then ship the cheapest artifact that makes the probe repeatable.
The probe shows the 12-byte group is an arithmetic partition of the word grid,
not a proven record. The shipped command therefore prints counts only:
word totals, whole/partial 12-byte partitions, marker incidence by word phase,
and a deterministic f32-range predicate by phase. It never prints payload
bytes, offsets, coordinates, or type names.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/level.rs` | Modified | word/group census types + formatter |
| `src/main.rs` | Modified | `--level-groups` arm and usage text |
| `docs/notes/placement.md` | New | findings note with probe ledger |
| `docs/notes/room-geometry.md` | Modified | correction pointer to the new note |
| `README.md` | Modified | status pointer |
| `docs/changes/placement/*` | New | this planning bundle |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| A counts-only diagnostic is read as a decode | Medium | name every field mechanical; state "diagnostic, not a record type" |
| The `0x34` marker is over-generalised again | High | report phase counts; state it is observed in 40/89 units and 0.39% of words |
| The f32 predicate is read as a type | Medium | define it as a bounded plausibility predicate; no semantic label |
| Repeating the single-unit error | Medium | the design pins one corpus-wide test per claim |
| Regression in `--level-records` | Low | additive arm; output regression test |

## Rollback Plan

Delete the `--level-groups` arm and the new `level.rs` helpers; `--level-info`
and `--level-records` are untouched. The note and change docs are additive and
can stay.

## Dependencies

- The shipped `.PCM` header/offset model in `src/level.rs`.
- Read-only SR1 data for manual acceptance (relative unit names only).

## Success Criteria

- [ ] The stride verdict is reproducible on all 89 units, not one block.
- [ ] The census prints counts only: no payload, offsets, coordinates, or tags.
- [ ] No placement, transform, or coordinate claim appears in output or docs.
- [ ] `--level-info` and `--level-records` output are unchanged.
