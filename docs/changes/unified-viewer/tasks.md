# Tasks: unified-viewer

Ordered work units, delivered as a fixed three-PR chain. Each PR is
independently committable, keeps its tests and docs with its behavior, and
stays below the 400-authored-line ceiling. `test_command` for every apply
phase: `cargo test`. Tests use synthetic bytes and temp dirs only.

## PR 1 - `feat(cli): add the capability catalog and grouped help`

- [ ] Add `src/cli_catalog.rs` (pure, no IO): `Cap` enum with one variant
      per capability (15 total), `Spec { key, group, summary, operands }`,
      `Operand { name, required }`, the five `GROUPS` in display order.
- [ ] `Cap::argv(operands: &[&str]) -> Vec<String>` emits only the legacy
      argv from the design table; an empty optional operand is omitted.
      This is the single mapping used by the router and the picker.
- [ ] `help_text() -> String` and `group_help(group) -> String` render
      from the table: group headers, one usage line per capability, a
      trailing note that every legacy flag form is unchanged.
- [ ] Register `mod cli_catalog;` in `main.rs` and route
      `--help`/`-h`/`help`/`help <group>` to it. Unknown group name is a
      loud error naming the token and exits 1.
- [ ] Help goes to stdout through `cli_out::print_line`, so a closed pipe
      still exits 0.
- [ ] Unit tests: every `Cap` is in exactly one group and every group is
      non-empty; `argv` for all 15 forms (required-only, with optional,
      option-order normalization); `help_text` contains every capability
      key and every group; `group_help` is a strict subset; unknown group
      error text.
- [ ] Verify: `cargo test` green (245 existing + new); `Cargo.toml`
      diff empty; `git status --short` only intended files.

## PR 2 - `feat(cli): route grouped subcommands to the existing dispatch`

- [ ] Add `src/cli_argv.rs` (pure): `Route { Help(Option<String>),
      Menu, Dispatch(Vec<String>), Error(String) }` and
      `route(&[String]) -> Route` implementing the fixed precedence table.
- [ ] Parse the five groups: action word, positional operands, and the
      `--map`/`--pose`/`--tex-dir`/`--out` options accepted in any order
      after the first operand, then emit the canonical argv via
      `cli_catalog::Cap::argv`.
- [ ] Extract the existing `pose_mode_conflict` + `if` chain from
      `main()` into `fn dispatch(args: &[String]) -> ExitCode` **verbatim**
      (no reordering, no message edits) and have `main` call it.
- [ ] Keep `strip_launcher_noops` before `route` so no-op launcher flags
      are still stripped first.
- [ ] Leave the no-argument usage block untouched; `Route::Dispatch([])`
      reaches it unchanged.
- [ ] Leave `Route::Menu` returning a loud "menu requires a terminal"
      error in this PR (placeholder for PR 3).
- [ ] Tests: router table for all 15 accepted forms (exact vector
      equality, S6); rejected forms (missing operand, unknown action,
      unknown flag, flag without value, extra operand); legacy
      passthrough for unknown words, unknown flags, `--textures ...`,
      `--mesh ... --pose ...`, `.DDS`, and a directory; empty argv;
      `help` precedence; reserved-word shadowing (`mesh` -> group help,
      `./mesh` -> legacy path).
- [ ] A dispatch-level test with one synthetic `.BIG` proving the new
      form and the legacy form return the same exit code.
- [ ] Verify: `cargo test` green; the 245 pre-existing tests are
      unmodified; `git diff Cargo.toml Cargo.lock` empty.

## PR 3 - `feat(cli): add the interactive terminal picker`

- [ ] Add `src/menu.rs`: `run(reader: &mut impl BufRead, out: &mut impl
      Write) -> io::Result<Option<Vec<String>>>`; `main` calls it only
      when `stdin().is_terminal() && stdout().is_terminal()`.
- [ ] Behaviour: numbered capabilities grouped by header, `select>`
      prompt, number -> prompt each declared operand (`[optional]`
      marker), empty required -> re-prompt, non-numeric -> re-prompt,
      `q`/EOF -> `Ok(None)`.
- [ ] Return the canonical argv from the same `Cap::argv` used by the
      router; `main` passes it to `dispatch`. The picker never opens a
      file, never validates a path, and never prints a decode error.
- [ ] Non-TTY `menu` prints a loud error naming the terminal requirement
      and exits 1 without reading stdin.
- [ ] Tests (`Cursor`-driven, no TTY, no process): choose the texture
      list capability and feed operands -> exact argv; optional operand
      left empty -> omitted; bad selection re-prompts; empty required
      re-prompts; `q` and EOF -> `None`; a full run with a synthetic
      `.BIG` reaches the handler and exits with its code.
- [ ] Update `README.md` "What works" with the grouped entry point, the
      `--help` grouping, the picker, the reserved-word rule, and an
      explicit "no new dependency" line. Update the "Next step" paragraph
      only if it still points at a finished slice.
- [ ] Record a short playtest note in the PR body: `--help`, one group
      help, one legacy flag run, and one `menu` session on a real TTY
      (what was selected, what ran, exit code). Relative names only.
- [ ] Verify: `cargo test` green; `Cargo.toml` diff empty; `git status
      --short` only intended files; no absolute install paths in docs.

## Review Workload Forecast

The test estimate is the first-instinct number multiplied by 1.5, as in
the `animation` slice. Production and docs are counted separately. The PR
split is fixed here, not chosen at apply time.

| PR | Production lines | Test lines (first instinct -> x1.5) | Docs lines | Total |
|---|---|---|---|---|
| PR 1 - catalog + help | 85 | 65 -> **100** | 20 | **205** |
| PR 2 - router + dispatch extraction | 115 | 95 -> **145** | 15 | **275** |
| PR 3 - terminal picker | 85 | 70 -> **105** | 30 | **220** |
| **Total** | **285** | **350** | **65** | **700** |

- **Per-PR ceiling:** 400 authored lines. Every PR is under it with 125+
  lines of headroom; PR 2 is the closest at ~275.
- **Session review budget:** 800 lines. The forecast total (~700) fits,
  but it still exceeds the 400-line single-slice ceiling, so this is a
  **chain of three PRs from the start**, not one PR.
- **Dependency budget:** zero. `Cargo.toml` and `Cargo.lock` must be
  unchanged at every PR; a non-empty diff is a stop signal, not a review
  note.
- **PR split plan:** PR 1 = table + help only (no routing). PR 2 = router
  + verbatim `dispatch` extraction + `menu` placeholder error. PR 3 =
  picker + README + playtest note. `main.rs` is touched by PR 2 (extract
  and hook) and PR 3 (one `Menu` arm body); no other file is shared.
- **Stop rule:** if PR 2 exceeds 400 authored lines, do not add the
  picker; split the router tests into a follow-up commit first and keep
  PR 3 unopened. Do not carry overage forward.
- **Why not one PR:** ~700 lines, a new argv surface, a verbatim
  `main()` extraction that must be provably behavior-free, and a
  prompt-driven picker. A single review would blur the compatibility
  proof, which is the whole point of the change.

## Apply gate

- [ ] PR 1 merged only if every `Cap` round-trips to a legacy argv that
      the existing chain accepts, and help is generated from the table.
- [ ] PR 2 merged only if all 245 pre-existing tests pass **unmodified**,
      empty argv prints the current usage and exits 1 byte for byte, and
      the router's vector equality covers all 15 forms.
- [ ] PR 3 merged only if the picker is driven entirely by scripted input
      in tests, `menu` on non-TTY stdin exits 1 without blocking, and a
      TTY playtest is recorded.
- [ ] `git status --short` clean of non-whitelisted files at every merge;
      no game bytes, absolute install paths, coordinates, or hashes in
      any PR.
- [ ] If any step needs a handler or render edit to make the new form
      work, STOP: that is a rewrite, not a unification. Document the
      blocking pair and re-scope.
