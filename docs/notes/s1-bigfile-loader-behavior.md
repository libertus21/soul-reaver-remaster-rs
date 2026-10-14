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
