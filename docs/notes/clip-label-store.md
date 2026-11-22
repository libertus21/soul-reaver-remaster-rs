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

This repository copy is the durable record. The probe's historical location,
`~/.sr1_clip_labels.json`, still works but lives outside version control: if that
home-directory file is lost, every measurement is lost. The committed copy makes
the measurements reviewable and recoverable.

`.gitignore` is a whitelist, so the copy is allowed explicitly:

```text
!data/
!data/clip-labels.json
```

The file must stay UTF-8 **without a BOM**; the parser rejects byte `0xEF` at
offset 0. A unit test asserts the committed file's first byte is `{`.

## Engine lookup order

`PlayOptions::labels_store_path` resolves one path, in this order:

1. `--labels-store <FILE>`, when given. An explicit path wins even when it does
   not exist; the existing "missing file means every state at rest" behaviour
   applies.
2. `data/clip-labels.json` relative to the current directory, when that file
   exists. This is the versioned copy.
3. The probe's user-home default, `~/.sr1_clip_labels.json` (from `USERPROFILE`
   or `HOME`; a relative name when neither variable exists).

A missing repository copy is not an error: lookup falls through to the home
path. A **malformed** copy is a loud error that names the resolved path; it is
never silently skipped. The store is only read when `--sra-dir` is supplied.

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

## Verification

- `engine::actors::tests::the_versioned_store_parses_to_the_measured_labels`
  parses this exact file and asserts `raziel 55/2/35/36/37` and
  `sluagh idle=8 walk=1`, and that there is no BOM.
- `engine::actors::tests::a_malformed_store_reports_the_failing_path` proves a
  bad file fails loudly with its path.
- `engine::play::tests::label_store_lookup_is_explicit_then_repo_then_home`
  pins the lookup order.
