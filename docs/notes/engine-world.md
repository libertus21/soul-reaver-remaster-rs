# Engine world and placed-object database (T3)

`engine::world::World` owns the loaded level identity (`LevelData { name,
entry, rooms }`) and the authored placements (`PlacedDb`).

`PlacedObject { name, position: [i16; 3], object_type, archetype, flags,
instance_id: u32 }` comes from the level object array. `object_type` is the
high 16 bits of `+0x28`, `instance_id` its low 16 bits, and `archetype` the
`u16` at `+0x26` (`docs/notes/object-types.md`, section 2.1). `instance_id` is
widened to `u32` in the engine record.

## Query, never iterate per frame

`PlacedDb` keeps `records` private and exposes no iterator: `by_name(name) ->
&[slot]`, `by_id(instance_id) -> &[slot]`, `record(slot) -> Option<&PlacedObject>`.
The indexes are built once at load. There is no `iter()`, no `IntoIterator` and
no public `records()` slice, so a per-frame walk cannot be written without
adding one deliberately.

## Duplicate names

Duplicate names are normal (`marker` is the common case). Every placement stays
a separate record; the name index maps a name to every slot that carries it.
Deduplication belongs to the tracker's name-keyed slot policy (T4), not to the
authored database.

## Entry resolution

`level::resolve_unit_entry(dat, name) -> Option<usize>` reuses the archive scan,
including the header-name plus BIGFILE-suffix rule. `World::load` then reuses
`level::read_entry_objects`, so the `.PCM` parser is not duplicated.

Runtime check against the retail archive (not committed, no game bytes):
`under1 entry=115 placed=95 markers=26`. Entry 115 is what the archive reports,
not a hardcoded guess.

## Determinism

T3 adds no tick loop and does not write `WorldState.live`. The canonical fold
and `GOLDEN_WORLD_STATE_HASH` are unchanged; the determinism goldens do not move
in this slice.
