//! Pure parser for the launcher CLI argument set discovered in `SRX.exe`
//! strings. Behavior notes only: every effect is `UNKNOWN`; this module
//! invents no semantics. No IO, no `std::env`, no process spawning.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Game {
    Sr1,
    Sr2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EgsEnv {
    Dev,
    Stage,
    Live,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LauncherArgs {
    pub nosteam: bool,
    pub noegs: bool,
    pub nolegal: bool,
    pub novsync: bool,
    pub dev: bool,
    pub ldpi: bool,
    pub nocheck: bool,
    pub skipintro: bool,
    pub egs_dev: bool,
    pub egs_stage: bool,
    pub egs_live: bool,
    pub egs_env: Option<EgsEnv>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub auth_password: Option<String>,
    pub epic_sandbox_id: Option<String>,
    pub epic_locale: Option<String>,
    pub epic_user_id: Option<String>,
    pub game: Option<Game>,
    pub unknown: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LauncherCliError {
    MissingValue(String),
    InvalidValue { flag: String, value: String },
}

impl fmt::Display for LauncherCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue(flag) => write!(f, "missing value for {flag}"),
            Self::InvalidValue { flag, value } => {
                write!(f, "invalid value for {flag}: {value}")
            }
        }
    }
}

impl std::error::Error for LauncherCliError {}

impl LauncherArgs {
    pub fn parse(args: &[String]) -> Result<Self, LauncherCliError> {
        parse(args)
    }
}

pub fn parse(args: &[String]) -> Result<LauncherArgs, LauncherCliError> {
    let mut out = LauncherArgs::default();
    let mut i = 0usize;
    while i < args.len() {
        let tok = &args[i];
        let (name, inline) = split_flag(tok);
        let lname = name.to_lowercase();

        // Value-taking flags.
        if is_value_flag(&lname) {
            let value = match inline {
                Some(v) => v,
                None => {
                    if i + 1 < args.len() {
                        i += 1;
                        args[i].clone()
                    } else {
                        return Err(LauncherCliError::MissingValue(flag_name(tok)));
                    }
                }
            };
            apply_value(&mut out, &lname, &value, tok)?;
            i += 1;
            continue;
        }

        // Boolean / selector flags.
        match lname.as_str() {
            "nosteam" => out.nosteam = true,
            "noegs" => out.noegs = true,
            "nolegal" => out.nolegal = true,
            "novsync" => out.novsync = true,
            "dev" => out.dev = true,
            "ldpi" => out.ldpi = true,
            "nocheck" => out.nocheck = true,
            "skipintro" => out.skipintro = true,
            "egs_dev" => {
                out.egs_dev = true;
                out.egs_env = Some(EgsEnv::Dev);
            }
            "egs_stage" => {
                out.egs_stage = true;
                out.egs_env = Some(EgsEnv::Stage);
            }
            "egs_live" => {
                out.egs_live = true;
                out.egs_env = Some(EgsEnv::Live);
            }
            "1" => out.game = Some(Game::Sr1),
            "2" => out.game = Some(Game::Sr2),
            _ => out.unknown.push(tok.clone()),
        }
        i += 1;
    }
    Ok(out)
}

fn is_value_flag(name: &str) -> bool {
    matches!(
        name,
        "width" | "height" | "auth_password" | "epicsandboxid" | "epiclocale" | "epicuserid"
    )
}

fn apply_value(
    out: &mut LauncherArgs,
    name: &str,
    value: &str,
    original: &str,
) -> Result<(), LauncherCliError> {
    let flag = flag_name(original);
    match name {
        "width" => {
            let n: u32 = value.parse().map_err(|_| LauncherCliError::InvalidValue {
                flag: flag.clone(),
                value: value.to_string(),
            })?;
            out.width = Some(n);
        }
        "height" => {
            let n: u32 = value.parse().map_err(|_| LauncherCliError::InvalidValue {
                flag: flag.clone(),
                value: value.to_string(),
            })?;
            out.height = Some(n);
        }
        "auth_password" => out.auth_password = Some(value.to_string()),
        "epicsandboxid" => out.epic_sandbox_id = Some(value.to_string()),
        "epiclocale" => out.epic_locale = Some(value.to_string()),
        "epicuserid" => out.epic_user_id = Some(value.to_string()),
        _ => {}
    }
    Ok(())
}

fn split_flag(tok: &str) -> (String, Option<String>) {
    let stripped = tok.trim_start_matches('-');
    // Collapse a leading second dash (--) into one for matching.
    let stripped = stripped.trim_start_matches('-');
    if let Some(eq) = stripped.find('=') {
        (stripped[..eq].to_string(), Some(stripped[eq + 1..].to_string()))
    } else {
        (stripped.to_string(), None)
    }
}

fn flag_name(tok: &str) -> String {
    // Preserve a leading dash for error reporting.
    let t = tok.trim_start_matches('-');
    let t = t.trim_start_matches('-');
    if let Some(eq) = t.find('=') {
        format!("-{}", &t[..eq])
    } else {
        // Re-add dash if original had one.
        if tok.starts_with('-') {
            // Use first dash group from original.
            let dashes = if tok.starts_with("--") { "--" } else { "-" };
            format!("{dashes}{}", t)
        } else {
            t.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn no_op_flags() {
        let a = parse(&s(&["--nosteam", "-nocheck", "-skipintro"])).unwrap();
        assert!(a.nosteam);
        assert!(a.nocheck);
        assert!(a.skipintro);
        assert!(a.unknown.is_empty());
    }

    #[test]
    fn value_flags_space() {
        let a = parse(&s(&["-nosteam", "-width", "1920"])).unwrap();
        assert!(a.nosteam);
        assert_eq!(a.width, Some(1920));
        assert!(a.unknown.is_empty());
    }

    #[test]
    fn value_flags_equals() {
        let a = parse(&s(&["-AUTH_PASSWORD=secret", "-width=800"])).unwrap();
        assert_eq!(a.auth_password.as_deref(), Some("secret"));
        assert_eq!(a.width, Some(800));
    }

    #[test]
    fn nocheck_skipintro_both_true() {
        let a = parse(&s(&["-nocheck", "-skipintro"])).unwrap();
        assert!(a.nocheck);
        assert!(a.skipintro);
    }

    #[test]
    fn missing_value() {
        let e = parse(&s(&["-width"])).unwrap_err();
        assert!(matches!(e, LauncherCliError::MissingValue(_)));
    }

    #[test]
    fn invalid_value() {
        let e = parse(&s(&["-width", "abc"])).unwrap_err();
        match e {
            LauncherCliError::InvalidValue { flag, value } => {
                assert_eq!(value, "abc");
                assert!(flag.contains("width"));
            }
            _ => panic!("expected InvalidValue"),
        }
    }

    #[test]
    fn unknown_token() {
        let a = parse(&s(&["-foo"])).unwrap();
        assert_eq!(a.unknown, vec!["-foo".to_string()]);
    }

    #[test]
    fn game_selection() {
        let a = parse(&s(&["-1"])).unwrap();
        assert_eq!(a.game, Some(Game::Sr1));
        let b = parse(&s(&["-2"])).unwrap();
        assert_eq!(b.game, Some(Game::Sr2));
    }

    #[test]
    fn egs_env() {
        let a = parse(&s(&["-egs_stage"])).unwrap();
        assert!(a.egs_stage);
        assert_eq!(a.egs_env, Some(EgsEnv::Stage));
    }

    #[test]
    fn mixed_argv() {
        let a = parse(&s(&["-dev", "-width", "1920", "-height", "1080", "-foo", "-novsync"])).unwrap();
        assert!(a.dev);
        assert_eq!(a.width, Some(1920));
        assert_eq!(a.height, Some(1080));
        assert!(a.novsync);
        assert_eq!(a.unknown, vec!["-foo".to_string()]);
    }

    #[test]
    fn launcher_args_parse_assoc() {
        let a = LauncherArgs::parse(&s(&["-ldpi"])).unwrap();
        assert!(a.ldpi);
    }

    #[test]
    fn pure_no_io() {
        // Compile-time-ish sanity: parse is callable with empty input.
        let a = parse(&[]).unwrap();
        assert!(a.unknown.is_empty());
    }
}