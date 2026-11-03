//! Interactive terminal picker: pure over injected IO. It lists capabilities
//! from `cli_catalog` and returns the canonical argv the router would
//! dispatch. It never touches the filesystem and never dispatches.
use std::io::{BufRead, Write};
use crate::cli_argv::{self, Route};
use crate::cli_catalog::{self, Capability};
const MAX_INVALID: usize = 3;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome { Dispatch(Vec<String>), Cancelled, Exhausted }
enum Stop { Cancel, Fail }
type Picked<T> = std::io::Result<Result<T, Stop>>;

pub fn run(reader: &mut impl BufRead, out: &mut impl Write) -> std::io::Result<Outcome> {
    writeln!(out, "soul-reaver-remaster-rs interactive menu (q to quit)")?;
    let mut invalid = 0;
    let group = match select(reader, out, "Groups:", &cli_catalog::GROUPS, &mut invalid)? {
        Ok(index) => cli_catalog::GROUPS[index],
        Err(Stop::Cancel) => return Ok(Outcome::Cancelled),
        Err(Stop::Fail) => return Ok(Outcome::Exhausted),
    };
    let caps: Vec<&Capability> = cli_catalog::catalog().iter().filter(|c| c.group == group).collect();
    let names: Vec<&str> = caps.iter().map(|c| c.name).collect();
    let cap = match select(reader, out, "Commands:", &names, &mut invalid)? {
        Ok(index) => caps[index],
        Err(Stop::Cancel) => return Ok(Outcome::Cancelled),
        Err(Stop::Fail) => return Ok(Outcome::Exhausted),
    };
    let mut tokens = vec![cap.group.to_string(), cap.name.to_string()];
    for (label, flag) in prompts(cap) {
        let value = match ask(reader, out, &format!("{label}> "), &mut invalid)? {
            Ok(value) => value,
            Err(Stop::Cancel) => return Ok(Outcome::Cancelled),
            Err(Stop::Fail) => return Ok(Outcome::Exhausted),
        };
        if let Some(flag) = flag { tokens.push(flag.to_string()); }
        tokens.push(value);
    }
    match cli_argv::route(&tokens) {
        Route::Dispatch(argv) => Ok(Outcome::Dispatch(argv)),
        _ => Ok(Outcome::Exhausted),
    }
}

fn select(reader: &mut impl BufRead, out: &mut impl Write, title: &str,
    items: &[&str], invalid: &mut usize) -> Picked<usize> {
    loop {
        writeln!(out, "\n{title}")?;
        for (index, item) in items.iter().enumerate() {
            writeln!(out, "  [{}] {item}", index + 1)?;
        }
        let Some(line) = prompt(reader, out, "select> ")? else { return Ok(Err(Stop::Cancel)) };
        if line.eq_ignore_ascii_case("q") || line.eq_ignore_ascii_case("quit") {
            return Ok(Err(Stop::Cancel));
        }
        let found = items.iter().position(|item| *item == line.as_str()).or_else(|| {
            line.parse::<usize>().ok().filter(|i| (1..=items.len()).contains(i)).map(|i| i - 1)
        });
        if let Some(index) = found { return Ok(Ok(index)); }
        writeln!(out, "error: unknown selection '{line}'")?;
        if bump(invalid, out)? { return Ok(Err(Stop::Fail)); }
    }
}

fn ask(reader: &mut impl BufRead, out: &mut impl Write,
    label: &str, invalid: &mut usize) -> Picked<String> {
    loop {
        let Some(value) = prompt(reader, out, label)? else { return Ok(Err(Stop::Cancel)) };
        if value.eq_ignore_ascii_case("q") || value.eq_ignore_ascii_case("quit") {
            return Ok(Err(Stop::Cancel));
        }
        if !value.is_empty() && !value.starts_with("--") { return Ok(Ok(value)); }
        writeln!(out, "error: this value is required")?;
        if bump(invalid, out)? { return Ok(Err(Stop::Fail)); }
    }
}

fn prompt(reader: &mut impl BufRead, out: &mut impl Write,
    text: &str) -> std::io::Result<Option<String>> {
    write!(out, "{text}")?;
    out.flush()?;
    let mut line = String::new();
    Ok((reader.read_line(&mut line)? > 0).then(|| line.trim().to_string()))
}

fn bump(invalid: &mut usize, out: &mut impl Write) -> std::io::Result<bool> {
    *invalid += 1;
    if *invalid < MAX_INVALID { return Ok(false); }
    writeln!(out, "error: too many invalid inputs; exiting")?;
    Ok(true)
}

/// Required operands of the legacy shape, in prompt order. Optional `[...]`
/// groups stay a legacy-flag surface and are not prompted.
fn prompts(cap: &Capability) -> Vec<(String, Option<&'static str>)> {
    let tokens: Vec<&str> = cap.legacy.split_whitespace()
        .filter(|token| !token.starts_with('[') && !token.ends_with(']')).collect();
    let mut prompts = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if cap.takes_value(tokens[i]) {
            if let Some(value) = tokens.get(i + 1) { prompts.push((angle(value), Some(tokens[i]))); }
            i += 2;
        } else if tokens[i].starts_with('<') {
            prompts.push((angle(tokens[i]), None));
            i += 1;
        } else { i += 1; }
    }
    prompts
}

fn angle(token: &str) -> String { token.replace('<', "").replace('>', "") }

#[cfg(test)]
mod tests {
    use super::{run, Outcome};
    use crate::cli_argv::{route, Route};

    fn drive(input: &str) -> (Outcome, String) {
        let mut reader = std::io::Cursor::new(input.as_bytes());
        let mut out = Vec::new();
        let outcome = run(&mut reader, &mut out).expect("in-memory IO cannot fail");
        (outcome, String::from_utf8(out).expect("menu writes UTF-8"))
    }

    fn canonical(group: &str, name: &str, operands: &[&str]) -> Vec<String> {
        let mut tokens = vec![group.to_string(), name.to_string()];
        tokens.extend(operands.iter().map(|value| value.to_string()));
        match route(&tokens) {
            Route::Dispatch(argv) => argv,
            other => panic!("router rejected {tokens:?}: {other:?}"),
        }
    }

    #[test]
    fn happy_path_returns_the_router_argv() {
        let (outcome, _) = drive("texture\nlist\nTEXTURES.BIG\n");
        assert_eq!(outcome, Outcome::Dispatch(canonical("texture", "list", &["TEXTURES.BIG"])));
    }

    #[test]
    fn invalid_input_re_prompts_then_accepts() {
        let (outcome, text) = drive("bogus\ntexture\nbogus\nlist\nTEXTURES.BIG\n");
        assert_eq!(outcome, Outcome::Dispatch(canonical("texture", "list", &["TEXTURES.BIG"])));
        assert!(text.contains("unknown selection 'bogus'"));
    }

    #[test]
    fn exhausted_retries_stop_without_panicking() {
        let (outcome, text) = drive("bogus\nbogus\nbogus\n");
        assert_eq!(outcome, Outcome::Exhausted);
        assert!(text.contains("too many invalid inputs"));
    }
}
