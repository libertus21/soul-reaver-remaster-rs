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

A visual save writes **both** stores, with the same `assigned_at` stamp: the
home copy is durable without git involvement, and the repository copy is the
versioned record. A `git checkout` or `git pull` can discard an uncommitted
repository change, but it cannot discard the home copy. Each write is attempted
even when the other fails, and a failure is reported per path.

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

The save writes the probe's home store and the repository copy with the same
assignments and stamp. The console is the audit trail:

```text
clip-inspect saved file=HUNTER.SRA fields=idle=2,walk=7 home=<path> repo=data/clip-labels.json
clip-inspect version-control repo=data/clip-labels.json is modified; commit it to keep the measurement
```

The `saved` line names only the targets that actually wrote. A failed target
gets its own line and is never hidden behind a success:

```text
clip-inspect save failed path=<path> reason=<detail>
```

Enter with nothing pending prints `clip-inspect save skipped: no pending
assignments` and writes nothing. Pending assignments are cleared only when both
writes succeed, so a partial failure can be retried with another Enter. The
status line carries `pending=` and `saved=`; `saved=` stays `none` until a save
writes both stores.

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
  pins the single-path lookup order (the helper is test-only now that the
  inspector saves to both stores).
- `engine::play::tests::label_store_sources_merge_repo_and_home_by_default`
  proves both default sources are read and an explicit store stays alone.
- `engine::actors::tests::label_stores_merge_per_field_with_the_newer_updated_winning`
  proves the per-field merge: newer wins, a missing field keeps the older
  value, and a home-only file is admitted.
- `engine::actors::tests::a_visual_assignment_writes_method_and_assigned_at_without_run_metadata`
  proves the visual save path, its metadata and the absent counters.
- `engine::play::tests::inspector_save_dual_writes_home_and_repo_with_equivalent_fields`
  proves a visual save writes both stores with equivalent fields and stamps,
  and that the console names both targets plus the uncommitted repository copy.
- `engine::play::tests::inspector_save_writes_the_healthy_target_and_reports_the_failed_one`
  proves a broken target never blocks the other and is reported per path.
- `engine::play::tests::inspector_save_with_nothing_pending_is_loud_and_writes_nothing`
  proves an empty Enter prints the skip line and writes nothing.
- `engine::actors::tests::precedence_is_motion_then_visual_then_capture` pins
  the evidence precedence.
- `engine::actors::tests::clip_inspect_selects_clip_n_for_every_actor_and_clamps_shorter_files`
  proves clip selection and per-file clamping.
- `engine::play::tests::clip_inspect_parses_but_refuses_headless_and_a_missing_sra_dir`
  pins the typed refusals.
