//! Pure one-bone pose math for decoded `.SRM` skeletons (no I/O).
//!
//! An `extra = 0` model stores 128 joint positions as 32 records of four `vec3` each,
//! plus three `(index, weight)` pairs per vertex. It has no stored per-slot matrix,
//! no hierarchy, and no clips, so a "joint transform" is built here as a rotation
//! about that joint's head. Joints are mapped to mesh space by negating Z
//! (`docs/changes/animation/design.md`, P5); vertices are already mesh space.
//! Weights blend linearly and are divided by the actual weight sum. Tests use
//! synthetic data only.
//!
//! The same module also owns the archived object-model skinning path
//! ([`skin`] / [`skin_positions`]). Those vertices are **bone-local** and each
//! bone owns a contiguous vertex range, so a 13-float runtime slot pose becomes
//! one affine per bone. The deformation pipeline is deliberately separate from
//! the animation semantics that produce the pose; see
//! `docs/notes/object-models.md` for the slot layout and the two UNKNOWN terms.

use crate::bone_hierarchy::{BoneHierarchy, ROOT};
use crate::object::{ObjectModel, Vertex};

pub use crate::srm::JOINT_SLOTS;

const MODEL_UP: [f32; 3] = [0.0, 0.0, 1.0];
const FALLBACK_AXIS: [f32; 3] = [1.0, 0.0, 0.0];
const EPS: f32 = 1e-6;

/// Row-major 3x4 affine: `[m00, m01, m02, tx, m10, ...]`, applied as `M * p + t`.
pub type Transform = [f32; 12];
/// Row-major 3x3 rotation.
pub type Mat3 = [[f32; 3]; 3];

/// One parsed `<bone>:<degrees>` pose request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoseSpec {
    pub bone: usize,
    pub degrees: f32,
}

/// Posed positions plus how many vertices actually moved.
#[derive(Debug, Clone, PartialEq)]
pub struct PoseOutcome {
    pub positions: Vec<[f32; 3]>,
    pub moved_vertices: usize,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn finite(p: [f32; 3]) -> bool {
    p.iter().all(|v| v.is_finite())
}

fn normalize(v: [f32; 3]) -> Option<[f32; 3]> {
    let len = dot(v, v).sqrt();
    (len.is_finite() && len > EPS).then(|| [v[0] / len, v[1] / len, v[2] / len])
}

fn rotate(m: Mat3, p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2],
        m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2],
        m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2],
    ]
}

fn affine(t: Transform, p: [f32; 3]) -> [f32; 3] {
    [
        t[0] * p[0] + t[1] * p[1] + t[2] * p[2] + t[3],
        t[4] * p[0] + t[5] * p[1] + t[6] * p[2] + t[7],
        t[8] * p[0] + t[9] * p[1] + t[10] * p[2] + t[11],
    ]
}

/// The no-op affine; every non-target joint contributes this.
pub fn identity_transform() -> Transform {
    [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]
}

/// Map raw decoded joints to mesh space by negating Z, preserving order.
pub fn mesh_space_joints(joints: &[[f32; 3]]) -> Vec<[f32; 3]> {
    joints.iter().map(|p| [p[0], p[1], -p[2]]).collect()
}

/// Parse `<bone>:<degrees>`: one colon, integer bone in `0..=127`, finite degrees
/// with `|degrees| <= 360`. Everything else is loud.
pub fn parse_spec(text: &str) -> Result<PoseSpec, String> {
    let mut parts = text.split(':');
    let (Some(bone), Some(degrees), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(format!("pose spec must be <bone>:<degrees>, got {text:?}"));
    };
    let bone: usize = bone.parse().map_err(|_| format!("pose bone must be an integer 0..={}, got {bone:?}", JOINT_SLOTS - 1))?;
    if bone >= JOINT_SLOTS {
        return Err(format!("pose bone {bone} is outside the {JOINT_SLOTS}-slot joint pool"));
    }
    let degrees: f32 = degrees.parse().map_err(|_| format!("pose angle must be a number, got {degrees:?}"))?;
    if !degrees.is_finite() || degrees.abs() > 360.0 {
        return Err(format!("pose angle must be finite and within +/-360 degrees, got {degrees}"));
    }
    Ok(PoseSpec { bone, degrees })
}

/// Rodrigues rotation about a caller-supplied axis (any non-zero finite direction).
pub fn axis_angle_matrix(axis: [f32; 3], degrees: f32) -> Result<Mat3, String> {
    let a = normalize(axis).ok_or_else(|| format!("rotation axis {axis:?} is zero-length or non-finite"))?;
    if !degrees.is_finite() {
        return Err(format!("rotation angle must be finite, got {degrees}"));
    }
    let (s, c) = degrees.to_radians().sin_cos();
    let t = 1.0 - c;
    let (x, y, z) = (a[0], a[1], a[2]);
    Ok([
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ])
}

/// Take the upper-left 3x3 of a caller-supplied row-major 4x4 matrix.
/// Only the tests exercise this today; the CLI pose path builds rotations
/// directly through [`axis_angle_matrix`].
#[cfg(test)]
pub fn mat3_from_mat4(m: &[[f32; 4]; 4]) -> Mat3 {
    [[m[0][0], m[0][1], m[0][2]], [m[1][0], m[1][1], m[1][2]], [m[2][0], m[2][1], m[2][2]]]
}

/// Swing axis for a joint: `normalize(cross(tail - head, +Z))`, where the tail is the
/// adjacent slot in the same four-slot record (next for corners 0..2, previous for 3).
/// A degenerate direction yields `+X`.
pub fn derived_axis(mesh_joints: &[[f32; 3]], slot: usize) -> Result<[f32; 3], String> {
    let head = *mesh_joints.get(slot).ok_or_else(|| format!("joint slot {slot} is outside the {} decoded joints", mesh_joints.len()))?;
    let tail_slot = if slot % 4 == 3 { slot - 1 } else { slot + 1 };
    let tail = *mesh_joints.get(tail_slot).ok_or_else(|| format!("joint tail slot {tail_slot} is outside the {} decoded joints", mesh_joints.len()))?;
    if !finite(head) || !finite(tail) {
        return Err(format!("joint slot {slot} or its tail {tail_slot} is not finite"));
    }
    Ok(normalize(cross(sub(tail, head), MODEL_UP)).unwrap_or(FALLBACK_AXIS))
}

/// Build the 3x4 affine for one joint slot: `rotation` about its head
/// (`t = head - R * head`). The `extra = 0` class stores no per-slot matrix.
pub fn transform_for_joint(mesh_joints: &[[f32; 3]], slot: usize, rotation: Mat3) -> Result<Transform, String> {
    let head = *mesh_joints.get(slot).ok_or_else(|| format!("joint slot {slot} is outside the {} decoded joints", mesh_joints.len()))?;
    if !finite(head) {
        return Err(format!("joint slot {slot} is not finite: {head:?}"));
    }
    if rotation.iter().flatten().any(|v| !v.is_finite()) {
        return Err(format!("rotation matrix for joint slot {slot} is not finite"));
    }
    let t = sub(head, rotate(rotation, head));
    let r = rotation;
    Ok([r[0][0], r[0][1], r[0][2], t[0], r[1][0], r[1][1], r[1][2], t[1], r[2][0], r[2][1], r[2][2], t[2]])
}

/// Blend `sum(w_i * M_i * p) / sum(w_i)` per vertex. Duplicate indices sum their
/// weights; a zero total copies the vertex unchanged. Every index must be inside
/// `transforms` (which also defines the joint count).
pub fn apply_transforms(positions: &[[f32; 3]], bone_indices: &[[u8; 3]], bone_weights: &[[u8; 3]], transforms: &[Transform]) -> Result<PoseOutcome, String> {
    if positions.len() != bone_indices.len() || positions.len() != bone_weights.len() {
        return Err(format!("length mismatch: {} positions, {} index triples, {} weight triples", positions.len(), bone_indices.len(), bone_weights.len()));
    }
    let mut out = Vec::with_capacity(positions.len());
    let mut moved = 0;
    for (v, ((p, indices), weights)) in positions.iter().zip(bone_indices).zip(bone_weights).enumerate() {
        let (mut acc, mut sum) = ([0.0f32; 3], 0.0f32);
        for k in 0..3 {
            let slot = usize::from(indices[k]);
            let t = transforms.get(slot).ok_or_else(|| format!("vertex {v} references joint {slot}, outside the {} decoded joints", transforms.len()))?;
            let w = f32::from(weights[k]);
            let q = affine(*t, *p);
            for axis in 0..3 {
                acc[axis] += w * q[axis];
            }
            sum += w;
        }
        let posed = if sum > 0.0 { [acc[0] / sum, acc[1] / sum, acc[2] / sum] } else { *p };
        if !finite(posed) {
            return Err(format!("vertex {v} produced a non-finite position"));
        }
        // The renormalised blend can differ from the rest position by a few
        // f32 ulp even when every contributing transform is the identity.
        // Compare against a magnitude-relative epsilon so a zero-angle pose
        // (control run) reports zero moved vertices and keeps exact rest
        // positions instead of counting rounding noise.
        let scale = p.iter().fold(1.0f32, |m, v| m.max(v.abs()));
        let d = sub(posed, *p);
        if dot(d, d).sqrt() > EPS * scale {
            moved += 1;
            out.push(posed);
        } else {
            out.push(*p);
        }
    }
    Ok(PoseOutcome { positions: out, moved_vertices: moved })
}

/// Apply `rotation` to exactly one joint slot. Refuses a slot with no non-zero
/// vertex weight. Other joints keep the identity transform.
pub fn apply_joint_rotation(positions: &[[f32; 3]], bone_indices: &[[u8; 3]], bone_weights: &[[u8; 3]], mesh_joints: &[[f32; 3]], slot: usize, rotation: Mat3) -> Result<PoseOutcome, String> {
    let transform = transform_for_joint(mesh_joints, slot, rotation)?;
    let weighted = bone_indices.iter().zip(bone_weights).any(|(indices, weights)| (0..3).any(|k| usize::from(indices[k]) == slot && weights[k] > 0));
    if !weighted {
        return Err(format!("joint slot {slot} has no vertex with a non-zero weight; nothing to pose"));
    }
    let mut transforms = vec![identity_transform(); mesh_joints.len()];
    transforms[slot] = transform;
    apply_transforms(positions, bone_indices, bone_weights, &transforms)
}

/// Full pose path: derive the swing axis for `spec.bone`, rotate by `spec.degrees`,
/// and blend. `mesh_joints` must already be mesh space.
pub fn apply(positions: &[[f32; 3]], bone_indices: &[[u8; 3]], bone_weights: &[[u8; 3]], mesh_joints: &[[f32; 3]], spec: PoseSpec) -> Result<PoseOutcome, String> {
    let axis = derived_axis(mesh_joints, spec.bone)?;
    let rotation = axis_angle_matrix(axis, spec.degrees)?;
    apply_joint_rotation(positions, bone_indices, bone_weights, mesh_joints, spec.bone, rotation)
}

/// One runtime slot pose exactly as the loader supplies it: row-major 3x3
/// rotation at indices `0..=8`, one scale marker at index `9`, and a
/// translation at indices `10..=12`.
///
/// This is a deformation input only. The animation semantics that produce the
/// 13 floats (frame selection, mode bits, composition order) are out of scope
/// and UNKNOWN here.
pub type SlotPose = [f32; 13];

/// The affine one [`SlotPose`] contributes to one bone.
///
/// - `rotation` is the first nine floats, copied row-major and applied as
///   given. Only finiteness is checked; an authored pose is trusted to be the
///   rotation the runtime built.
/// - `translation` is the last three floats, added after the rotation.
/// - `scale` is **always `1.0`**. Index 9 is the scale marker, but what the
///   marker means is UNKNOWN (`docs/notes/object-models.md`), so it is
///   preserved in `scale_marker` for inspection and never multiplies vertices.
///   Treating it as a scale would invent semantics the corpus does not support.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoseMatrix {
    pub rotation: Mat3,
    pub scale: f32,
    pub translation: [f32; 3],
    pub scale_marker: f32,
}

/// Build the [`PoseMatrix`] for one [`SlotPose`]. Every float must be finite;
/// the rotation is not re-orthonormalised and the scale marker is not applied.
pub fn slot_pose_matrix(pose: &SlotPose) -> Result<PoseMatrix, String> {
    if pose.iter().any(|value| !value.is_finite()) {
        return Err("slot pose contains a non-finite value".to_string());
    }
    Ok(PoseMatrix {
        rotation: [
            [pose[0], pose[1], pose[2]],
            [pose[3], pose[4], pose[5]],
            [pose[6], pose[7], pose[8]],
        ],
        scale: 1.0,
        translation: [pose[10], pose[11], pose[12]],
        scale_marker: pose[9],
    })
}

/// `3x3` product `a * b`, row-major.
fn mat3_mul(a: Mat3, b: Mat3) -> Mat3 {
    let mut out = [[0.0f32; 3]; 3];
    for (row, out_row) in out.iter_mut().enumerate() {
        for col in 0..3 {
            out_row[col] =
                a[row][0] * b[0][col] + a[row][1] * b[1][col] + a[row][2] * b[2][col];
        }
    }
    out
}

/// One full turn in the runtime's binary angle unit: `4096` units `== 360`
/// degrees. This is a turn-based unit, not radians and not a `1.0 == 4096`
/// fraction.
#[allow(dead_code)]
pub const BINARY_ANGLE_UNITS_PER_TURN: f32 = 4096.0;

/// Convert one binary angle (`4096` units per turn) to radians.
#[allow(dead_code)]
pub fn binary_angle_to_radians(value: f32) -> f32 {
    value * (std::f32::consts::TAU / BINARY_ANGLE_UNITS_PER_TURN)
}

/// Build the rotation the runtime orientation constructors produce for one
/// `(a, b, c)` binary-angle triple: `Rz(c) * Ry(b) * Rx(a)`, row-major.
///
/// X is the innermost (first-applied) factor and Z the outermost. The fit is
/// unique on the captured corpus; the formula and its evidence are in
/// `docs/notes/rotation-model.md`.
///
/// This helper is additive. `slot_pose_matrix` keeps copying the stored 3x3
/// exactly as the loader supplies it, so no existing CLI behaviour changes and
/// nothing consumes this function yet.
#[allow(dead_code)]
pub fn rotation_from_binary_angles(a: f32, b: f32, c: f32) -> Mat3 {
    let (sa, ca) = binary_angle_to_radians(a).sin_cos();
    let (sb, cb) = binary_angle_to_radians(b).sin_cos();
    let (sc, cc) = binary_angle_to_radians(c).sin_cos();
    let rx = [[1.0, 0.0, 0.0], [0.0, ca, -sa], [0.0, sa, ca]];
    let ry = [[cb, 0.0, sb], [0.0, 1.0, 0.0], [-sb, 0.0, cb]];
    let rz = [[cc, -sc, 0.0], [sc, cc, 0.0], [0.0, 0.0, 1.0]];
    mat3_mul(mat3_mul(rz, ry), rx)
}

impl PoseMatrix {
    /// The no-op pose: identity rotation, zero translation, unit scale.
    pub fn identity() -> Self {
        Self {
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            scale: 1.0,
            translation: [0.0; 3],
            scale_marker: 0.0,
        }
    }

    /// The affine this pose contributes, row-major `[R | t]`.
    #[allow(dead_code)]
    pub fn transform(&self) -> Transform {
        let s = self.scale;
        let r = self.rotation;
        [
            r[0][0] * s, r[0][1] * s, r[0][2] * s, self.translation[0],
            r[1][0] * s, r[1][1] * s, r[1][2] * s, self.translation[1],
            r[2][0] * s, r[2][1] * s, r[2][2] * s, self.translation[2],
        ]
    }

    /// Compose `self` (a parent world matrix) with a child's local matrix:
    /// `world * local`, i.e. rotation `R_w * R_l` and translation
    /// `R_w * t_l + t_w`.
    pub fn then(&self, local: &PoseMatrix) -> PoseMatrix {
        let rotation = mat3_mul(self.rotation, local.rotation);
        let scaled = [
            local.translation[0] * self.scale,
            local.translation[1] * self.scale,
            local.translation[2] * self.scale,
        ];
        let t = rotate(self.rotation, scaled);
        PoseMatrix {
            rotation,
            scale: self.scale * local.scale,
            translation: [
                t[0] + self.translation[0],
                t[1] + self.translation[1],
                t[2] + self.translation[2],
            ],
            scale_marker: local.scale_marker,
        }
    }
}

/// Resolve `world[child] = world[parent] * local[child]` from the root down.
///
/// Locals are supplied directly; `compose` and [`compose_with_locals`] build
/// them from a pose slice. The walk is bounded by the bone count and never
/// panics: a bone whose parent is out of range, or a bone inside a parent
/// cycle, keeps its own local matrix instead of blocking composition.
fn compose_locals(hierarchy: &BoneHierarchy, locals: &[PoseMatrix]) -> Vec<PoseMatrix> {
    let len = hierarchy.len();
    let mut world = locals.to_vec();
    let mut resolved = vec![false; len];
    for bone in 0..len {
        let parent = hierarchy.parent_of(bone).unwrap_or(ROOT);
        let usable = parent >= 0 && (parent as usize) < len && parent as usize != bone;
        if !usable {
            resolved[bone] = true;
        }
    }
    for _ in 0..len {
        let mut changed = false;
        for bone in 0..len {
            if resolved[bone] {
                continue;
            }
            let parent = hierarchy.parent_of(bone).unwrap_or(ROOT);
            if parent >= 0 && (parent as usize) < len && resolved[parent as usize] {
                world[bone] = world[parent as usize].then(&locals[bone]);
                resolved[bone] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    world
}

/// Compose a pose-only tree: `world[child] = world[parent] * local[child]`.
///
/// Poses are indexed by bone; the slice's order is the bone order, and the
/// result does not depend on the hierarchy's storage order (children stored
/// before parents resolve on a later pass). A missing or non-finite pose is the
/// identity local, never a panic. This is the pure pose composition; skinning
/// uses [`compose_with_locals`], which also folds in each bone's rest offset.
#[allow(dead_code)]
pub fn compose(hierarchy: &BoneHierarchy, poses: &[SlotPose]) -> Vec<PoseMatrix> {
    compose_with_locals(hierarchy, poses, &[])
}

/// Compose the tree while giving each bone its own rest offset:
/// `local[bone] = pose * translate(rest_offset)`, so an all-identity pose
/// reproduces the rest translation chain.
///
/// This is the matrix the hierarchical skin applies to bone-local vertices.
/// Every bone still rotates about its own origin: the offset is the distance
/// from the parent bone's origin, exactly like [`crate::object`]'s rest chain.
pub fn compose_with_locals(
    hierarchy: &BoneHierarchy,
    poses: &[SlotPose],
    locals: &[[f32; 3]],
) -> Vec<PoseMatrix> {
    let local_matrices: Vec<PoseMatrix> = (0..hierarchy.len())
        .map(|bone| {
            let mut matrix = poses
                .get(bone)
                .and_then(|pose| slot_pose_matrix(pose).ok())
                .unwrap_or_else(PoseMatrix::identity);
            if let Some(offset) = locals.get(bone) {
                for axis in 0..3 {
                    matrix.translation[axis] += offset[axis];
                }
            }
            matrix
        })
        .collect();
    compose_locals(hierarchy, &local_matrices)
}

/// Parse the documented pose-file shape: one 13-float [`SlotPose`] per bone, in
/// bone order. Blank lines and `#` comments are skipped; every other line must
/// hold exactly 13 finite floats.
///
/// Extra lines are accepted and ignored by [`skin`] (more poses than bones);
/// fewer lines leave the missing bones at rest. Nothing here knows the bone
/// count, so the same file can be reused against different models.
pub fn parse_pose_lines(text: &str) -> Result<Vec<SlotPose>, String> {
    let mut poses = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut values = [0.0f32; 13];
        let mut count = 0;
        for token in line.split_whitespace() {
            let value: f32 = token
                .parse()
                .map_err(|_| format!("pose line {}: {token:?} is not a number", index + 1))?;
            if !value.is_finite() {
                return Err(format!("pose line {}: {token:?} is not finite", index + 1));
            }
            if count == values.len() {
                return Err(format!(
                    "pose line {} has more than 13 values",
                    index + 1
                ));
            }
            values[count] = value;
            count += 1;
        }
        if count != values.len() {
            return Err(format!(
                "pose line {} has {count} values, expected 13",
                index + 1
            ));
        }
        poses.push(values);
    }
    Ok(poses)
}

/// Deform `model` with one [`SlotPose`] per bone, in model space.
///
/// Archived object vertices are bone-local and every bone owns
/// `first_vertex..=last_vertex`, so a bone's pose is applied in that local
/// frame and the bone's rest translation (its accumulated ancestor chain,
/// exactly as [`crate::object::rest_positions`] computes it) is added back:
///
/// ```text
/// posed = scale * R * local + rest_offset + translation
/// ```
///
/// The decision is **pose over rest, added**: a bone pose does not replace the
/// rest translation, it rotates the bone-local vertices about the bone's rest
/// origin and then offsets them. Two reasons, both from the existing code:
///
/// 1. [`crate::object::rest_positions`] is a pure translation chain with no
///    rotation, so an identity pose at zero translation must land exactly on
///    the rest pose. Adding keeps that true; replacing would collapse every
///    part onto its bone origin.
/// 2. The existing mesh-space path ([`transform_for_joint`]) rotates about the
///    joint head and re-adds it (`t = head - R * head`). Here the bone's rest
///    translation *is* that head, so `R * local + rest_offset` is the same
///    composition.
///
/// A bone without a pose (fewer poses than bones) uses the rest pose; poses
/// past the bone count are ignored. A pose with any non-finite float is
/// treated as absent rather than poisoning a vertex. Nothing panics and no
/// missing bone is silently zeroed.
pub fn skin_positions(model: &ObjectModel, poses: &[SlotPose]) -> Vec<[f32; 3]> {
    let mut positions: Vec<[f32; 3]> = model.vertices.iter().map(|(p, _)| p.map(f32::from)).collect();
    if positions.is_empty() {
        return positions;
    }
    let offsets = crate::object::bone_world_offsets(&model.bones);
    for (index, bone) in model.bones.iter().enumerate() {
        if bone.first_vertex == u16::MAX || bone.last_vertex == u16::MAX {
            continue;
        }
        let first = bone.first_vertex as usize;
        let last = (bone.last_vertex as usize).min(positions.len() - 1);
        if first > last {
            continue;
        }
        let matrix = poses.get(index).and_then(|pose| slot_pose_matrix(pose).ok());
        let offset = offsets
            .get(index)
            .copied()
            .unwrap_or([0; 3])
            .map(|value| value as f32);
        for position in &mut positions[first..=last] {
            *position = match matrix {
                Some(matrix) => {
                    let rotated = rotate(matrix.rotation, *position);
                    [
                        rotated[0] * matrix.scale + offset[0] + matrix.translation[0],
                        rotated[1] * matrix.scale + offset[1] + matrix.translation[1],
                        rotated[2] * matrix.scale + offset[2] + matrix.translation[2],
                    ]
                }
                None => [
                    position[0] + offset[0],
                    position[1] + offset[1],
                    position[2] + offset[2],
                ],
            };
        }
    }
    positions
}

/// [`skin_positions`] in the archived 8-byte vertex shape, with the normal id
/// preserved. Positions are model-space now, so they are rounded to the nearest
/// integer and clamped to the `i16` range of the stored shape; renderers should
/// consume [`skin_positions`], which keeps full `f32` precision.
pub fn skin(model: &ObjectModel, poses: &[SlotPose]) -> Vec<Vertex> {
    let positions = skin_positions(model, poses);
    model
        .vertices
        .iter()
        .zip(positions)
        .map(|((_, normal), position)| (quantize(position), *normal))
        .collect()
}

fn quantize(position: [f32; 3]) -> [i16; 3] {
    position.map(|value| value.round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16)
}

/// Hierarchical skin: each bone-local vertex is placed by that bone's
/// **composed** world matrix, not its local pose alone.
///
/// ```text
/// world[child] = world[parent] * (pose[child] * translate(bone.rest_local))
/// posed[v]     = world[bone(v)] * vertex[v]
/// ```
///
/// So a root rotation carries every descendant's frame with it, while a leaf
/// rotation moves only its own vertex range. An all-identity pose reproduces
/// [`crate::object::rest_positions`] exactly. The hierarchy must describe the
/// same bone count as the model; a mismatch is a loud error, not a silent
/// partial pose. Missing poses stay identity (`compose_with_locals`), and this
/// path never panics.
#[allow(dead_code)]
pub fn skin_positions_hierarchical(
    model: &ObjectModel,
    hierarchy: &BoneHierarchy,
    poses: &[SlotPose],
) -> Result<Vec<[f32; 3]>, String> {
    if hierarchy.len() != model.bones.len() {
        return Err(format!(
            "hierarchy has {} bones but the model has {}",
            hierarchy.len(),
            model.bones.len()
        ));
    }
    let locals: Vec<[f32; 3]> = model
        .bones
        .iter()
        .map(|bone| bone.local.map(f32::from))
        .collect();
    let world = compose_with_locals(hierarchy, poses, &locals);
    let mut positions: Vec<[f32; 3]> = model
        .vertices
        .iter()
        .map(|(position, _)| position.map(f32::from))
        .collect();
    if positions.is_empty() {
        return Ok(positions);
    }
    for (index, bone) in model.bones.iter().enumerate() {
        if bone.first_vertex == u16::MAX || bone.last_vertex == u16::MAX {
            continue;
        }
        let first = bone.first_vertex as usize;
        let last = (bone.last_vertex as usize).min(positions.len() - 1);
        if first > last {
            continue;
        }
        let matrix = world[index].transform();
        for position in &mut positions[first..=last] {
            *position = affine(matrix, *position);
        }
    }
    Ok(positions)
}

/// Axis-aligned bounds of `positions`, or `None` when there are none.
pub fn bounds(positions: &[[f32; 3]]) -> Option<([f32; 3], [f32; 3])> {
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    for position in positions {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(position[axis]);
            maximum[axis] = maximum[axis].max(position[axis]);
        }
    }
    (minimum[0] <= maximum[0]).then_some((minimum, maximum))
}

#[cfg(test)]
mod tests {
    use super::*;

    const R_X90: Mat3 = [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]];

    fn close(actual: [f32; 3], expected: [f32; 3]) {
        for axis in 0..3 {
            assert!((actual[axis] - expected[axis]).abs() < 1e-5, "axis {axis}: {actual:?} != {expected:?}");
        }
    }

    fn joints() -> Vec<[f32; 3]> {
        vec![[0.0; 3]; JOINT_SLOTS]
    }

    fn translation(x: f32, y: f32, z: f32) -> Transform {
        [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z]
    }

    #[test]
    fn parse_spec_accepts_only_the_documented_shape() {
        assert_eq!(parse_spec("127:-90.5"), Ok(PoseSpec { bone: 127, degrees: -90.5 }));
        for good in ["0:0", "7:45", "7:360", "7:-360.0"] {
            assert!(parse_spec(good).is_ok(), "{good:?} must parse");
        }
        for bad in ["7", ":45", "7:", "7:45:0", "x:45", "7:nan", "7:361", "128:45", "-1:45", "", "7:45 ", "7:inf"] {
            assert!(parse_spec(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn mesh_space_and_zero_angle_preserve_the_rest_pose() {
        assert_eq!(mesh_space_joints(&[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]), vec![[1.0, 2.0, -3.0], [4.0, 5.0, -6.0]]);
        let positions = vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
        let out = apply(&positions, &[[0, 0, 0]; 2], &[[255, 0, 0]; 2], &joints(), PoseSpec { bone: 0, degrees: 0.0 }).expect("zero-angle pose");
        assert_eq!(out.positions, positions);
        assert_eq!(out.moved_vertices, 0);
    }

    #[test]
    fn zero_angle_keeps_large_rest_coordinates_exact() {
        let positions = vec![[12345.0, -9876.0, 5432.0], [20000.0, 10000.0, -30000.0]];
        let out = apply(&positions, &[[0, 1, 2]; 2], &[[85, 85, 85]; 2], &joints(), PoseSpec { bone: 0, degrees: 0.0 }).expect("zero-angle pose");
        assert_eq!(out.positions, positions, "rounding noise must never displace the rest mesh");
        assert_eq!(out.moved_vertices, 0);
    }

    #[test]
    fn full_weight_rotation_hits_exact_coordinates_and_keeps_the_pivot() {
        let out = apply_joint_rotation(&[[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]], &[[0, 0, 0]; 3], &[[255, 0, 0]; 3], &joints(), 0, R_X90).expect("full-weight rotation");
        assert_eq!(out.positions[0], [0.0, -1.0, 0.0]);
        assert_eq!(out.positions[1], [1.0, 0.0, 0.0]);
        assert_eq!(out.positions[2], [0.0, 0.0, 0.0]);
        assert_eq!(out.moved_vertices, 1);
    }

    #[test]
    fn derived_axis_uses_the_adjacent_slot_and_falls_back() {
        let mut j = joints();
        j[1] = [0.0, 1.0, 0.0];
        assert_eq!(derived_axis(&j, 0), Ok([1.0, 0.0, 0.0]));
        j[1] = [0.0, 0.0, 5.0];
        assert_eq!(derived_axis(&j, 0), Ok(FALLBACK_AXIS));
        j[2] = [0.0, 1.0, 0.0];
        assert_eq!(derived_axis(&j, 3), Ok([1.0, 0.0, 0.0]));
        let mut raw = joints();
        raw[1] = [0.0, 1.0, 0.0];
        let out = apply(&[[0.0, 0.0, 1.0]], &[[0, 0, 0]], &[[255, 0, 0]], &mesh_space_joints(&raw), PoseSpec { bone: 0, degrees: 90.0 }).expect("derived-axis pose");
        close(out.positions[0], [0.0, -1.0, 0.0]);
    }

    #[test]
    fn partial_and_duplicate_weights_blend() {
        let partial = apply_joint_rotation(&[[0.0, 0.0, 1.0]], &[[0, 1, 0]], &[[128, 127, 0]], &joints(), 0, R_X90).expect("partial weight");
        let t = 128.0 / 255.0;
        close(partial.positions[0], [0.0, -t, 1.0 - t]);
        let duplicate = apply_joint_rotation(&[[0.0, 0.0, 1.0]], &[[0, 0, 0]], &[[128, 127, 0]], &joints(), 0, R_X90).expect("duplicate indices");
        close(duplicate.positions[0], [0.0, -1.0, 0.0]);
        assert_eq!(partial.moved_vertices, 1);
    }

    #[test]
    fn mixed_weights_normalise_by_the_actual_sum_and_leave_untouched_vertices() {
        let positions = [[7.0, 8.0, 9.0], [1.0, 2.0, 3.0]];
        let mut transforms = vec![identity_transform(); JOINT_SLOTS];
        transforms[0] = transform_for_joint(&joints(), 0, R_X90).expect("joint transform");
        let untouched = apply_transforms(&positions, &[[0, 1, 2], [0, 1, 0]], &[[0, 0, 0], [0, 255, 0]], &transforms).expect("untouched vertices");
        assert_eq!(untouched.positions, positions.to_vec());
        assert_eq!(untouched.moved_vertices, 0);
        transforms[5] = translation(1.0, 0.0, 0.0);
        transforms[6] = translation(0.0, 2.0, 0.0);
        let mixed = apply_transforms(&[[2.0, 0.0, 0.0]], &[[5, 6, 0]], &[[51, 204, 0]], &transforms).expect("mixed weights");
        close(mixed.positions[0], [2.2, 1.6, 0.0]);
        transforms[0] = translation(5.0, 0.0, 0.0);
        let unit = apply_transforms(&[[0.0, 0.0, 0.0]], &[[0, 0, 0]], &[[1, 0, 0]], &transforms).expect("actual weight sum");
        close(unit.positions[0], [5.0, 0.0, 0.0]);
        assert_eq!(mixed.moved_vertices + unit.moved_vertices, 2);
    }

    #[test]
    fn refuses_out_of_range_unweighted_and_non_finite_inputs() {
        let err = apply_transforms(&[[0.0, 0.0, 0.0]], &[[200, 0, 0]], &[[255, 0, 0]], &vec![identity_transform(); JOINT_SLOTS]).expect_err("index 200");
        assert!(err.contains("200"), "{err}");
        assert!(transform_for_joint(&joints()[..2], 5, R_X90).is_err());
        let err = apply(&[[0.0, 0.0, 0.0]], &[[0, 0, 0]], &[[255, 0, 0]], &joints(), PoseSpec { bone: 5, degrees: 45.0 }).expect_err("no weights");
        assert!(err.contains("no vertex"), "{err}");
        let mut j = joints();
        j[0] = [f32::NAN, 0.0, 0.0];
        assert!(derived_axis(&j, 0).is_err());
        assert!(apply_transforms(&[[0.0, 0.0, 0.0]], &[], &[[255, 0, 0]], &[identity_transform()]).is_err());
        assert!(axis_angle_matrix([0.0, 0.0, 0.0], 45.0).is_err());
        assert!(axis_angle_matrix([1.0, 0.0, 0.0], f32::INFINITY).is_err());
    }

    #[test]
    fn mat4_extraction_takes_the_upper_basis() {
        let m = [[1.0, 2.0, 3.0, 0.0], [4.0, 5.0, 6.0, 0.0], [7.0, 8.0, 9.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        assert_eq!(mat3_from_mat4(&m), [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]);
    }

    const IDENTITY_SLOT: SlotPose = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0];

    fn slot_pose(rotation: Mat3, translation: [f32; 3]) -> SlotPose {
        [
            rotation[0][0], rotation[0][1], rotation[0][2],
            rotation[1][0], rotation[1][1], rotation[1][2],
            rotation[2][0], rotation[2][1], rotation[2][2],
            1.0, translation[0], translation[1], translation[2],
        ]
    }

    fn single_bone_model() -> crate::object::ObjectModel {
        crate::object::ObjectModel {
            vertices: vec![([1, 2, 3], 7), ([4, 5, 6], 9)],
            bones: vec![crate::object::Bone {
                first_vertex: 0,
                last_vertex: 1,
                local: [10, 0, 0],
                parent: u16::MAX,
            }],
            ..crate::object::ObjectModel::default()
        }
    }

    #[test]
    fn slot_pose_matrix_copies_the_rotation_and_translation_but_not_the_marker() {
        let pose = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 2.5, 10.0, 20.0, 30.0];
        let matrix = slot_pose_matrix(&pose).expect("finite pose");
        assert_eq!(matrix.rotation, [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]);
        assert_eq!(matrix.translation, [10.0, 20.0, 30.0]);
        assert_eq!(matrix.scale_marker, 2.5, "the raw marker stays observable");
        assert_eq!(matrix.scale, 1.0, "the unknown marker must not scale");
        assert!(slot_pose_matrix(&[f32::NAN; 13]).is_err());
    }

    #[test]
    fn identity_slot_pose_reproduces_the_rest_pose_exactly() {
        let model = crate::object::parse_object(&crate::object::synthetic_bone_model())
            .expect("bone model must parse");
        let poses = vec![IDENTITY_SLOT; model.bones.len()];
        let rest = crate::object::rest_positions(&model);
        assert_eq!(skin_positions(&model, &poses), rest);
        let mut marked = IDENTITY_SLOT;
        marked[9] = 37.5;
        assert_eq!(
            skin_positions(&model, &vec![marked; model.bones.len()]),
            rest,
            "the unknown scale marker must not deform the rest pose"
        );
        let skinned = skin(&model, &poses);
        let positions: Vec<[f32; 3]> = skinned.iter().map(|(p, _)| p.map(f32::from)).collect();
        let normals: Vec<u16> = skinned.iter().map(|(_, n)| *n).collect();
        assert_eq!(positions, rest, "the archived shape keeps the rest positions");
        assert_eq!(normals, model.vertices.iter().map(|(_, n)| *n).collect::<Vec<_>>());
    }

    #[test]
    fn one_bone_rotation_changes_only_its_range() {
        let model = crate::object::parse_object(&crate::object::synthetic_bone_model())
            .expect("bone model must parse");
        let rest = crate::object::rest_positions(&model);
        let poses = [IDENTITY_SLOT, slot_pose(R_X90, [0.0; 3])];
        let posed = skin_positions(&model, &poses);
        assert_eq!(&posed[..2], &rest[..2], "bone 0 must stay untouched");
        assert_eq!(posed[2], [0.0, 10.0, 20.0], "bone 1 rotates about its rest origin");
        assert_eq!(posed[3], [0.0, 0.0, 10.0], "a vertex at the pivot stays put");
    }

    #[test]
    fn one_bone_translation_moves_only_its_range_by_the_delta() {
        let model = crate::object::parse_object(&crate::object::synthetic_bone_model())
            .expect("bone model must parse");
        let rest = crate::object::rest_positions(&model);
        let delta = [5.0, -3.0, 2.0];
        let posed = skin_positions(&model, &[IDENTITY_SLOT, slot_pose([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], delta)]);
        assert_eq!(&posed[..2], &rest[..2]);
        for index in 2..4 {
            assert_eq!(
                posed[index],
                [rest[index][0] + delta[0], rest[index][1] + delta[1], rest[index][2] + delta[2]]
            );
        }
    }

    #[test]
    fn missing_poses_fall_back_to_rest_and_extra_poses_are_ignored() {
        let model = crate::object::parse_object(&crate::object::synthetic_bone_model())
            .expect("bone model must parse");
        let rest = crate::object::rest_positions(&model);
        assert_eq!(skin_positions(&model, &[]), rest, "no poses at all is the rest pose");
        assert_eq!(
            skin_positions(&model, &[IDENTITY_SLOT]),
            rest,
            "the second bone must keep the rest pose"
        );
        let mut poisoned = IDENTITY_SLOT;
        poisoned[0] = f32::NAN;
        assert_eq!(skin_positions(&model, &[poisoned]), rest, "a non-finite pose is absent, not fatal");
        let extras = [IDENTITY_SLOT, IDENTITY_SLOT, [9.0; 13], [f32::NAN; 13]];
        assert_eq!(skin_positions(&model, &extras), rest, "poses past the bone count are ignored");
    }

    #[test]
    fn a_single_bone_model_skins_through_the_same_path() {
        let model = single_bone_model();
        assert_eq!(
            skin_positions(&model, &[IDENTITY_SLOT]),
            vec![[11.0, 2.0, 3.0], [14.0, 5.0, 6.0]]
        );
        let posed = skin_positions(&model, &[slot_pose(R_X90, [0.0; 3])]);
        assert_eq!(posed, vec![[11.0, -3.0, 2.0], [14.0, -6.0, 5.0]]);
        assert_eq!(skin(&model, &[IDENTITY_SLOT]).iter().map(|(_, n)| *n).collect::<Vec<_>>(), vec![7, 9]);
    }

    #[test]
    fn pose_lines_parse_one_row_per_bone_and_refuse_the_rest() {
        let text = "# identity pose\n\n1 0 0 0 1 0 0 0 1 1 0 0 0\n";
        assert_eq!(parse_pose_lines(text), Ok(vec![IDENTITY_SLOT]));
        let short = "1 0 0 0 1 0 0 0 1 1 0 0";
        let long = "1 0 0 0 1 0 0 0 1 1 0 0 0 0";
        let text = "1 0 0 0 1 0 0 0 1 1 0 0 x";
        let infinite = "1 0 0 0 1 0 0 0 1 1 0 0 inf";
        for bad in [short, long, text, infinite] {
            assert!(parse_pose_lines(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn bounds_cover_every_axis_and_an_empty_mesh_has_none() {
        assert_eq!(bounds(&[]), None);
        let (minimum, maximum) = bounds(&[[1.0, -2.0, 3.0], [-4.0, 5.0, -6.0]]).expect("bounds");
        assert_eq!(minimum, [-4.0, -2.0, -6.0]);
        assert_eq!(maximum, [1.0, 5.0, 3.0]);
    }

    const R_Z90: Mat3 = [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
    const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    /// Three-bone chain: root at the origin, a middle bone 10 along X and a
    /// leaf 10 along Y, so a parent rotation visibly carries the child frames.
    fn chain_model() -> crate::object::ObjectModel {
        use crate::object::Bone;
        crate::object::ObjectModel {
            vertices: vec![([5, 0, 0], 0), ([0, 0, 0], 0), ([1, 0, 0], 0), ([0, 1, 0], 0)],
            bones: vec![
                Bone { first_vertex: 0, last_vertex: 0, local: [0, 0, 0], parent: u16::MAX },
                Bone { first_vertex: 1, last_vertex: 1, local: [10, 0, 0], parent: 0 },
                Bone { first_vertex: 2, last_vertex: 3, local: [0, 10, 0], parent: 1 },
            ],
            ..crate::object::ObjectModel::default()
        }
    }

    #[test]
    fn composing_a_chain_carries_the_child_world_origin_with_the_parent() {
        let hierarchy = BoneHierarchy::from_parents(vec![-1, 0]);
        let locals = [[0.0, 0.0, 0.0], [0.0, 10.0, 0.0]];
        let world = compose_with_locals(&hierarchy, &[slot_pose(R_Z90, [0.0; 3]), IDENTITY_SLOT], &locals);
        assert_eq!(world[0].rotation, R_Z90);
        assert_eq!(world[1].rotation, R_Z90, "the child frame inherits the parent rotation");
        close(world[1].translation, [-10.0, 0.0, 0.0]);
        // The pose-only composer carries a child pose translation the same way.
        let poses = [
            slot_pose(R_Z90, [0.0; 3]),
            slot_pose(IDENTITY, [0.0, 10.0, 0.0]),
        ];
        close(compose(&hierarchy, &poses)[1].translation, [-10.0, 0.0, 0.0]);
    }

    #[test]
    fn composition_resolves_by_index_not_array_or_slice_order() {
        // Child stored before its parent; poses indexed by bone, not by order.
        let hierarchy = BoneHierarchy::from_parents(vec![1, -1]);
        let poses = [IDENTITY_SLOT, slot_pose(R_Z90, [0.0; 3])];
        let locals = [[0.0, 10.0, 0.0], [0.0, 0.0, 0.0]];
        let world = compose_with_locals(&hierarchy, &poses, &locals);
        assert_eq!(world[1].rotation, R_Z90);
        close(world[0].translation, [-10.0, 0.0, 0.0]);
        // Pose index 1 drives bone 1; bone 0's identity pose leaves it alone.
        let forward = compose(&BoneHierarchy::from_parents(vec![-1, 0]), &poses);
        assert_eq!(forward[0].rotation, IDENTITY);
        assert_eq!(forward[1].rotation, R_Z90);
    }

    #[test]
    fn a_root_rotation_moves_the_whole_model_and_a_leaf_only_its_range() {
        let model = chain_model();
        let hierarchy = BoneHierarchy::from_parents(vec![-1, 0, 1]);
        let rest = crate::object::rest_positions(&model);
        let root_pose = [slot_pose(R_Z90, [0.0; 3]), IDENTITY_SLOT, IDENTITY_SLOT];
        let posed = skin_positions_hierarchical(&model, &hierarchy, &root_pose).expect("root pose");
        for (before, after) in rest.iter().zip(&posed) {
            assert_ne!(before, after, "a root rotation must move every vertex");
        }
        let leaf_pose = [IDENTITY_SLOT, IDENTITY_SLOT, slot_pose(R_Z90, [0.0; 3])];
        let posed = skin_positions_hierarchical(&model, &hierarchy, &leaf_pose).expect("leaf pose");
        assert_eq!(&posed[..2], &rest[..2], "only the leaf range may move");
        assert_eq!(posed.iter().zip(&rest).filter(|(a, b)| a != b).count(), 2);
    }

    #[test]
    fn identity_and_missing_hierarchical_poses_reproduce_the_rest_pose() {
        let model = chain_model();
        let hierarchy = BoneHierarchy::from_parents(vec![-1, 0, 1]);
        let rest = crate::object::rest_positions(&model);
        assert_eq!(
            skin_positions_hierarchical(&model, &hierarchy, &[IDENTITY_SLOT; 3]).expect("identity"),
            rest
        );
        assert_eq!(
            skin_positions_hierarchical(&model, &hierarchy, &[]).expect("no poses"),
            rest
        );
        assert_eq!(
            skin_positions_hierarchical(&model, &hierarchy, &[IDENTITY_SLOT]).expect("short"),
            rest
        );
    }

    #[test]
    fn a_single_root_bone_skins_hierarchically() {
        let model = single_bone_model();
        let hierarchy = BoneHierarchy::from_parents(vec![-1]);
        let posed = skin_positions_hierarchical(&model, &hierarchy, &[slot_pose(R_X90, [0.0; 3])])
            .expect("single root");
        assert_eq!(posed, vec![[11.0, -3.0, 2.0], [14.0, -6.0, 5.0]]);
    }

    #[test]
    fn composition_is_cycle_safe_without_validation() {
        let world = compose(&BoneHierarchy::from_parents(vec![1, 0]), &[IDENTITY_SLOT; 2]);
        assert_eq!(world.len(), 2, "an unvalidated cycle must not hang");
        let model = chain_model();
        let short = BoneHierarchy::from_parents(vec![-1, 0]);
        assert!(skin_positions_hierarchical(&model, &short, &[]).is_err());
    }

    fn matrix_close(actual: Mat3, expected: Mat3, tolerance: f32) {
        for row in 0..3 {
            for col in 0..3 {
                assert!(
                    (actual[row][col] - expected[row][col]).abs() <= tolerance,
                    "[{row}][{col}]: {} != {} (tolerance {tolerance})",
                    actual[row][col],
                    expected[row][col]
                );
            }
        }
    }

    #[test]
    fn binary_angle_units_are_4096_per_turn() {
        assert_eq!(binary_angle_to_radians(0.0), 0.0);
        let quarter = binary_angle_to_radians(1024.0);
        assert!((quarter - std::f32::consts::FRAC_PI_2).abs() < 1e-6, "{quarter}");
        let half = binary_angle_to_radians(2048.0);
        assert!((half - std::f32::consts::PI).abs() < 1e-6, "{half}");
        let turn = binary_angle_to_radians(4096.0);
        assert!((turn - std::f32::consts::TAU).abs() < 1e-5, "{turn}");
        assert!((binary_angle_to_radians(-4096.0) + std::f32::consts::TAU).abs() < 1e-5);
    }

    #[test]
    fn rotation_from_binary_angles_is_identity_at_zero() {
        assert_eq!(rotation_from_binary_angles(0.0, 0.0, 0.0), IDENTITY);
    }

    #[test]
    fn rotation_from_binary_angles_matches_each_single_axis_closed_form() {
        for units in [-1536.0f32, -512.0, 128.0, 1024.0, 2047.0] {
            let (s, c) = binary_angle_to_radians(units).sin_cos();
            let tolerance = 1e-6;
            matrix_close(
                rotation_from_binary_angles(units, 0.0, 0.0),
                [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
                tolerance,
            );
            matrix_close(
                rotation_from_binary_angles(0.0, units, 0.0),
                [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
                tolerance,
            );
            matrix_close(
                rotation_from_binary_angles(0.0, 0.0, units),
                [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
                tolerance,
            );
        }
    }

    #[test]
    fn rotation_from_binary_angles_advances_monotonically_per_axis() {
        let mut previous_x = -1.0f32;
        let mut previous_y = -1.0f32;
        let mut previous_z = -1.0f32;
        for units in 0..=1024 {
            let angle = binary_angle_to_radians(units as f32);
            let x = rotation_from_binary_angles(units as f32, 0.0, 0.0);
            let y = rotation_from_binary_angles(0.0, units as f32, 0.0);
            let z = rotation_from_binary_angles(0.0, 0.0, units as f32);
            let recovered = [
                x[2][1].atan2(x[1][1]),
                y[0][2].atan2(y[0][0]),
                z[1][0].atan2(z[0][0]),
            ];
            for (index, value) in recovered.iter().enumerate() {
                let previous = [previous_x, previous_y, previous_z][index];
                assert!(*value > previous, "axis {index} stopped advancing at {units} units");
                assert!((value - angle).abs() < 1e-5, "axis {index} at {units} units");
            }
            previous_x = recovered[0];
            previous_y = recovered[1];
            previous_z = recovered[2];
        }
    }

    #[test]
    fn rotation_from_binary_angles_matches_observed_runtime_matrices() {
        // Observed triples and their runtime 3x3 blocks. The capture records six
        // decimals, so the tolerance is 1e-5; the fitted model agrees with the
        // unrounded runtime matrices to about 5e-7 (docs/notes/rotation-model.md).
        let rows: [([f32; 3], [f32; 9]); 3] = [
            (
                [-286.230957, -146.703003, -83.153755],
                [
                    0.966866, 0.209233, -0.146261, -0.124003, 0.885725, 0.447341, 0.223145,
                    -0.414382, 0.882323,
                ],
            ),
            (
                [-869.289001, 19.552134, 66.055061],
                [
                    0.994423, -0.052780, -0.091305, 0.101108, 0.230948, 0.967698, -0.029988,
                    -0.971534, 0.234996,
                ],
            ),
            (
                [-104.001480, 703.635010, 103.602119],
                [
                    0.465944, -0.294541, 0.834351, 0.074679, 0.952694, 0.294614, -0.881657,
                    -0.074965, 0.465898,
                ],
            ),
        ];
        for (row, (triple, expected)) in rows.iter().enumerate() {
            let m = rotation_from_binary_angles(triple[0], triple[1], triple[2]);
            let flat = [
                m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2],
            ];
            for (index, (actual, expected)) in flat.iter().zip(expected).enumerate() {
                assert!(
                    (actual - expected).abs() < 1e-5,
                    "row {row} element {index}: {actual} != {expected}"
                );
            }
        }
    }
}
