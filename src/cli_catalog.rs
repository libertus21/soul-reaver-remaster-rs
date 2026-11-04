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

impl Capability {
    /// Whether this capability emits `token` as a named option with a value
    /// instead of a literal token followed by its operand placeholder.
    pub fn takes_value(&self, token: &str) -> bool {
        if token == "--textures" {
            return matches!(
                self.name,
                "object-mesh" | "render-objects" | "render-entry" | "render-group"
            );
        }
        matches!(token, "--map" | "--pose" | "--tex-dir" | "--out")
            || ((self.name.ends_with("-entry") || self.name == "object-info" || self.name == "object-mesh")
                && matches!(token, "--bigfile" | "--entry"))
            || (self.name.ends_with("-group")
                && matches!(token, "--bigfile" | "--group"))
    }
}

/// Display order for the grouped help.
pub const GROUPS: [&str; 5] = ["texture", "archive", "mesh", "audio", "level"];

/// Every existing mode, exactly once. This is the single source for help.
pub const CAPABILITIES: [Capability; 27] = [
    Capability { group: "texture", name: "list", legacy: "--textures <BIG> --tlist [--map <MAP>]", summary: "List the remaster .BIG header and slot classes.", window: false },
    Capability { group: "texture", name: "show", legacy: "--textures <BIG> --tshow <SEL> [--map <MAP>]", summary: "Show one slot as its raw 256x256 image.", window: true },
    Capability { group: "texture", name: "file", legacy: "<FILE.DDS>", summary: "Show one .DDS texture file.", window: true },
    Capability { group: "texture", name: "gallery", legacy: "<DIR>", summary: "Browse a directory of .DDS files.", window: true },
    Capability { group: "archive", name: "list", legacy: "--bigfile <DAT> --list", summary: "Print the archive index, one line per entry.", window: false },
    Capability { group: "archive", name: "show", legacy: "--bigfile <DAT> --show <SEL>", summary: "Show one archived DDS entry from memory.", window: true },
    Capability { group: "mesh", name: "show", legacy: "--mesh <SRM> [--pose <BONE>:<DEG>]", summary: "Show one .SRM mesh as a flat-shaded solid.", window: true },
    Capability { group: "mesh", name: "textured", legacy: "--mesh-textured <SRM> --tex-dir <TEX>", summary: "Show one .SRM with its material textures.", window: true },
    Capability { group: "mesh", name: "gallery", legacy: "--mesh-all <DIR> --tex-dir <TEX>", summary: "Browse a directory of .SRM meshes.", window: true },
    Capability { group: "mesh", name: "object-info", legacy: "--object-info --bigfile <DAT> --entry <INDEX>", summary: "Print one archived object container's counts and validation.", window: false },
    Capability { group: "mesh", name: "object-mesh", legacy: "--object-mesh --bigfile <DAT> --entry <INDEX> [--textures <BIG>]", summary: "Show one archived object container's model.", window: true },
    Capability { group: "audio", name: "list", legacy: "--audio <SND> --alist", summary: "List the waves in a .SND bank.", window: false },
    Capability { group: "audio", name: "wave", legacy: "--audio <SND> --wave <N> --out <OUT>", summary: "Write one .SND wave to a .wav file.", window: false },
    Capability { group: "level", name: "info", legacy: "--level-info <PATH>", summary: "Print the level header summary.", window: false },
    Capability { group: "level", name: "records", legacy: "--level-records <PATH>", summary: "Print the per-unit block census.", window: false },
    Capability { group: "level", name: "structure", legacy: "--level-structure <PATH>", summary: "Print header and relocation-directory integrity.", window: false },
    Capability { group: "level", name: "links", legacy: "--level-links <PATH>", summary: "Print the decoded link table.", window: false },
    Capability { group: "level", name: "geometry", legacy: "--level-geometry <PATH>", summary: "Print room vertex and face counts with a box.", window: false },
    Capability { group: "level", name: "objects", legacy: "--level-objects <PATH>", summary: "Print one unit's placed objects: name, position, type and flags.", window: false },
    Capability { group: "level", name: "render", legacy: "--level-render <PATH> [--textures <BIG>]", summary: "Show one unit's room geometry, flat or textured.", window: true },
    Capability { group: "level", name: "render-objects", legacy: "--level-render <PATH> --objects [--textures <BIG>]", summary: "Show one unit's room geometry plus one cube per object.", window: true },
    Capability { group: "level", name: "geometry-entry", legacy: "--level-geometry --bigfile <DAT> --entry <INDEX>", summary: "Print one archive entry's room vertex and face counts.", window: false },
    Capability { group: "level", name: "objects-entry", legacy: "--level-objects --bigfile <DAT> --entry <INDEX>", summary: "Print one archive entry's placed objects.", window: false },
    Capability { group: "level", name: "render-entry", legacy: "--level-render --bigfile <DAT> --entry <INDEX> [--textures <BIG>]", summary: "Show one archive entry's room geometry.", window: true },
    Capability { group: "level", name: "geometry-group", legacy: "--level-geometry --bigfile <DAT> --group <PREFIX>", summary: "Print every room in an archive level family with totals.", window: false },
    Capability { group: "level", name: "objects-group", legacy: "--level-objects --bigfile <DAT> --group <PREFIX>", summary: "Print every room's objects in an archive level family with a total.", window: false },
    Capability { group: "level", name: "render-group", legacy: "--level-render --bigfile <DAT> --group <PREFIX> [--textures <BIG>]", summary: "Show a merged archive level family as one solid.", window: true },
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

    const EXPECTED: [(&str, &str); 27] = [
        ("texture", "list"),
        ("texture", "show"),
        ("texture", "file"),
        ("texture", "gallery"),
        ("archive", "list"),
        ("archive", "show"),
        ("mesh", "show"),
        ("mesh", "textured"),
        ("mesh", "gallery"),
        ("mesh", "object-info"),
        ("mesh", "object-mesh"),
        ("audio", "list"),
        ("audio", "wave"),
        ("level", "info"),
        ("level", "records"),
        ("level", "structure"),
        ("level", "links"),
        ("level", "geometry"),
        ("level", "objects"),
        ("level", "render"),
        ("level", "render-objects"),
        ("level", "geometry-entry"),
        ("level", "objects-entry"),
        ("level", "render-entry"),
        ("level", "geometry-group"),
        ("level", "objects-group"),
        ("level", "render-group"),
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
