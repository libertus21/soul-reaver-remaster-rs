//! Pure argv router: translates grouped subcommands into the legacy argv.
//! `cli_catalog` is the single source of truth for the emitted shape.

use crate::cli_catalog::{self, Capability};

#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Help(Option<String>),
    Menu,
    Dispatch(Vec<String>),
    Error(String),
}

pub fn route(args: &[String]) -> Route {
    route_with_terminal(args, false)
}

/// Empty argv enters the picker only on an interactive stdout; explicit
/// `menu` always routes there and `main` gates the terminal.
pub fn route_with_terminal(args: &[String], stdout_is_terminal: bool) -> Route {
    let Some(first) = args.first() else {
        return if stdout_is_terminal { Route::Menu } else { Route::Dispatch(Vec::new()) };
    };
    let first = first.as_str();
    if first == "menu" { return Route::Menu; }
    if matches!(first, "--help" | "-h" | "help") {
        return Route::Help(args.get(1).cloned());
    }
    if cli_catalog::is_known_group(first) {
        return route_group(first, &args[1..]);
    }
    // Rule 5: an unreserved token can never be reinterpreted.
    Route::Dispatch(args.to_vec())
}

fn route_group(group: &str, rest: &[String]) -> Route {
    let Some(action) = rest.first() else {
        return Route::Error(command_error(group, None));
    };
    let Some(cap) = cli_catalog::catalog()
        .iter()
        .find(|cap| cap.group == group && cap.name == action.as_str())
    else {
        return Route::Error(command_error(group, Some(action)));
    };
    match parse_operands(cap, &rest[1..]).and_then(|(p, o)| build(cap, &p, &o)) {
        Ok(argv) => Route::Dispatch(argv),
        Err(message) => Route::Error(message),
    }
}

fn command_error(group: &str, action: Option<&str>) -> String {
    let commands: Vec<&str> = cli_catalog::catalog()
        .iter()
        .filter(|cap| cap.group == group)
        .map(|cap| cap.name)
        .collect();
    let known = format!("known commands for '{group}': {}", commands.join(", "));
    let groups = format!("known groups: {}", cli_catalog::GROUPS.join(", "));
    match action {
        Some(action) => format!("error: unknown command '{group} {action}'\n{known}\n{groups}"),
        None => format!("error: '{group}' requires a command\n{known}\n{groups}"),
    }
}

fn parse_operands<'a>(
    cap: &Capability,
    rest: &'a [String],
) -> Result<(Vec<&'a str>, Vec<(&'a str, &'a str)>), String> {
    let mut positional = Vec::new();
    let mut options = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let token = rest[i].as_str();
        if cap.takes_value(token) {
            let Some(value) = rest.get(i + 1) else {
                return Err(format!("error: option '{token}' needs a value\n{}", usage(cap)));
            };
            options.push((token, value.as_str()));
            i += 2;
        } else if token.starts_with("--") {
            return Err(format!(
                "error: unknown option '{token}' for '{} {}'\n{}",
                cap.group,
                cap.name,
                usage(cap)
            ));
        } else {
            positional.push(token);
            i += 1;
        }
    }
    Ok((positional, options))
}

fn build(
    cap: &Capability,
    positional: &[&str],
    options: &[(&str, &str)],
) -> Result<Vec<String>, String> {
    let tokens: Vec<&str> = cap.legacy.split_whitespace().collect();
    let mut argv = Vec::new();
    let mut next = 0;
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        if token.starts_with('[') {
            let mut group = Vec::new();
            while i < tokens.len() {
                let part = tokens[i];
                group.push(part.trim_start_matches('[').trim_end_matches(']'));
                i += 1;
                if part.ends_with(']') {
                    break;
                }
            }
            if let Some(value) = option(options, group[0]) {
                argv.push(group[0].to_string());
                argv.push(value.to_string());
            }
        } else if cap.takes_value(token) {
            let Some(value) = option(options, token) else {
                return Err(format!("error: missing required option '{token}'\n{}", usage(cap)));
            };
            argv.push(token.to_string());
            argv.push(value.to_string());
            i += 2; // skip the value placeholder
        } else if token.starts_with('<') && token.ends_with('>') {
            let Some(value) = positional.get(next) else {
                return Err(format!("error: missing operand '{token}'\n{}", usage(cap)));
            };
            argv.push((*value).to_string());
            next += 1;
            i += 1;
        } else {
            argv.push(token.to_string());
            i += 1;
        }
    }
    if next < positional.len() {
        return Err(format!("error: unexpected argument '{}'\n{}", positional[next], usage(cap)));
    }
    Ok(argv)
}

fn option<'a>(options: &[(&str, &'a str)], flag: &str) -> Option<&'a str> {
    options
        .iter()
        .rev()
        .find(|(name, _)| *name == flag)
        .map(|(_, value)| *value)
}

fn usage(cap: &Capability) -> String {
    format!("usage: {} {} -> legacy {}", cap.group, cap.name, cap.legacy)
}

#[cfg(test)]
mod tests {
    use super::{route, route_with_terminal, Route};

    fn argv(input: &[&str]) -> Vec<String> {
        input.iter().map(|token| token.to_string()).collect()
    }

    fn dispatch(input: &[&str]) -> Vec<String> {
        match route(&argv(input)) {
            Route::Dispatch(canonical) => canonical,
            other => panic!("expected Dispatch for {input:?}, got {other:?}"),
        }
    }

    fn error(input: &[&str]) -> String {
        match route(&argv(input)) {
            Route::Error(message) => message,
            other => panic!("expected Error for {input:?}, got {other:?}"),
        }
    }

    fn assert_dispatch(input: &[&str], expected: &[&str]) {
        assert_eq!(dispatch(input), argv(expected), "input {input:?}");
    }

    #[test]
    fn every_catalogued_mode_translates_to_its_legacy_argv() {
        let cases: &[(&[&str], &[&str])] = &[
            (
                &["texture", "list", "TEXTURES.BIG"],
                &["--textures", "TEXTURES.BIG", "--tlist"],
            ),
            (
                &["texture", "show", "TEXTURES.BIG", "3"],
                &["--textures", "TEXTURES.BIG", "--tshow", "3"],
            ),
            (&["texture", "file", "hero.DDS"], &["hero.DDS"]),
            (&["texture", "gallery", "texdir"], &["texdir"]),
            (
                &["archive", "list", "BIGFILE.DAT"],
                &["--bigfile", "BIGFILE.DAT", "--list"],
            ),
            (
                &["archive", "show", "BIGFILE.DAT", "7"],
                &["--bigfile", "BIGFILE.DAT", "--show", "7"],
            ),
            (&["mesh", "show", "hero.SRM"], &["--mesh", "hero.SRM"]),
            (
                &["mesh", "textured", "hero.SRM", "--tex-dir", "tex"],
                &["--mesh-textured", "hero.SRM", "--tex-dir", "tex"],
            ),
            (
                &["mesh", "gallery", "objdir", "--tex-dir", "tex"],
                &["--mesh-all", "objdir", "--tex-dir", "tex"],
            ),
            (
                &["mesh", "object-info", "--bigfile", "BIGFILE.DAT", "--entry", "66"],
                &["--object-info", "--bigfile", "BIGFILE.DAT", "--entry", "66"],
            ),
            (
                &["mesh", "object-info-actors", "--bigfile", "BIGFILE.DAT", "--entry", "66"],
                &["--object-info", "--bigfile", "BIGFILE.DAT", "--entry", "66", "--actors"],
            ),
            (
                &["mesh", "actor-census", "--bigfile", "BIGFILE.DAT"],
                &["--actor-census", "--bigfile", "BIGFILE.DAT"],
            ),
            (
                &["mesh", "actor-census", "--bigfile", "BIGFILE.DAT", "--objects-dir", "OBJECT"],
                &["--actor-census", "--bigfile", "BIGFILE.DAT", "--objects-dir", "OBJECT"],
            ),
            (
                &["audio", "list", "bank.SND"],
                &["--audio", "bank.SND", "--alist"],
            ),
            (
                &["audio", "wave", "bank.SND", "2", "--out", "out.wav"],
                &["--audio", "bank.SND", "--wave", "2", "--out", "out.wav"],
            ),
            (&["level", "info", "LAIR1.PCM"], &["--level-info", "LAIR1.PCM"]),
            (
                &["level", "records", "LAIR1.PCM"],
                &["--level-records", "LAIR1.PCM"],
            ),
            (
                &["level", "structure", "LAIR1.PCM"],
                &["--level-structure", "LAIR1.PCM"],
            ),
            (
                &["level", "links", "LAIR1.PCM"],
                &["--level-links", "LAIR1.PCM"],
            ),
            (
                &["level", "render", "LAIR1.PCM"],
                &["--level-render", "LAIR1.PCM"],
            ),
            (
                &["level", "objects", "LAIR1.PCM"],
                &["--level-objects", "LAIR1.PCM"],
            ),
            (
                &["level", "render-objects", "LAIR1.PCM"],
                &["--level-render", "LAIR1.PCM", "--objects"],
            ),
            (
                &["level", "objects-entry", "--entry", "115", "--bigfile", "BIGFILE.DAT"],
                &["--level-objects", "--bigfile", "BIGFILE.DAT", "--entry", "115"],
            ),
            (
                &["level", "objects-group", "--bigfile", "BIGFILE.DAT", "--group", "under"],
                &["--level-objects", "--bigfile", "BIGFILE.DAT", "--group", "under"],
            ),
            (
                &[
                    "level", "geometry-entry", "--bigfile", "BIGFILE.DAT", "--entry", "115",
                ],
                &[
                    "--level-geometry", "--bigfile", "BIGFILE.DAT", "--entry", "115",
                ],
            ),
            (
                &[
                    "level", "render-entry", "--entry", "115", "--bigfile", "BIGFILE.DAT",
                ],
                &[
                    "--level-render", "--bigfile", "BIGFILE.DAT", "--entry", "115",
                ],
            ),
            (
                &[
                    "level", "geometry-group", "--bigfile", "BIGFILE.DAT", "--group", "under",
                ],
                &[
                    "--level-geometry", "--bigfile", "BIGFILE.DAT", "--group", "under",
                ],
            ),
            (
                &[
                    "level", "render-group", "--group", "under", "--bigfile", "BIGFILE.DAT",
                ],
                &[
                    "--level-render", "--bigfile", "BIGFILE.DAT", "--group", "under",
                ],
            ),
            (
                &["level", "links-entry", "--bigfile", "BIGFILE.DAT", "--entry", "115"],
                &["--level-links", "--bigfile", "BIGFILE.DAT", "--entry", "115"],
            ),
            (
                &["level", "links-group", "--bigfile", "BIGFILE.DAT", "--group", "under"],
                &["--level-links", "--bigfile", "BIGFILE.DAT", "--group", "under"],
            ),
            (
                &["level", "reach", "--bigfile", "BIGFILE.DAT", "--start", "under1"],
                &["--level-reach", "--bigfile", "BIGFILE.DAT", "--start", "under1"],
            ),
            (
                &["level", "reach", "--bigfile", "BIGFILE.DAT"],
                &["--level-reach", "--bigfile", "BIGFILE.DAT"],
            ),
        ];
        for (input, expected) in cases {
            assert_dispatch(input, expected);
        }
    }

    #[test]
    fn options_reemit_in_canonical_order() {
        assert_dispatch(
            &["texture", "list", "T.BIG", "--map", "T.MAP"],
            &["--textures", "T.BIG", "--tlist", "--map", "T.MAP"],
        );
        assert_dispatch(
            &["texture", "list", "--map", "T.MAP", "T.BIG"],
            &["--textures", "T.BIG", "--tlist", "--map", "T.MAP"],
        );
        assert_dispatch(
            &["mesh", "show", "hero.SRM", "--pose", "3:90"],
            &["--mesh", "hero.SRM", "--pose", "3:90"],
        );
    }

    #[test]
    fn legacy_forms_pass_through_unchanged() {
        let cases: &[&[&str]] = &[
            &[],
            &["--textures", "T.BIG", "--tlist"],
            &["--mesh", "hero.SRM", "--pose", "3:90"],
            &["hero.DDS"],
            &["texdir"],
            &["--bogus"],
            &["foo", "bar"],
            &["./mesh"],
        ];
        for case in cases {
            assert_eq!(dispatch(case), argv(case), "case {case:?}");
        }
    }

    #[test]
    fn help_is_routed_before_groups() {
        assert_eq!(route(&argv(&["--help"])), Route::Help(None));
        assert_eq!(route(&argv(&["-h"])), Route::Help(None));
        assert_eq!(
            route(&argv(&["help", "level"])),
            Route::Help(Some("level".to_string()))
        );
    }

    #[test]
    fn known_group_with_unknown_or_missing_command_fails_loud() {
        let unknown = error(&["texture", "bogus"]);
        assert!(unknown.contains("unknown command 'texture bogus'"), "{unknown}");
        assert!(unknown.contains("list, show, file, gallery"), "{unknown}");
        assert!(unknown.contains("known groups:"), "{unknown}");

        let missing = error(&["level"]);
        assert!(missing.contains("requires a command"), "{missing}");
        assert!(missing.contains("info, records, structure, links"), "{missing}");
    }

    #[test]
    fn menu_requires_an_interactive_stdout_and_is_a_reserved_word() {
        assert_eq!(route_with_terminal(&[], true), Route::Menu);
        assert_eq!(route_with_terminal(&[], false), Route::Dispatch(Vec::new()));
        assert_eq!(route_with_terminal(&argv(&["menu"]), false), Route::Menu);
        assert_eq!(route(&argv(&["menu"])), Route::Menu);
    }

    #[test]
    fn malformed_subcommands_are_rejected() {
        for case in [
            &["texture", "list"][..],
            &["texture", "list", "T.BIG", "EXTRA"][..],
            &["texture", "list", "T.BIG", "--bogus", "x"][..],
            &["texture", "list", "T.BIG", "--map"][..],
        ] {
            let message = error(case);
            assert!(
                message.contains("missing operand")
                    || message.contains("unexpected argument")
                    || message.contains("unknown option")
                    || message.contains("needs a value"),
                "case {case:?}: {message}"
            );
        }
    }
}
