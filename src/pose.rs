//! Pure one-bone pose math for decoded `.SRM` skeletons (no I/O).
//!
//! An `extra = 0` model stores 128 joint positions as 32 records of four `vec3` each,
//! plus three `(index, weight)` pairs per vertex. It has no stored per-slot matrix,
//! no hierarchy, and no clips, so a "joint transform" is built here as a rotation
//! about that joint's head. Joints are mapped to mesh space by negating Z
//! (`docs/changes/animation/design.md`, P5); vertices are already mesh space.
//! Weights blend linearly and are divided by the actual weight sum. Tests use
//! synthetic data only.

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
        let d = sub(posed, *p);
        if dot(d, d).sqrt() > EPS {
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
}
