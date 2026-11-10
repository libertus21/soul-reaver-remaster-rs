//! Bone hierarchy parsed from the per-slot descriptor array (pure, no I/O).
//!
//! The object loader's 24-byte descriptor record carries a **signed parent bone
//! index** at `+18` (`-1` is a root). This is the only hierarchy source: the
//! 24-byte bone table's own `parent` word follows a different, legacy
//! convention and is not used here.
//!
//! Evidence (live body-model dump, 66 records): the parents form exactly one
//! root, no cycles, every parent index precedes its child (`parent < own`,
//! perfect topological order) and the maximum depth is 9. This supersedes the
//! earlier reading of `+18` as an opaque "slot parent" word.
//!
//! Contracts kept deliberately small:
//!
//! - [`BoneHierarchy`] is the raw parsed parents; [`BoneHierarchy::validate`]
//!   turns it into a [`Hierarchy`] and reports, but never repairs.
//! - Exactly one root (`-1`) is required. `0xFFFF` is `-1` as a signed `i16`,
//!   so the retired unsigned wording and this one agree byte for byte.
//! - A parent index outside `0..len`, a second root and a cycle are errors.
//! - Non-topological storage order is **reported** ([`Hierarchy::topological`])
//!   but not rejected: composition resolves by index, not by array order.
//!
//! No game bytes, offsets or addresses are recorded here; only the field role.

use std::fmt;

use crate::slot_descriptor::SlotDescriptor;

/// The parent value that marks a root bone.
pub const ROOT: i16 = -1;

/// Raw parent indices parsed from a descriptor array, in bone order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoneHierarchy {
    parents: Vec<i16>,
}

impl BoneHierarchy {
    /// Wrap an existing parent list. No validation is performed here.
    pub fn from_parents(parents: Vec<i16>) -> Self {
        Self { parents }
    }

    /// Read `+18` from every descriptor as a signed parent index.
    pub fn from_descriptors(descriptors: &[SlotDescriptor]) -> Self {
        Self {
            parents: descriptors.iter().map(SlotDescriptor::parent_index).collect(),
        }
    }

    /// The raw parents, in bone order.
    pub fn parents(&self) -> &[i16] {
        &self.parents
    }

    /// Number of bones the hierarchy covers.
    pub fn len(&self) -> usize {
        self.parents.len()
    }

    /// Whether no bone is described.
    pub fn is_empty(&self) -> bool {
        self.parents.is_empty()
    }

    /// Raw parent of `bone`, or `None` outside the hierarchy.
    pub fn parent_of(&self, bone: usize) -> Option<i16> {
        self.parents.get(bone).copied()
    }

    /// Check the structural invariants. See [`HierarchyError`] for refusals.
    pub fn validate(&self) -> Result<Hierarchy, HierarchyError> {
        if self.parents.is_empty() {
            return Err(HierarchyError::Empty);
        }
        let len = self.parents.len();
        let mut roots = Vec::new();
        for (bone, &parent) in self.parents.iter().enumerate() {
            if parent < ROOT || (parent != ROOT && parent as usize >= len) {
                return Err(HierarchyError::ParentOutOfRange { bone, parent });
            }
            if parent == ROOT {
                roots.push(bone);
            }
        }
        match roots.len() {
            0 => return Err(HierarchyError::NoRoot),
            1 => {}
            count => return Err(HierarchyError::MultipleRoots { roots: count }),
        }
        let root = roots[0];
        // A functional graph with one root is acyclic exactly when every chain
        // reaches that root. Walk each chain once; a repeated node inside one
        // walk is a cycle that never reaches the root.
        let mut reached = vec![false; len];
        reached[root] = true;
        let mut mark = vec![0u32; len];
        let mut walk = 0u32;
        for start in 0..len {
            if reached[start] {
                continue;
            }
            walk += 1;
            let mut path = Vec::new();
            let mut cursor = start;
            loop {
                if reached[cursor] {
                    break;
                }
                if mark[cursor] == walk {
                    let at = path.iter().position(|&node| node == cursor).unwrap_or(0);
                    return Err(HierarchyError::Cycle {
                        bones: path.len() - at,
                    });
                }
                mark[cursor] = walk;
                path.push(cursor);
                let parent = self.parents[cursor];
                if parent == ROOT {
                    break;
                }
                cursor = parent as usize;
            }
            for node in path {
                reached[node] = true;
            }
        }
        let depth = (0..len)
            .map(|bone| self.edges_to_root(bone))
            .max()
            .unwrap_or(0);
        let topological = self
            .parents
            .iter()
            .enumerate()
            .all(|(bone, &parent)| parent == ROOT || (parent as usize) < bone);
        Ok(Hierarchy {
            parents: self.parents.clone(),
            roots,
            depth,
            topological,
        })
    }

    /// Edges from `bone` up to the root. Only called after cycles are excluded.
    fn edges_to_root(&self, bone: usize) -> usize {
        let mut edges = 0;
        let mut cursor = bone;
        for _ in 0..self.parents.len() {
            let parent = self.parents[cursor];
            if parent == ROOT {
                break;
            }
            edges += 1;
            cursor = parent as usize;
        }
        edges
    }
}

/// A validated hierarchy: exactly one root, in-range parents and no cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hierarchy {
    parents: Vec<i16>,
    roots: Vec<usize>,
    depth: usize,
    topological: bool,
}

impl Hierarchy {
    /// The raw parents, in bone order.
    pub fn parents(&self) -> &[i16] {
        &self.parents
    }

    /// Bone count.
    pub fn len(&self) -> usize {
        self.parents.len()
    }

    /// Whether no bone is described (never true after a successful validate).
    pub fn is_empty(&self) -> bool {
        self.parents.is_empty()
    }

    /// The single root index.
    pub fn root(&self) -> usize {
        self.roots[0]
    }

    /// Every root; always length one for a validated hierarchy.
    pub fn roots(&self) -> &[usize] {
        &self.roots
    }

    /// Maximum number of parent edges from a bone up to the root.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Whether every parent index precedes its child in storage order.
    pub fn topological(&self) -> bool {
        self.topological
    }

    /// Raw parent of `bone`.
    pub fn parent_of(&self, bone: usize) -> Option<i16> {
        self.parents.get(bone).copied()
    }

    /// Parents-before-children order, deterministic and index-stable.
    ///
    /// A child appears only after its parent, so iterating this order is a safe
    /// composition order for a validated (acyclic) hierarchy.
    pub fn order(&self) -> Vec<usize> {
        let len = self.parents.len();
        let mut order = Vec::with_capacity(len);
        let mut placed = vec![false; len];
        for &root in &self.roots {
            placed[root] = true;
            order.push(root);
        }
        for _ in 0..len {
            let mut changed = false;
            for bone in 0..len {
                if placed[bone] {
                    continue;
                }
                let parent = self.parents[bone];
                if parent == ROOT || placed[parent as usize] {
                    placed[bone] = true;
                    order.push(bone);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        order
    }
}

/// Why a [`BoneHierarchy`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HierarchyError {
    /// The descriptor array carried no records.
    Empty,
    /// No record has `-1` as its parent.
    NoRoot,
    /// More than one record has `-1` as its parent.
    MultipleRoots { roots: usize },
    /// A parent index is below `-1` or outside `0..len`.
    ParentOutOfRange { bone: usize, parent: i16 },
    /// A parent chain loops without reaching the root.
    Cycle { bones: usize },
}

impl fmt::Display for HierarchyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("bone hierarchy has no records"),
            Self::NoRoot => f.write_str("bone hierarchy has no root (-1) record"),
            Self::MultipleRoots { roots } => {
                write!(f, "bone hierarchy has {roots} roots, expected exactly one")
            }
            Self::ParentOutOfRange { bone, parent } => {
                write!(f, "bone {bone} has parent {parent}, outside the hierarchy")
            }
            Self::Cycle { bones } => {
                write!(f, "bone hierarchy has a {bones}-bone parent cycle")
            }
        }
    }
}

/// Archive-wide agreement counters for [`Hierarchy::validate`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HierarchyCensus {
    /// Object containers scanned.
    pub objects: usize,
    /// Containers whose descriptor array was found.
    pub located: usize,
    /// Containers with no locatable descriptor array.
    pub unlocated: usize,
    /// Hierarchies that passed validation.
    pub valid: usize,
    /// Hierarchies that failed validation (sum of the buckets below).
    pub invalid: usize,
    pub empty: usize,
    pub no_root: usize,
    pub multiple_roots: usize,
    pub parent_out_of_range: usize,
    pub cycles: usize,
}

impl HierarchyCensus {
    /// Fold one container's parsed hierarchy (`None` = array not located).
    pub fn record(&mut self, hierarchy: Option<&BoneHierarchy>) {
        self.objects += 1;
        let Some(hierarchy) = hierarchy else {
            self.unlocated += 1;
            return;
        };
        self.located += 1;
        match hierarchy.validate() {
            Ok(_) => self.valid += 1,
            Err(error) => {
                self.invalid += 1;
                match error {
                    HierarchyError::Empty => self.empty += 1,
                    HierarchyError::NoRoot => self.no_root += 1,
                    HierarchyError::MultipleRoots { .. } => self.multiple_roots += 1,
                    HierarchyError::ParentOutOfRange { .. } => self.parent_out_of_range += 1,
                    HierarchyError::Cycle { .. } => self.cycles += 1,
                }
            }
        }
    }

    /// Percentage of located hierarchies that validate, or `None` when none
    /// were located.
    pub fn agreement_percent(&self) -> Option<f64> {
        (self.located > 0).then(|| 100.0 * self.valid as f64 / self.located as f64)
    }

    /// One stable summary line, no offsets or addresses.
    pub fn summary_line(&self) -> String {
        let agreement = self
            .agreement_percent()
            .map(|percent| format!("{percent:.1}%"))
            .unwrap_or_else(|| "n/a".to_string());
        format!(
            "hierarchy census objects={} located={} unlocated={} valid={} invalid={} agreement={agreement} empty={} no_root={} multiple_roots={} parent_out_of_range={} cycles={}",
            self.objects,
            self.located,
            self.unlocated,
            self.valid,
            self.invalid,
            self.empty,
            self.no_root,
            self.multiple_roots,
            self.parent_out_of_range,
            self.cycles
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptors(parents: &[i16]) -> Vec<SlotDescriptor> {
        parents
            .iter()
            .map(|&parent| SlotDescriptor {
                word0: 0,
                word4: 0,
                word6: 0,
                word8: 0,
                word10: 0,
                word12: 0,
                word14: 0,
                word16: 0,
                word18: parent as u16,
                aim_offset: 0,
                word22: 0,
            })
            .collect()
    }

    #[test]
    fn parses_signed_parents_and_accepts_one_acyclic_root() {
        let hierarchy = BoneHierarchy::from_descriptors(&descriptors(&[-1, 0, 1, 1, 3]));
        assert_eq!(hierarchy.parents(), &[-1, 0, 1, 1, 3]);
        let valid = hierarchy.validate().expect("one root, acyclic");
        assert_eq!(valid.root(), 0);
        assert_eq!(valid.roots(), &[0]);
        assert_eq!(valid.depth(), 3);
        assert!(valid.topological());
        assert_eq!(valid.order(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn one_child_before_its_parent_is_reported_not_rejected() {
        let valid = BoneHierarchy::from_parents(vec![1, -1])
            .validate()
            .expect("reversed storage is still one acyclic root");
        assert!(!valid.topological());
        assert_eq!(valid.order(), vec![1, 0]);
    }

    #[test]
    fn two_roots_are_refused() {
        assert_eq!(
            BoneHierarchy::from_parents(vec![-1, -1]).validate(),
            Err(HierarchyError::MultipleRoots { roots: 2 })
        );
    }

    #[test]
    fn a_cycle_is_refused_even_with_one_root() {
        assert_eq!(
            BoneHierarchy::from_parents(vec![-1, 2, 1]).validate(),
            Err(HierarchyError::Cycle { bones: 2 })
        );
        assert_eq!(
            BoneHierarchy::from_parents(vec![0, 1]).validate(),
            Err(HierarchyError::NoRoot)
        );
    }

    #[test]
    fn out_of_range_and_empty_hierarchies_are_refused() {
        assert_eq!(
            BoneHierarchy::from_parents(vec![-1, 5]).validate(),
            Err(HierarchyError::ParentOutOfRange { bone: 1, parent: 5 })
        );
        assert_eq!(
            BoneHierarchy::from_parents(vec![-1, -2]).validate(),
            Err(HierarchyError::ParentOutOfRange { bone: 1, parent: -2 })
        );
        assert_eq!(BoneHierarchy::from_parents(Vec::new()).validate(), Err(HierarchyError::Empty));
    }

    #[test]
    fn a_single_root_is_a_valid_one_bone_hierarchy() {
        let valid = BoneHierarchy::from_parents(vec![-1]).validate().expect("single root");
        assert_eq!((valid.len(), valid.depth(), valid.topological()), (1, 0, true));
        assert_eq!(valid.parent_of(0), Some(-1));
        assert_eq!(valid.parent_of(1), None);
    }

    #[test]
    fn census_counts_valid_invalid_and_unlocated_containers() {
        let mut census = HierarchyCensus::default();
        census.record(None);
        census.record(Some(&BoneHierarchy::from_parents(vec![-1, 0])));
        census.record(Some(&BoneHierarchy::from_parents(vec![-1, -1])));
        census.record(Some(&BoneHierarchy::from_parents(vec![-1, 2, 1])));
        census.record(Some(&BoneHierarchy::from_parents(vec![-1, 7])));
        census.record(Some(&BoneHierarchy::from_parents(Vec::new())));
        assert_eq!(
            (census.objects, census.located, census.unlocated),
            (6, 5, 1)
        );
        assert_eq!((census.valid, census.invalid), (1, 4));
        assert_eq!((census.multiple_roots, census.cycles, census.parent_out_of_range, census.empty), (1, 1, 1, 1));
        assert_eq!(census.agreement_percent(), Some(20.0));
        assert!(census.summary_line().contains("agreement=20.0%"));
        assert!(HierarchyCensus::default().summary_line().contains("agreement=n/a"));
    }
}
