//! `--play` option parsing and the engine entry point.
//!
//! Parsing is pure: it never touches the filesystem and never constructs an
//! engine. This slice stops at the facade; [`run`] reports a typed
//! "not implemented" error until T9 (windowed) and T10 (headless) land.

use std::fmt;
use std::path::PathBuf;

/// Camera modes accepted by `--camera`.
///
/// The discriminants are explicit and `#[repr(u8)]` so the value hashed by
/// `determinism::write_options` cannot drift when a variant is added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CameraMode {
    Orbit = 0,
    Fly = 1,
    Player = 2,
}

impl CameraMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "orbit" => Some(Self::Orbit),
            "fly" => Some(Self::Fly),
            "player" => Some(Self::Player),
            _ => None,
        }
    }
}

/// Parsed `--play` options.
///
/// `ticks` is resolved during parsing: headless defaults to 60, while `None`
/// in windowed mode means no frame cap. The fields are consumed by the
/// world, clock and renderer slices; T1 only parses them.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayOptions {
    pub bigfile: PathBuf,
    pub textures: Option<PathBuf>,
    pub unit: String,
    pub ticks: Option<u32>,
    pub headless: bool,
    pub camera: CameraMode,
}

impl PlayOptions {
    pub const DEFAULT_UNIT: &'static str = "under1";

    /// Parse the argv that follows `--play`.
    ///
    /// Pure by construction: unknown flags, missing values, a missing
    /// `--bigfile` and malformed `--ticks`/`--camera` values are typed errors.
    pub fn parse(args: &[String]) -> Result<Self, PlayError> {
        let mut options = Self {
            bigfile: PathBuf::new(),
            textures: None,
            unit: Self::DEFAULT_UNIT.to_string(),
            ticks: None,
            headless: false,
            camera: CameraMode::Fly,
        };
        let mut bigfile = None;
        let mut index = 0;
        while index < args.len() {
            let token = args[index].as_str();
            match token {
                "--bigfile" => bigfile = Some(PathBuf::from(value(args, index, "--bigfile")?)),
                "--textures" => {
                    options.textures = Some(PathBuf::from(value(args, index, "--textures")?));
                }
                "--unit" => options.unit = value(args, index, "--unit")?.to_string(),
                "--ticks" => {
                    let raw = value(args, index, "--ticks")?;
                    options.ticks = Some(
                        raw.parse::<u32>()
                            .map_err(|_| PlayError::InvalidTicks(raw.to_string()))?,
                    );
                }
                "--camera" => {
                    let raw = value(args, index, "--camera")?;
                    options.camera = CameraMode::parse(raw)
                        .ok_or_else(|| PlayError::InvalidCamera(raw.to_string()))?;
                }
                "--headless" => {
                    options.headless = true;
                    index += 1;
                    continue;
                }
                other => return Err(PlayError::UnknownOption(other.to_string())),
            }
            index += 2;
        }
        let Some(bigfile) = bigfile else {
            return Err(PlayError::MissingArchive);
        };
        options.bigfile = bigfile;
        if options.headless {
            if options.ticks.is_none() {
                options.ticks = Some(60);
            }
            if options.ticks == Some(0) {
                return Err(PlayError::ZeroTicksHeadless);
            }
        }
        Ok(options)
    }
}

fn value<'a>(
    args: &'a [String],
    index: usize,
    option: &'static str,
) -> Result<&'a str, PlayError> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or(PlayError::MissingValue(option))
}

/// Typed `--play` failures. `main` prints each one and exits 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayError {
    UnknownOption(String),
    MissingValue(&'static str),
    MissingArchive,
    InvalidTicks(String),
    ZeroTicksHeadless,
    InvalidCamera(String),
    NotImplemented,
}

impl fmt::Display for PlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOption(option) => write!(f, "unknown option '{option}' for --play"),
            Self::MissingValue(option) => write!(f, "option '{option}' needs a value"),
            Self::MissingArchive => write!(f, "--play requires --bigfile <BIGFILE.DAT>"),
            Self::InvalidTicks(value) => write!(
                f,
                "invalid value '{value}' for --ticks: expected a non-negative integer"
            ),
            Self::ZeroTicksHeadless => write!(
                f,
                "invalid value '0' for --ticks: expected a positive integer in headless mode"
            ),
            Self::InvalidCamera(value) => write!(
                f,
                "invalid value '{value}' for --camera: expected orbit, fly or player"
            ),
            Self::NotImplemented => write!(
                f,
                "--play is not implemented until T9 (windowed) and T10 (headless)"
            ),
        }
    }
}

impl std::error::Error for PlayError {}

/// Engine entry point.
///
/// T1 has no world, clock or renderer, so this is a loud typed error; it must
/// never turn into a silent success.
pub fn run(_options: &PlayOptions) -> Result<(), PlayError> {
    Err(PlayError::NotImplemented)
}

#[cfg(test)]
mod tests {
    use super::{run, CameraMode, PlayError, PlayOptions};
    use std::path::PathBuf;

    fn argv(input: &[&str]) -> Vec<String> {
        input.iter().map(|token| token.to_string()).collect()
    }

    fn parse(input: &[&str]) -> Result<PlayOptions, PlayError> {
        PlayOptions::parse(&argv(input))
    }

    #[test]
    fn defaults_fill_unit_camera_and_ticks() {
        let options = parse(&["--bigfile", "BIGFILE.DAT"]).expect("defaults must parse");
        assert_eq!(options.bigfile, PathBuf::from("BIGFILE.DAT"));
        assert_eq!(options.textures, None);
        assert_eq!(options.unit, "under1");
        assert_eq!(options.ticks, None);
        assert!(!options.headless);
        assert_eq!(options.camera, CameraMode::Fly);
    }

    #[test]
    fn every_option_is_parsed() {
        let options = parse(&[
            "--bigfile",
            "BIGFILE.DAT",
            "--textures",
            "TEXTURES.BIG",
            "--unit",
            "under2",
            "--ticks",
            "120",
            "--headless",
            "--camera",
            "player",
        ])
        .expect("full argv must parse");
        assert_eq!(options.bigfile, PathBuf::from("BIGFILE.DAT"));
        assert_eq!(options.textures, Some(PathBuf::from("TEXTURES.BIG")));
        assert_eq!(options.unit, "under2");
        assert_eq!(options.ticks, Some(120));
        assert!(options.headless);
        assert_eq!(options.camera, CameraMode::Player);
    }

    #[test]
    fn headless_defaults_to_sixty_ticks() {
        let options = parse(&["--bigfile", "B", "--headless"]).expect("headless must parse");
        assert_eq!(options.ticks, Some(60));
    }

    #[test]
    fn zero_ticks_is_allowed_in_windowed_mode() {
        let options = parse(&["--bigfile", "B", "--ticks", "0"]).expect("0 is unlimited");
        assert_eq!(options.ticks, Some(0));
        assert!(!options.headless);
    }

    #[test]
    fn unknown_flag_is_a_typed_error() {
        assert_eq!(
            parse(&["--bigfile", "B", "--bogus"]),
            Err(PlayError::UnknownOption("--bogus".to_string()))
        );
        assert_eq!(
            parse(&["--bigfile", "B", "--bogus"]).unwrap_err().to_string(),
            "unknown option '--bogus' for --play"
        );
    }

    #[test]
    fn missing_values_are_typed_errors() {
        for option in ["--bigfile", "--textures", "--unit", "--ticks", "--camera"] {
            assert_eq!(
                parse(&[option]),
                Err(PlayError::MissingValue(option)),
                "option {option}"
            );
        }
    }

    #[test]
    fn missing_archive_is_reported() {
        assert_eq!(parse(&[]), Err(PlayError::MissingArchive));
        assert_eq!(
            parse(&["--unit", "under1"]),
            Err(PlayError::MissingArchive)
        );
        assert_eq!(
            PlayError::MissingArchive.to_string(),
            "--play requires --bigfile <BIGFILE.DAT>"
        );
    }

    #[test]
    fn ticks_and_camera_validate() {
        assert_eq!(
            parse(&["--bigfile", "B", "--ticks", "abc"]),
            Err(PlayError::InvalidTicks("abc".to_string()))
        );
        assert_eq!(
            parse(&["--bigfile", "B", "--ticks", "-1"]),
            Err(PlayError::InvalidTicks("-1".to_string()))
        );
        assert_eq!(
            parse(&["--bigfile", "B", "--headless", "--ticks", "0"]),
            Err(PlayError::ZeroTicksHeadless)
        );
        assert_eq!(
            parse(&["--bigfile", "B", "--camera", "top"]),
            Err(PlayError::InvalidCamera("top".to_string()))
        );
    }

    #[test]
    fn run_is_loud_until_t9_t10() {
        let options = parse(&["--bigfile", "B"]).expect("minimal argv must parse");
        assert_eq!(run(&options), Err(PlayError::NotImplemented));
        let message = PlayError::NotImplemented.to_string();
        assert!(message.contains("T9"), "{message}");
        assert!(message.contains("T10"), "{message}");
    }
}
