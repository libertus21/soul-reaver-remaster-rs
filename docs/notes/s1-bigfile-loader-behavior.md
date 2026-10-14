# Behavior notes: s1-bigfile-loader loader path (apply-time re-pin)

Source of truth for the Task 2 mapping. Behavior paraphrases only — no
disassembly, no decompiled code, no Ghidra databases in the repo. Raw
function addresses are deliberately WITHHELD per repo hygiene
(`AGENTS.md` rule 3); survey-era addresses were starting hints only and
are not repeated here, not even as hints. Every role label below was
assigned fresh at apply time and is re-derivable from the strings alone
(see Re-run).

## Re-pin method (bounded, headless)

- Engine `12.1.4` headless, existing project under `%TEMP%` reopened
  read-only with analysis OFF (no import, no writes to the binary).
- One probe pass: case-insensitive search of defined strings for the two
  loader error formats + the default archive name; for each hit, the
  referencing functions (xrefs) and counts; for the file-IO imports, the
  caller functions and per-function site counts. No function body was
  read beyond what naming the stage required; nothing was pasted.
- Result: `function_count=5243`, `matched_string_count=3`, `probe_done=1`.

## Re-run (second pair of eyes)

1. Open the existing `%TEMP%` Ghidra project read-only, `-noanalysis`.
2. Search defined strings for `can't load big file named`, `can't read
   from bigfile`, `bigfile.dat` (case-insensitive).
3. Follow xrefs from each hit to its referencing function(s); list the
   callers of `CreateFileW`, `SetFilePointerEx`, `GetFileSizeEx`,
   `ReadFile` with per-function site counts.
4. The role labels + counts below must reproduce exactly.

## String evidence (fresh)

| Role | String content (short evidence quote) | Code xrefs |
|---|---|---|
| `load-failure-format` | `Can't load big file named "%s"` (+ trailing newline in binary) | 1, to a single function (`open-error-half`) |
| `read-failure-format` | `Can't read from bigfile: %s` | 1, to a single function (`read-error-half`), distinct from `open-error-half` |
| `default-archive-name` | `bigfile.dat` | 2: the single-export entry (`run`, within its first bytes — launcher-to-game default-archive handoff) + one archive-referencing function |

## Caller clusters (fresh)

| Role | Import | Shape |
|---|---|---|
| `open-cluster` | `CreateFileW` | 3 call sites in one function + 1 site each in two further functions; remaining callers are CRT console helpers (out of scope) |
| `seek-size-pair` | `SetFilePointerEx` + `GetFileSizeEx` | BOTH imports called from one shared function (seek-plus-size pair) |
| `read-cluster` | `ReadFile` | 5 caller functions sitting in one neighborhood |
| `seek-extra` | `SetFilePointerEx` | 3 further caller functions outside the pair |

The open cluster and the seek/size/read cluster are DISJOINT function
sets — the shape of an open-then-read loader. Call EDGES between the
clusters and the error halves were NOT traced (out of the bounded
method), so the call order below is shape-consistent but
edge-unconfirmed.

## Call order (working shape, edges UNKNOWN)

`open (open-cluster)` → `size/seek (seek-size-pair, seek-extra)` →
`read (read-cluster)` → `error (open-error-half | read-error-half)`.
The ORDER itself is inference from cluster roles, not an observed call
graph — recorded as INFERRED, never as fact.

## Per-step failure conditions + stage-to-message mapping

| # | Stage (behavior paraphrase) | Message family | Verdict |
|---|---|---|---|
| O1 | Open fails (archive cannot be opened) | `load-failure-format` with the archive name substituted | CONFIRMED family: the format exists, has exactly one code referrer, and a missing/unopenable archive can fail ONLY at open |
| O2 | Seek/size/read fails after a successful open | `read-failure-format` with the archive identity substituted | CONFIRMED family (exists, single referrer distinct from O1), but the PER-CASE stage assignment is UNKNOWN — no call-edge evidence ties any specific read (count word, table, entry bytes) to `read-error-half` |
| O3 | Process termination on loader failure | UNKNOWN — never observed; no exit-code/dialog/return claim is made | UNKNOWN (static headless only; nothing was executed) |

## Per-failure-case verdicts (input to Task 2)

| Failure case | Verdict | Reason |
|---|---|---|
| Missing file (open fails) | CONFIRMED → O1 | Fails at open by construction on both sides; O1 is the open-stage family |
| Truncated count word / bad count | UNKNOWN → EXCLUDED | Unconfirmed which original stage reads the count word |
| Truncated index table mid-read | UNKNOWN → EXCLUDED | Post-open on our side, but the original's table-read stage is unconfirmed |
| Entry range past EOF (bad index data) | UNKNOWN → EXCLUDED | No string/call evidence ties it to either half |
| Seek failure during entry read | UNKNOWN → EXCLUDED | Seek cluster confirmed to exist, but its error routing is unconfirmed |
| Short read of entry bytes | UNKNOWN → EXCLUDED | Read cluster confirmed to exist, but its error routing is unconfirmed |
| Termination behavior | UNKNOWN → NO CHANGE | O3 unobserved; our loud-stderr + exit-1 contract stays untouched |

## Deliberately not established

Which original function reads the count word / index table / entry
bytes; any call edge into `open-error-half` / `read-error-half`; any
termination observable; anything about other archives, hash-name
lookup, or non-DDS handling (out of slice scope).

## Legal

Binaries read-only; Ghidra project outside the repo under `%TEMP%`. No
game bytes, paths, hashes, or offsets are recorded here or anywhere in
the change. The two short user-visible message formats above are
evidence quotes already present in the committed survey.

---

# Addendum S2: call-edge trace (apply-time, 2026-10-14)

Task 1 of `docs/changes/s2-read-error-parity/tasks.md`. Behavior
paraphrases only — same clean-room discipline as S1 (no disassembly,
no decompiled code, no Ghidra databases in the repo, no game data).
Role labels below are assigned fresh at apply time and are
re-derivable from the strings alone (see Re-run S2).

## Method (bounded, headless, 2 passes used of 2)

- Engine `12.1.4` headless, existing project under
  `%TEMP%\opencode\ghidra-probe` reopened read-only with analysis OFF
  (no import, no writes to the binary). Probe scripts live outside the
  repo under `%TEMP%\opencode\s2-edge` (`EdgeTraceProbe.java`,
  `EdgeTraceProbe2.java`) and emit only RVAs, function labels, import
  names, short string contents, reference counts, and
  caller-to-callee pairs.
- Pass 1: re-pin by STRING XREF (both error formats, default archive
  name, all four IO imports) + depth-1 edges — callees of every
  cluster function, callers of each error-half function, every
  cluster-to-error-half caller-to-callee pair.
- Pass 2 (final pass of the box): depth-2 around each error half —
  callers + callees of every error-half caller, flagged wherever a
  mid-level caller reaches an IO import.
- Box closed by the pass budget (2 of 2 used), wall time minutes, not
  hours. No DRM/anti-cheat/packing was encountered on this line.

## Re-pin (reproduces S1 exactly)

- `function_count=5243`, `matched_string_count=3` — identical to S1.
- `load-failure-format` (`Can't load big file named "%s"`): 1 xref, to
  a single function (`open-error-half`), distinct referrer as in S1.
- `read-failure-format` (`Can't read from bigfile: %s`): 1 xref, to a
  single function (`read-error-half`), distinct from `open-error-half`.
- `default-archive-name` (`bigfile.dat`): 2 xrefs — the single-export
  entry (`run`) + one archive-referencing function — as in S1.
- Cluster shapes unchanged from S1: `open-cluster` (`CreateFileW`: 3
  sites in one function + 1 site each in two further functions, rest
  CRT console helpers out of scope), `seek-size-pair`
  (`SetFilePointerEx` + `GetFileSizeEx` from one shared function),
  `read-cluster` (`ReadFile`: 5 callers in one neighborhood),
  `seek-extra` (`SetFilePointerEx`: 3 further callers outside the pair).
  15 distinct cluster functions total (3 console helpers included).

## Observed edges (pass 1, depth-1)

- `direct_edge_count=0`: NO cluster function calls either error half.
  Every IO-cluster member's callee set is its IO import plus nearby
  helpers — neither error half appears in any of the 15 callee sets.
- `overlap_count=0` for both halves: NO error-half caller is itself a
  cluster member.
- Only shared reach: a console-scope read function reaches two
  `ReadFile` cluster members — CRT console helpers, out of scope per
  S1 (not a loader edge).

## Observed edges (pass 2, depth-2)

- `read-error-half` has exactly 3 callers, each a thin single-callee
  wrapper whose SOLE callee is `read-error-half`:
  - `read-half-caller-V`: 3 static call sites from 2 higher functions
    (one calls it twice, another once). Genuine static path, but the
    wrapper performs no IO itself and calls no IO import.
  - `read-half-caller-U`: ZERO static callers (no static path; reached
    only indirectly if at all).
  - `read-half-caller-W`: ZERO static callers (same as above).
- `EDGE2` (mid-level caller reaching an IO import): 0 observed — none
  of the three wrappers calls `CreateFileW` / `ReadFile` /
  `SetFilePointerEx` / `GetFileSizeEx`.
- `open-error-half` has exactly 1 caller: the archive-referencing
  function (the second `bigfile.dat` referrer), itself called once by
  the single-export entry (`run`). Edge
  `archive-referencing-function -> open-error-half`
  (`UNCONDITIONAL_CALL`) is OBSERVED — this reinforces S1 row O1 and
  changes no code (S1 already aligned that row).

## Per-row verdicts (input to Task 2 — all HELD)

| Failure case | Verdict | Edge evidence |
|---|---|---|
| Seek failure during entry read | HELD → EXCLUDED from Task 3 | No seek-cluster → `read-error-half` edge (pass 1: 0 direct, 0 overlap; pass 2: no wrapper reaches a seek import) |
| Short read of entry bytes | HELD → EXCLUDED from Task 3 | No read-cluster → `read-error-half` edge (same zeros; wrappers call nothing but the half itself) |
| Truncated count word / bad count | HELD → EXCLUDED from Task 3 | No count-word reader identified; no edge from any reader stage to either half |
| Truncated index table mid-read | HELD → EXCLUDED from Task 3 | No table-read stage identified; no edge from any reader stage to either half |
| Entry range past EOF (bad index data) | HELD → EXCLUDED from Task 3 | No bounds-check → error-half edge, as predicted (likely no original counterpart) |
| Termination behavior | UNKNOWN → NO CHANGE | Static headless only; nothing was executed, no new observable |

Nothing resolved to PROVEN: the single observed loader edge
(`archive-referencing-function -> open-error-half`) belongs to the
already-aligned S1 open-stage row. Per the plan's box rule, S2 narrows
to the docs-only slice (this addendum + hold pins) and Task 3 makes
zero production-line changes.

## Re-run S2 (second pair of eyes)

1. Open `%TEMP%\opencode\ghidra-probe` read-only, `-noanalysis`.
2. Pass 1: search defined strings for `can't load big file named`,
   `can't read from bigfile`, `bigfile.dat` (case-insensitive); follow
   xrefs to referrers; list callers of `CreateFileW`, `ReadFile`,
   `SetFilePointerEx`, `GetFileSizeEx`; for each cluster function list
   callees (`getCalledFunctions`); for each error-half function list
   flow callers (references to entry of type CALL/JUMP/BRANCH).
   Expect: `direct_edge_count=0`, both overlaps 0.
3. Pass 2: for each error-half caller, list its flow callers and its
   callees; flag any callee that is one of the four IO imports.
   Expect: read half has 3 single-callee wrapper callers (3-site /
   0-site / 0-site shapes); open half has 1 caller (the second
   `bigfile.dat` referrer, called by the single-export entry).
4. The verdict table above must reproduce exactly from those outputs.

## Legal (addendum)

Same as S1: binaries read-only, Ghidra work outside the repo, no game
bytes/paths/hashes/offsets recorded. Probe outputs stay under
`%TEMP%` and are not committed.
