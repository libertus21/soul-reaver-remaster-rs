# bone-hierarchy - the descriptor `+18` word is a signed parent index, and the
# composed tree drives hierarchical skinning (2026-11-10)

Read-only format slice plus pure math. The archived object descriptor's `+18`
word is a **signed `i16` parent bone index** (`-1` = root), not an opaque slot
word. This note records the corpus agreement, the composition rule, the skinning
path and the runtime root-vs-leaf witness. Nothing was launched, attached or
instrumented; no game file was written and no game bytes, addresses, file
offsets or install paths appear here.

Evidence classes: **CONFIRMED** (observed shape with a falsifiable test),
**PARTLY** (structure observed, one side open), **UNKNOWN** (not established).

> Scope note: the "no hierarchy" verdicts in `docs/changes/animation/` are about
> **`.SRM`** (a flat 128-slot joint pool). Archived object models are a
> different format and **do** carry a parent tree in their descriptor array;
> this note covers only the archived objects. `.SRM` stays flat.

## Headline

1. **Parse.** `BoneHierarchy::from_descriptors` reads `+18` as `i16`. `0xFFFF`
   and `-1` are the same two bytes, so the retired unsigned wording and this
   signed one agree byte for byte.
2. **Validate.** Exactly one `-1` root, in-range parents and no cycle. A
   child stored before its parent is **reported** (`topological = false`), not
   rejected; composition resolves by index.
3. **Compose.** `world[child] = world[parent] * local[child]`, root first,
   bounded by the bone count, cycle-safe even without validation.
4. **Skin.** Hierarchical skinning applies the **composed** world matrix to each
   bone-local vertex; an all-identity pose reproduces the rest pose exactly.
   The non-hierarchical `--pose` path is unchanged; `--hierarchy` selects the
   composed path.

## A. Parse and validate

The record layout (24 bytes, little-endian) names only `+18` and `+20`; the rest
stay raw. On the live raziel dump the 66 parents are:

```text
[-1, 0, 1, 2, 1, 4, 5, 6, 7, 8, 5, 10, 11, 12, 1, 14, 15, 16, 17, 18, ...]
```

That is exactly the observed dump: one root (bone 0), `parent < own` for every
child, maximum depth 9 and no cycle. `--bone-hierarchy --bigfile <DAT> --entry 66`
prints `roots=1 root=0 depth=9 topological=true valid`.

Refusals are typed: `Empty`, `NoRoot`, `MultipleRoots`, `ParentOutOfRange`,
`Cycle`. A self-parent is a one-bone cycle and therefore has no root under the
strict `-1` rule; it is refused, not quietly reinterpreted.

## B. Corpus agreement

`--bone-hierarchy --bigfile <DAT>` scans every skeletal object container (bone
count above zero) and folds the validation result:

| Bucket | Count |
|---|---:|
| Skeletal object containers | 394 |
| Descriptor array located | 394 |
| Valid (one `-1` root, acyclic) | **199 (50.5%)** |
| Invalid | 195 |
| `no_root` | 194 |
| `multiple_roots` | 1 |
| `parent_out_of_range` | 0 |
| `cycles` | 0 |

Two failure shapes account for every invalid container:

- **194 one-bone props.** `sphere`, `box`, `dbg_cam`, `glphicon`, `mcardx`,
  `particle`, ... store a single record with parent `0` (a self-parent) instead
  of `-1`. Under the signed rule that is a 1-cycle with no root. These objects
  have no skeleton to speak of; the strict rule still reports them.
- **1 multi-bone model.** `matsrevr` (7 bones) carries two `-1` roots. Whether
  the second root is real or a second encoding is **UNKNOWN**; the validator
  reports it, it does not repair it.

So the tree is universal on real body models but the `-1` root convention is
not universal across tiny props. The agreement rate is a property of the
corpus, not a parser failure.

## C. Composition rule

`local[bone]` is the bone's 13-float slot pose. For skinning the pose is applied
at the bone's own origin and prepended with that bone's rest offset:

```text
local[bone]   = pose[bone] * translate(bone.local)
world[child]  = world[parent] * local[child]      (root: world = local)
posed[v]      = world[bone(v)] * vertex[v]
```

- With every pose identity, `world[bone]` is the accumulated `bone.local`
  translation, which is exactly `object::rest_positions`.
- A missing or non-finite pose is the identity local, never a panic.
- Composition is bounded by the bone count: a parent outside the range, or a
  cycle in an unvalidated hierarchy, leaves the affected bone at its own local
  matrix instead of blocking.
- The result is indexed by bone, so a child stored before its parent (or a pose
  slice whose internal order differs) resolves on a later pass to the same
  answer.

The pose-only `compose(hierarchy, poses)` omits the rest offsets; the skinning
path uses `compose_with_locals(hierarchy, poses, bone_locals)`.

## D. Hierarchical skinning

`pose::skin_positions_hierarchical(model, hierarchy, poses)` refuses a hierarchy
whose bone count differs from the model, then applies each bone's composed world
matrix to that bone's `first_vertex..=last_vertex` range. The existing
`pose::skin_positions` (local pose over rest) remains the default `--pose`
behaviour; `--object-mesh ... --pose <FILE> --hierarchy` selects the composed
path, and `--bounds` prints the result without opening a window.

## E. Runtime root-vs-leaf witness

Input: one 30-degree rotation about Z (`0.8660254 -0.5 0 0.5 0.8660254 0 0 0 1
1 0 0 0`) in the chosen bone's 13-float line; every other line is identity. The
`--hierarchy --bounds` line reports the composed tree, the moved-vertex count and
the bounding box.

| Model | Bones | Rest bbox `min` / `max` | Root rotation | Leaf rotation |
|---|---:|---|---|---|
| `raziel` | 66 | `(-90,-86,3)` / `(90,284,509)` | bone 0: moved **299/299**, `(-174.909,-84.112,3)` / `(70.739,265.451,509)` | bone 9: moved **8**, `(-90,-85,3)` / `(90,284,509)` |
| `skinner` | 35 | `(-98,-79,1)` / `(100,35,569)` | bone 0: moved **173/173**, `(-84.870,-94.416,1)` / `(86.603,62.883,569)` | bone 7: moved **4**, `(-98,-80.399,1)` / `(100,35,569)` |

The root rotation carries every descendant frame and moves the whole model; a
leaf rotation moves only its own range (4-8 vertices) and barely disturbs the
box. Bones with no vertex range (`0xFFFF` first/last) move nothing by
construction.

## F. Headless digest

The engine path is untouched. `--play --headless --unit under1 --ticks 60`
still prints `digest=ede39c540a10ee39`, identical to the pre-change value.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| `+18` is a signed parent index | a valid body model whose `+18` values do not form a tree | Tested: raziel and skinner match, one root, acyclic |
| Real body models carry exactly one root | a multi-bone body model with two roots | `matsrevr` has two, reported as `multiple_roots` |
| Composition is order-independent | a child-before-parent hierarchy composing differently | Tested by index; no order dependence |
| Root rotation moves the whole model | a descendant vertex that does not move | Tested: raziel 299/299, skinner 173/173 |
| Leaf rotation is local | a vertex outside the leaf range that moves | Tested: raziel bone 9 moves 8, skinner bone 7 moves 4 |

## Limits

- The `+18` root convention is `-1`, per the live body dump. The one-bone
  self-parent shape is reported as a failure; a second convention stays
  **UNKNOWN** rather than being assumed.
- The 13-float slot pose remains the deformation input; how the runtime produces
  those floats (frame selection, mode bits, composition order) is still out of
  scope (`docs/notes/object-models.md`).
- The scale marker at index 9 is still not applied, exactly as in the
  non-hierarchical path.
- `.SRM` (`extra = 0`) keeps its flat 128-slot pool; this hierarchy does not
  apply to it.

## Legal

Read-only. No game process was launched or instrumented. No game file was
written, no game bytes were copied into the repository, and no disassembly,
address, file offset or install path appears here. Only structure, field roles,
format constants and corpus counts are recorded.
