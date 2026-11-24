//! Read-only census of the placed-object `flags` word.
//!
//! `--flag-census` feeds one [`ObjectReport`] per archived unit into
//! [`FlagsCensus::record_unit`]; [`FlagsCensus::render`] turns the counts into
//! deterministic lines. Nothing here interprets a bit: the module reports which
//! bits exist, how many names use each, and whether a bit is constant inside a
//! name. Bit meanings stay UNKNOWN until a runtime trace settles them.

use std::collections::{BTreeMap, BTreeSet};

use crate::level::ObjectReport;

/// Per-name flag counts and the units that place the name.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct NameFlags {
    pub objects: usize,
    pub units: BTreeSet<String>,
    pub values: BTreeMap<u32, usize>,
}

/// Whole-archive aggregate over every placed record's `flags` word.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FlagsCensus {
    units: usize,
    objects: usize,
    values: BTreeMap<u32, usize>,
    names: BTreeMap<String, NameFlags>,
}

/// Whether `bit` is set on some values and clear on others.
fn bit_varies(values: &BTreeMap<u32, usize>, bit: u32) -> bool {
    let mut seen = values.keys().map(|value| value >> bit & 1 == 1);
    match seen.next() {
        Some(first) => seen.any(|other| other != first),
        None => false,
    }
}

impl FlagsCensus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one unit's placed records into the census. A unit that places no
    /// object is not counted, so `units` is the number of object-bearing units.
    pub fn record_unit(&mut self, unit: &str, report: &ObjectReport) {
        if report.objects.is_empty() {
            return;
        }
        self.units += 1;
        for object in &report.objects {
            self.objects += 1;
            *self.values.entry(object.flags).or_default() += 1;
            let entry = self.names.entry(object.name.clone()).or_default();
            entry.objects += 1;
            entry.units.insert(unit.to_string());
            *entry.values.entry(object.flags).or_default() += 1;
        }
    }

    /// Objects whose `flags` value has `bit` set.
    fn objects_with_bit(&self, bit: u32) -> usize {
        self.values
            .iter()
            .filter(|(value, _)| **value >> bit & 1 == 1)
            .map(|(_, count)| count)
            .sum()
    }

    /// Names where `bit` is set on at least one record.
    fn names_with_bit(&self, bit: u32) -> usize {
        self.names
            .values()
            .filter(|name| name.values.keys().any(|value| value >> bit & 1 == 1))
            .count()
    }

    /// Names where `bit` is set on some records and clear on others.
    fn names_varying_bit(&self, bit: u32) -> usize {
        self.names
            .values()
            .filter(|name| bit_varies(&name.values, bit))
            .count()
    }

    /// Deterministic census lines: header, distinct values, per-name flag sets,
    /// then per-bit totals and the names that vary inside a bit.
    pub fn render(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "flags-census units={} objects={} names={} distinct_flags={}",
            self.units,
            self.objects,
            self.names.len(),
            self.values.len()
        )];
        for (value, count) in &self.values {
            lines.push(format!("flags value={value:#010x} decimal={value} objects={count}"));
        }
        for (name, entry) in &self.names {
            let values: Vec<String> = entry
                .values
                .iter()
                .map(|(value, count)| format!("{value:#010x}:{count}"))
                .collect();
            lines.push(format!(
                "name name={name} objects={} units={} values={}",
                entry.objects,
                entry.units.len(),
                values.join(",")
            ));
        }
        for bit in 0..u32::BITS {
            let objects = self.objects_with_bit(bit);
            if objects == 0 {
                continue;
            }
            lines.push(format!(
                "bit bit={bit} mask={:#010x} objects={objects} names={} varied={}",
                1u32 << bit,
                self.names_with_bit(bit),
                self.names_varying_bit(bit)
            ));
        }
        for (name, entry) in &self.names {
            for bit in 0..u32::BITS {
                if !bit_varies(&entry.values, bit) {
                    continue;
                }
                let values: Vec<String> = entry
                    .values
                    .keys()
                    .map(|value| format!("{value:#010x}"))
                    .collect();
                lines.push(format!(
                    "bit-varying bit={bit} name={name} values={}",
                    values.join(",")
                ));
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::{FlagsCensus, NameFlags};
    use crate::level::{ObjectRecord, ObjectReport};

    fn report(records: &[(&str, u32)]) -> ObjectReport {
        ObjectReport {
            objects: records
                .iter()
                .map(|(name, flags)| ObjectRecord {
                    name: (*name).to_string(),
                    position: [0, 0, 0],
                    object_type: 0,
                    archetype: 0,
                    instance_id: 0,
                    flags: *flags,
                })
                .collect(),
            ..ObjectReport::default()
        }
    }

    fn census(units: &[(&str, &[(&str, u32)])]) -> FlagsCensus {
        let mut census = FlagsCensus::new();
        for (unit, records) in units {
            census.record_unit(unit, &report(records));
        }
        census
    }

    #[test]
    fn counts_units_objects_distinct_values_and_per_name_sets() {
        let census = census(&[
            ("under1", &[("portal", 0), ("campath", 0x10000)]),
            ("under2", &[("portal", 0x80), ("marker", 0x8000)]),
        ]);
        assert_eq!((census.units, census.objects), (2, 4));
        assert_eq!(
            census.values,
            [(0, 1), (0x80, 1), (0x8000, 1), (0x10000, 1)].into_iter().collect()
        );
        let portal = census.names.get("portal").expect("portal seen");
        assert_eq!(portal.objects, 2);
        assert_eq!(portal.units.len(), 2);
        assert_eq!(portal.values, [(0, 1), (0x80, 1)].into_iter().collect());
    }

    #[test]
    fn a_unit_without_objects_is_not_counted() {
        let mut census = FlagsCensus::new();
        census.record_unit("empty", &report(&[]));
        assert_eq!((census.units, census.objects), (0, 0));
        assert_eq!(census.names.len(), 0);
    }

    #[test]
    fn a_bit_that_varies_inside_a_name_is_reported() {
        let census = census(&[
            ("under1", &[("portal", 0), ("campath", 0x10000)]),
            ("under2", &[("portal", 0x80), ("marker", 0x8000)]),
        ]);
        let text = census.render();
        assert!(text[0].ends_with("units=2 objects=4 names=3 distinct_flags=4"), "{text:?}");
        assert!(text.contains(&"bit bit=7 mask=0x00000080 objects=1 names=1 varied=1".to_string()));
        assert!(text.contains(&"bit-varying bit=7 name=portal values=0x00000000,0x00000080".to_string()));
        assert!(!text.contains(&"bit-varying bit=7 name=campath".to_string()));
    }

    #[test]
    fn a_constant_bit_emits_no_varying_line() {
        let census = census(&[
            ("cathy1", &[("campath", 0x10000)]),
            ("cathy2", &[("campath", 0x10000)]),
        ]);
        let text = census.render();
        assert!(text.contains(&"bit bit=16 mask=0x00010000 objects=2 names=1 varied=0".to_string()));
        assert!(!text.iter().any(|line| line.starts_with("bit-varying")));
        let campath: &NameFlags = census.names.get("campath").expect("campath seen");
        assert_eq!(campath.units.len(), 2);
    }

    #[test]
    fn render_is_deterministic_and_sorted() {
        let census = census(&[
            ("b", &[("zeta", 8), ("alpha", 0)]),
            ("a", &[("alpha", 0x10000), ("zeta", 0)]),
        ]);
        let text = census.render();
        assert_eq!(text, census.render());
        let names: Vec<&String> =
            text.iter().filter(|line| line.starts_with("name ")).collect();
        assert!(names[0].contains("name=alpha"), "{names:?}");
        assert!(names[1].contains("name=zeta"), "{names:?}");
    }
}
