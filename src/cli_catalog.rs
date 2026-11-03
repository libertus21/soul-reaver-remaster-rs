//! Static capability catalog for the grouped CLI.
//!
//! This module is data only: it describes every existing mode, the legacy
//! argv shape it maps to, and whether it opens a window. It performs no
//! filesystem access and no execution, so the help surface is unit-testable
//! without a terminal, a process, or game bytes.

/// One existing capability. `legacy` is the argv shape `main` already
/// accepts; `window` says whether the handler opens a window or only prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    pub group: &'static str,
    pub name: &'static str,
    pub legacy: &'static str,
    pub summary: &'static str,
    pub window: bool,
}

/// Display order for the grouped help.
pub const GROUPS: [&str; 5] = ["texture", "archive", "mesh", "audio", "level"];

/// Every existing mode, exactly once. This is the single source for help.
pub const CAPABILITIES: [Capability; 15] = [
    Capability { group: "texture", name: "list", legacy: "--textures <BIG> --tlist [--map <MAP>]", summary: "List the remaster .BIG header and slot classes.", window: false },
    Capability { group: "texture", name: "show", legacy: "--textures <BIG> --tshow <SEL> [--map <MAP>]", summary: "Show one slot as its raw 256x256 image.", window: true },
    Capability { group: "texture", name: "file", legacy: "<FILE.DDS>", summary: "Show one .DDS texture file.", window: true },
    Capability { group: "texture", name: "gallery", legacy: "<DIR>", summary: "Browse a directory of .DDS files.", window: true },
    Capability { group: "archive", name: "list", legacy: "--bigfile <DAT> --list", summary: "Print the archive index, one line per entry.", window: false },
    Capability { group: "archive", name: "show", legacy: "--bigfile <DAT> --show <SEL>", summary: "Show one archived DDS entry from memory.", window: true },
    Capability { group: "mesh", name: "show", legacy: "--mesh <SRM> [--pose <BONE>:<DEG>]", summary: "Show one .SRM mesh as a flat-shaded solid.", window: true },
    Capability { group: "mesh", name: "textured", legacy: "--mesh-textured <SRM> --tex-dir <TEX>", summary: "Show one .SRM with its material textures.", window: true },
    Capability { group: "mesh", name: "gallery", legacy: "--mesh-all <DIR> --tex-dir <TEX>", summary: "Browse a directory of .SRM meshes.", window: true },
    Capability { group: "audio", name: "list", legacy: "--audio <SND> --alist", summary: "List the waves in a .SND bank.", window: false },
    Capability { group: "audio", name: "wave", legacy: "--audio <SND> --wave <N> --out <OUT>", summary: "Write one .SND wave to a .wav file.", window: false },
    Capability { group: "level", name: "info", legacy: "--level-info <PATH>", summary: "Print the level header summary.", window: false },
    Capability { group: "level", name: "records", legacy: "--level-records <PATH>", summary: "Print the per-unit block census.", window: false },
    Capability { group: "level", name: "structure", legacy: "--level-structure <PATH>", summary: "Print header and relocation-directory integrity.", window: false },
    Capability { group: "level", name: "links", legacy: "--level-links <PATH>", summary: "Print the decoded link table.", window: false },
];

/// The capability table.
pub fn catalog() -> &'static [Capability] {
    &CAPABILITIES
}

/// Whether `group` is one of [`GROUPS`].
pub fn is_known_group(group: &str) -> bool {
    GROUPS.contains(&group)
}

/// Render grouped help. `None` renders every group; an unknown group renders
/// only an error line so callers can tell it apart and exit non-zero.
pub fn render_help(group: Option<&str>) -> String {
    if let Some(group) = group {
        if !is_known_group(group) {
            return format!(
                "error: unknown group '{group}'\nknown groups: {}",
                GROUPS.join(", ")
            );
        }
    }
    let mut text = String::from("soul-reaver-remaster-rs capabilities\n\n");
    text.push_str("usage: soul-reaver-remaster-rs --help [group]\n\n");
    for name in GROUPS {
        if group.is_some_and(|wanted| wanted != name) {
            continue;
        }
        text.push_str(&format!("[{name}]\n"));
        for cap in catalog().iter().filter(|cap| cap.group == name) {
            let window = if cap.window { " (opens a window)" } else { "" };
            text.push_str(&format!(
                "  {} {} -> {} - {}{}\n",
                cap.group, cap.name, cap.legacy, cap.summary, window
            ));
        }
        text.push('\n');
    }
    text.push_str("Every legacy flag form is still accepted unchanged.\n");
    text
}

#[cfg(test)]
mod tests {
    use super::{catalog, is_known_group, render_help, GROUPS};

    const EXPECTED: [(&str, &str); 15] = [
        ("texture", "list"),
        ("texture", "show"),
        ("texture", "file"),
        ("texture", "gallery"),
        ("archive", "list"),
        ("archive", "show"),
        ("mesh", "show"),
        ("mesh", "textured"),
        ("mesh", "gallery"),
        ("audio", "list"),
        ("audio", "wave"),
        ("level", "info"),
        ("level", "records"),
        ("level", "structure"),
        ("level", "links"),
    ];

    #[test]
    fn every_existing_mode_appears_exactly_once() {
        let caps = catalog();
        assert_eq!(caps.len(), EXPECTED.len(), "catalog size changed");
        for cap in caps {
            assert!(
                GROUPS.contains(&cap.group),
                "ungrouped capability: {}/{}",
                cap.group,
                cap.name
            );
        }
        for expected in EXPECTED {
            let count = caps
                .iter()
                .filter(|cap| (cap.group, cap.name) == expected)
                .count();
            assert_eq!(count, 1, "expected exactly one {expected:?}");
        }
    }

    #[test]
    fn help_without_group_lists_every_group_and_mode() {
        let text = render_help(None);
        for group in GROUPS {
            assert!(text.contains(&format!("[{group}]")), "missing group {group}");
        }
        for cap in catalog() {
            assert!(
                text.contains(&format!("{} {} ->", cap.group, cap.name)),
                "missing capability {}/{}",
                cap.group,
                cap.name
            );
        }
    }

    #[test]
    fn group_help_contains_only_that_group() {
        let text = render_help(Some("level"));
        assert!(text.contains("[level]"));
        for cap in catalog() {
            let line = format!("{} {} ->", cap.group, cap.name);
            if cap.group == "level" {
                assert!(text.contains(&line), "missing {line}");
            } else {
                assert!(!text.contains(&line), "leaked {line}");
            }
        }
        for other in GROUPS.into_iter().filter(|group| *group != "level") {
            assert!(!text.contains(&format!("[{other}]")), "leaked group {other}");
        }
    }

    #[test]
    fn unknown_group_is_a_distinguishable_error() {
        let text = render_help(Some("bogus"));
        assert!(text.starts_with("error: unknown group 'bogus'"));
        assert!(text.contains("known groups: texture, archive, mesh, audio, level"));
        assert!(!is_known_group("bogus"));
        assert_ne!(text, render_help(Some("level")));
        assert_ne!(text, render_help(None));
    }
}
