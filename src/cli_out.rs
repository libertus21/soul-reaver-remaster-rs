//! Broken-pipe-safe stdout output for the CLI.
//!
//! `println!` panics when the downstream reader closes the pipe (e.g.
//! `... | head -5`), which prints a localized panic message and exits
//! with 101. Every CLI stdout line goes through [`print_line`] instead:
//! a closed pipe ends the process cleanly with exit 0, while any other
//! write error is still reported loud on stderr.

use std::io::{self, Write};

/// True when `e` means the downstream reader closed the pipe.
///
/// `ErrorKind::BrokenPipe` is the portable signal. On Windows the same
/// condition surfaces as raw OS error 232 (`ERROR_NO_DATA`) with a
/// localized message, so the numeric code is accepted as well.
pub fn is_broken_pipe(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::BrokenPipe || e.raw_os_error() == Some(232)
}

/// Print one formatted line to stdout.
///
/// On a broken pipe, terminate the process with exit code 0 and no
/// message. Any other write error is reported on stderr and exits 1.
pub fn print_line(args: std::fmt::Arguments<'_>) {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let result = out
        .write_fmt(args)
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush());
    match result {
        Ok(()) => {}
        Err(e) if is_broken_pipe(&e) => std::process::exit(0),
        Err(e) => {
            eprintln!("error: failed writing to stdout: {e}");
            std::process::exit(1);
        }
    }
}

macro_rules! outln {
    ($($arg:tt)*) => {
        crate::cli_out::print_line(format_args!($($arg)*))
    };
}

pub(crate) use outln;

#[cfg(test)]
mod tests {
    use super::is_broken_pipe;
    use std::io;

    #[test]
    fn broken_pipe_kind_is_detected() {
        let e = io::Error::from(io::ErrorKind::BrokenPipe);
        assert!(is_broken_pipe(&e));
    }

    #[test]
    fn raw_windows_232_is_detected() {
        let e = io::Error::from_raw_os_error(232);
        assert!(is_broken_pipe(&e));
    }

    #[test]
    fn unrelated_errors_are_not_broken_pipe() {
        let disk_full = io::Error::from_raw_os_error(28);
        assert!(!is_broken_pipe(&disk_full));

        let denied = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(!is_broken_pipe(&denied));
    }
}
