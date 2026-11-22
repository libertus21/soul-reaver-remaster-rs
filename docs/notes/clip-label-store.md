# The versioned clip-label store

## What it is

The per-actor animation store maps a `.SRA` file name to the clip index the
engine selected for `idle`, `walk`, `jump_up`, `fall` and `land`. The engine
reads it in `src/engine/actors.rs`; the measurement probe writes it.

The content is **only clip indices and `.SRA` file names** plus measurement
metadata (`agreement`, `field_agreement`, `run_median`, `samples`, `runs`,
`evidence`, `updated`). It carries no game bytes, no addresses, no RVAs and no
install paths, so versioning it is safe.

## Canonical location

```text
data/clip-labels.json
```

This repository copy is the durable record. The probe writes its historical
location, `~/.sr1_clip_labels.json`, which lives outside version control. The
engine merges both, so a fresh probe run reaches the renderer and a lost home
file still leaves the committed measurements intact.

`.gitignore` is a whitelist, so the copy is allowed explicitly:

```text
!data/
!data/clip-labels.json
```

The file must stay UTF-8 **without a BOM**; the parser rejects byte `0xEF` at
offset 0. A unit test asserts the committed file's first byte is `{`.

## Engine lookup and merge

`PlayOptions::labels_store_sources` returns the stores to read:

1. `--labels-store <FILE>`, when given. An explicit path is the only source and
   wins even when it does not exist; the existing "missing file means every
   state at rest" behaviour applies.
2. Otherwise both `data/clip-labels.json` (when it exists) and the probe's
   user-home default, `~/.sr1_clip_labels.json` (from `USERPROFILE` or `HOME`).

The two sources are **merged per field**: for each file and state, the label
from the source with the newer ISO-8601 `updated` stamp wins. A state missing
from the newer entry keeps the older measured value. Equal stamps keep the
repository copy, so the result is deterministic.

This merge is the consumption rule. Before it, the loader read the first
existing file only, so a probe run that wrote `~/.sr1_clip_labels.json` never
reached the renderer while the versioned copy existed. The repository copy stays
the durable baseline; the home copy can add or refresh fields without replacing
the whole entry.

A missing source is not an error. A **malformed** source is a loud error that
names the resolved path; it is never silently skipped. The stores are only read
when `--sra-dir` is supplied.

## Schema and current values

The root object holds `version`, `conflicts` and `entries`. `entries` is keyed
by uppercase `.SRA` file name. Label values are JSON strings (`"55"`) or
numbers; `"unknown"` and `null` mean "not measured" and leave that state at
rest.

Current measured content:

| File | idle | walk | jump_up | fall | land | evidence |
|---|---:|---:|---|---|---|---|
| `RAZIEL.SRA` | 55 | 2 | 35 | 36 | 37 | `runtime-capture` |
| `SLUAGH.SRA` | 8 | 1 | unknown | unknown | unknown | `runtime-motion` |

The accepted `RAZIEL.SRA` map is the controlled-capture result documented in
`docs/changes/runtime-capture/clip-labelling.md`. Do not replace or hand-edit a
confirmed value without a measurement; the probe's precedence rule protects
`capture` from `motion`.

## Visual evidence

`--clip-inspect <N> --sra-dir <DIR>` plays clip `N` on every resolved actor,
looped, in a window. `[` and `]` move to the previous/next clip, clamped to the
target file's clip count; `Tab` cycles the target `.SRA`; `1`..`5` assign the
current clip to `idle`, `walk`, `jump_up`, `fall` and `land`; `Z` or Backspace
clears pending assignments; Enter saves them. The window title and a printed
status line always carry the clip, its frames/divisor/duration and the pending
assignments.

A visual field is written through `LabelStore::save_visual`, the normal store
write path, with:

- the label as a JSON string;
- `field_method: "visual"` in the entry's per-field map;
- an `assigned_at` stamp in `field_assigned_at` (and at the entry level);
- `evidence: "visual"` when every labelled field in the entry is visual.

The write preserves every other field, including `slots`, `agreement`, `runs`
and `samples`. A visual-only entry carries no `runs` or `samples`: the user's
explicit action is the evidence, and the engine never fabricates counters. The
parser tolerates those absent fields.

The precedence is `capture > visual > motion`, resolved per field before any
timestamp comparison. `motion` never replaces `visual`; `visual` may replace
`motion`; `capture` is never replaced by either. Within one method the newer
`assigned_at` (or `updated`) wins. At playback the engine consumes a visual
label exactly like a motion label.

## Verification

- `engine::actors::tests::the_versioned_store_parses_to_the_measured_labels`
  parses this exact file and asserts `raziel 55/2/35/36/37` and
  `sluagh idle=8 walk=1`, and that there is no BOM.
- `engine::actors::tests::a_malformed_store_reports_the_failing_path` proves a
  bad file fails loudly with its path.
- `engine::play::tests::label_store_lookup_is_explicit_then_repo_then_home`
  pins the single-path order.
- `engine::play::tests::label_store_sources_merge_repo_and_home_by_default`
  proves both default sources are read and an explicit store stays alone.
- `engine::actors::tests::label_stores_merge_per_field_with_the_newer_updated_winning`
  proves the per-field merge: newer wins, a missing field keeps the older
  value, and a home-only file is admitted.
- `engine::actors::tests::a_visual_assignment_writes_method_and_assigned_at_without_run_metadata`
  proves the visual save path, its metadata and the absent counters.
- `engine::actors::tests::precedence_is_motion_then_visual_then_capture` pins
  the evidence precedence.
- `engine::actors::tests::clip_inspect_selects_clip_n_for_every_actor_and_clamps_shorter_files`
  proves clip selection and per-file clamping.
- `engine::play::tests::clip_inspect_parses_but_refuses_headless_and_a_missing_sra_dir`
  pins the typed refusals.
