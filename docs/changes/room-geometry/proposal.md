# Proposal: room-geometry

## Intent

The previous slice proved the `.PCM` offset table, the name table, and the
cross-links, then recorded geometry as NO-GO. This change takes the first real
look inside a unit: what are the blocks the offset table points at, and is a
visible geometry win reachable without guessing? A documented no-go is an
acceptable outcome.

## Scope

### In Scope

- Classify the per-unit blocks by size/layout clique; prove there are no tags.
- Decide the "records 0/1 are a directory" hypothesis one way or the other.
- Name the strongest geometry candidate with measured evidence (stride, range,
  count), without decoding a mesh.
- Ship `--level-records <path>`: a per-unit block census.

### Out of Scope

- Vertex/face decode, mesh assembly, room rendering.
- Object placement, transforms, collision.
- `.PCM` <-> PS1 `.drm` equivalence.
- `.PMF`/`.PNF` (audio sidecars, already closed).

## Capabilities

### New Capabilities

- `level-block-census`: a read-only, per-unit count of blocks by shape class,
  plus the largest block's ordinal and size.

### Modified Capabilities

- None.

## Approach

Probe first, then ship the cheapest verifiable artifact. The offset table is
the only directory: no block carries a type tag. Blocks are therefore grouped by
size clique, and each candidate is tested for a 16-bit stride and range. The
shipped command prints counts only; it makes no vertex, face, placement, or
render claim.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/level.rs` | Modified | block sizes on `LevelUnit`; shape classifier; census lines |
| `src/main.rs` | Modified | `--level-records` arm and usage text |
| `docs/notes/room-geometry.md` | New | findings note |
| `README.md` | Modified | status pointer (implementation phase) |
| `src/render.rs`, `src/srm.rs`, `src/mesh_entry.rs` | None | untouched by contract |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Shape labels read as semantic types | Medium | name classes by size only; document the limit |
| Overclaiming geometry | Medium | no vertex/face counts in output or docs |
| Probe overreach | Low | timeboxed; residual facts go to UNKNOWN |
| Regression in `--level-info` | Low | additive arm; output regression test |

## Rollback Plan

Revert the `src/level.rs` and `src/main.rs` hunks; the command disappears and
`--level-info` is untouched. The note and change docs are additive and can stay.

## Dependencies

- The shipped `.PCM` header/offset model in `src/level.rs`.
- Read-only SR1 data for manual acceptance.

## Success Criteria

- [ ] The census runs on a small, a medium, and a large unit.
- [ ] Every unit's classes sum to its block count.
- [ ] No geometry, placement, or render claim appears in output or docs.
- [ ] `--level-info` output is unchanged.
