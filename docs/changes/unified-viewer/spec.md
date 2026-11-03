# Spec: unified-viewer

The binary gains a grouped, discoverable entry point:
`soul-reaver-remaster-rs <group> <action> [operands]`, a `--help` that
groups every capability, and an optional `menu` picker. Every existing
invocation keeps its exact stdout, stderr, and exit code. The new forms
are translated into the legacy argv and executed by the existing
handlers; validation, messages, and rendering stay in one place.

## Quick path

1. `cargo run -- --help` lists five groups with one example per
   capability and exits 0.
2. `cargo run -- texture list <BIG>` behaves exactly like
   `cargo run -- --textures <BIG> --tlist`.
3. `cargo run -- menu` (TTY only) lists the catalog and prompts for the
   operands of the chosen capability.
4. `cargo run -- --textures <BIG> --tlist` is unchanged.

## Requirements

| ID | Requirement |
|---|---|
| R1 | The five capability groups are `texture`, `archive`, `mesh`, `audio`, `level`; their union covers every current mode. `texture` groups DDS file, DDS gallery, `.BIG` slot listing, and slot show (with `--map`); `archive` groups the SR1PC `.DAT` index and one archived texture; `mesh` groups plain, textured, gallery, and one-bone pose; `audio` groups wave listing and WAV export; `level` groups `info`, `records`, `structure`, `links` |
| R2 | `--help`, `-h`, and `help` print the grouped catalog to stdout and exit 0. `help <group>` prints that group only. Help output is generated from the capability table, never hand-duplicated |
| R3 | New grammar: `texture list <BIG> [--map <MAP>]`, `texture show <BIG> <index\|name> [--map <MAP>]`, `texture file <FILE>`, `texture gallery <DIR>`, `archive list <DAT>`, `archive show <DAT> <index\|hash>`, `mesh show <FILE.SRM> [--pose <bone>:<degrees>]`, `mesh textured <FILE.SRM> --tex-dir <TEX>`, `mesh gallery <DIR> --tex-dir <TEX>`, `audio list <FILE.SND>`, `audio wave <FILE.SND> <n> --out <FILE.wav>`, `level info\|records\|structure\|links <path>` |
| R4 | Canonicalization: each new form maps to exactly this legacy argv and nothing else: `texture list` -> `--textures <BIG> --tlist [--map <MAP>]`; `texture show` -> `--textures <BIG> --tshow <SEL> [--map <MAP>]`; `texture file`/`texture gallery` -> `<PATH>`; `archive list` -> `--bigfile <DAT> --list`; `archive show` -> `--bigfile <DAT> --show <SEL>`; `mesh show` -> `--mesh <SRM> [--pose <SPEC>]`; `mesh textured` -> `--mesh-textured <SRM> --tex-dir <TEX>`; `mesh gallery` -> `--mesh-all <DIR> --tex-dir <TEX>`; `audio list` -> `--audio <SND> --alist`; `audio wave` -> `--audio <SND> --wave <N> --out <OUT>`; `level X` -> `--level-X <PATH>`. Option flags may appear in any position after the first operand and are re-emitted in the canonical order above |
| R5 | Router precedence is fixed: empty argv -> legacy (unchanged usage, exit 1); `argv[0]` in `{--help, -h, help}` -> help; `argv[0] == menu` -> picker; `argv[0]` in the five groups -> new grammar; anything else -> legacy argv, untouched, including unknown words and paths |
| R6 | Reserved words: `texture`, `archive`, `mesh`, `audio`, `level`, `menu`, `help`, `--help`, `-h` are reserved in `argv[0]`. A filesystem path with one of those names is addressed as `./name`. This shadowing is documented and covered by a test; it is never silent |
| R7 | Loud failures: missing operand, unknown action, unknown flag, a flag without its value, or an extra operand in the new grammar prints a clear stderr line naming the token and exits 1. Legacy invocations keep their exact current messages and exit codes, including the level modes' 1/2/3 |
| R8 | The picker is one-shot: it lists numbered capabilities from the same table, prompts only for the operands that capability declares, builds the same canonical argv as R4, and returns it to the dispatcher. `q` or EOF exits 0 with no action. An empty required operand re-prompts; the picker never validates file contents or paths itself |
| R9 | Headless safety: the picker runs only when stdin and stdout are both terminals (`std::io::IsTerminal`). `menu` with piped stdin prints a loud error and exits 1 instead of blocking. `--help` and every subcommand are fully non-interactive |
| R10 | No dependency change. `Cargo.toml` and `Cargo.lock` are byte-identical after the change. No `clap`, no `egui`/`eframe`, no `is-terminal` crate |
| R11 | No handler change: `run_bigfile`, `run_textures`, `run_mesh`, `run_mesh_textured`, `run_mesh_all`, `run_audio`, `run_level_*`, `show_texture`, `strip_launcher_noops`, `pose_mode_conflict`, and the render/decode modules are not edited. The only edit in `main.rs` is extracting the existing chain into `fn dispatch(&[String]) -> ExitCode` verbatim plus the router hook |
| R12 | Legal/tests: tests use synthetic bytes and temp dirs only; no game files touched; no game bytes, absolute install paths, coordinates, or hashes in code or docs; the whitelist `.gitignore` stays |

## Acceptance scenarios

- [ ] **S1 Legacy texture listing:** `--textures <BIG> --tlist` and
      `--textures <BIG> --tlist --map <MAP>` print the same lines and exit
      the same code as before this change.
- [ ] **S2 Legacy texture show:** `--textures <BIG> --tshow <SEL>` and the
      `--map` variant keep their stdout, stderr, and exit codes, including
      the shared-name refusal.
- [ ] **S3 Legacy positional modes:** a single `.DDS` path and a directory
      path keep their current gallery behavior and exit codes.
- [ ] **S4 Legacy archive/mesh/audio/level:** every `--bigfile`,
      `--mesh`, `--mesh-textured`, `--mesh-all`, `--audio`, `--level-*`
      form keeps its current messages and exit codes; `--pose` position
      refusal is unchanged.
- [ ] **S5 Discoverable entry:** `--help` exits 0, names all five groups,
      and shows one usage line per capability; `help mesh` shows only the
      mesh group and still lists the pose form.
- [ ] **S6 Equivalent dispatch:** for each capability, the router returns
      the exact legacy argv from R4 (asserted as a vector equality, which
      is the byte-compatibility proof), and dispatching that argv reaches
      the same handler the legacy form reaches.
- [ ] **S7 Invalid new input:** `texture`, `texture frobnicate <BIG>`,
      `texture show`, `texture list <BIG> --bogus`, `mesh textured <SRM>`
      (missing `--tex-dir`), and `audio wave <SND>` each exit 1 with a
      message naming the bad token.
- [ ] **S8 Unknown legacy input:** `frobnicate` and `--frobnicate` keep
      today's behavior (treated as a path / unknown arg, exit 1); the
      router does not reinterpret them.
- [ ] **S9 Empty argv unchanged:** no arguments prints the current usage
      block and exits 1, with the `menu` hint absent from that block (the
      block is not edited).
- [ ] **S10 Picker session (scripted):** a `Cursor` feeding a capability
      number and its operands yields the canonical argv; `q` and EOF both
      exit 0 with no argv; an empty required operand re-prompts; a
      non-numeric choice re-prompts.
- [ ] **S11 Headless:** `menu` with non-TTY stdin exits 1 loud and never
      blocks; `--help`, `help <group>`, and every subcommand work with
      stdin closed.
- [ ] **S12 Reserved word shadowing:** `mesh` alone shows the mesh group
      help (exit 0); a temp directory literally named `mesh` is browsed
      through `./mesh` in a legacy-shaped invocation.
- [ ] **S13 No new dependency:** `git diff` for `Cargo.toml` and
      `Cargo.lock` is empty at every PR.
- [ ] **S14 Legal:** `git status --short` shows only intended source/doc
      files; tests reference no install path; docs use relative names.

## Next step

See `design.md` for the option comparison, the router precedence table,
the argv mapping, and the module boundaries; `tasks.md` for the three-PR
chain and the line forecast.
