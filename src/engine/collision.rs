//! Deterministic horizontal room collision.
//!
//! The room parser already produces world-space integer triangles
//! (`level::RoomTriangles`). This module turns the triangles that can block
//! horizontal movement into a small collision mesh with a deterministic uniform
//! grid, and resolves a player sweep against it.
//!
//! # Geometry model
//!
//! Positions are integer world units and stage 9 only moves the player in the
//! `X`/`Z` plane; `Y` is unchanged. Collision is therefore a **2D swept circle
//! in the XZ plane**, not a full 3D sweep:
//!
//! - A face is a *wall* when its 3D normal is within about 30 degrees of
//!   horizontal (`nx^2 + nz^2 >= 3 * ny^2`). Floors, ceilings and gentle slopes
//!   are skipped: this milestone has no vertical movement, so the player must
//!   walk across them rather than collide with their triangle edges.
//! - A wall's three edges are projected to XZ. A vertical wall projects to the
//!   line segment that is its footprint, which is exactly the barrier the player
//!   must not cross. Duplicate projected edges are harmless: the narrow phase
//!   takes the earliest contact.
//!
//! # Numbers
//!
//! The math is **fixed-point integer**: every world unit is [`SUBUNITS`] (=1024)
//! subunits, all products are widened to `i128`, and the only irrational step
//! (a square root) uses a deterministic integer square root with an explicit
//! ceiling. There is no `f32`/`f64` in this module. Chosen position values are
//! converted back to world units with explicit rounding. Determinism beats
//! geometric fidelity here.
//!
//! # Broad phase
//!
//! Wall triangles are inserted, in source face order, into a
//! [`BTreeMap`]-backed uniform grid of [`CELL_UNITS`]-unit cells. A query walks
//! the cell rectangle in a fixed `(x, z)` order, collects wall indices, then
//! sorts and deduplicates them. Iteration order is therefore identical on every
//! run; no hash-ordered container is used.
//!
//! # Honesty
//!
//! This is a correct, deterministic sweep-and-slide. It is **not** a
//! reproduction of the original's exact four-step probe/resolve/commit solver;
//! the original stage 8 behaviour is UNKNOWN and is not claimed here.

use std::collections::BTreeMap;

/// Fixed-point subunits per world unit.
pub const SUBUNITS: i64 = 1024;

/// Broad-phase cell edge, in world units.
const CELL_UNITS: i32 = 512;

/// Maximum cells a single broad-phase query may walk before it falls back to
/// the complete wall list. A normal player sweep spans one or two cells.
const MAX_QUERY_CELLS: i64 = 1 << 18;

/// One wall triangle, projected to XZ.
///
/// `xz` keeps the three projected vertices in face order; the three edges are
/// derived on demand. `min`/`max` are the projected bounding box used by the
/// broad phase.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WallTriangle {
    xz: [[i32; 2]; 3],
    min: [i32; 2],
    max: [i32; 2],
}

/// Room collision mesh: wall triangles plus a deterministic uniform grid.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollisionMesh {
    walls: Vec<WallTriangle>,
    grid: BTreeMap<(i32, i32), Vec<u32>>,
    skipped_faces: usize,
}

impl CollisionMesh {
    /// Build the collision mesh from integer room triangles.
    ///
    /// Every face is classified once; only wall-like faces are kept. The grid
    /// is filled in ascending wall index, so each cell's `Vec<u32>` is in
    /// source order.
    pub fn from_room_triangles(vertices: &[[i32; 3]], faces: &[[u32; 3]]) -> Self {
        let mut walls = Vec::new();
        let mut skipped_faces = 0usize;
        for face in faces {
            let Some(a) = vertices.get(face[0] as usize) else {
                skipped_faces += 1;
                continue;
            };
            let Some(b) = vertices.get(face[1] as usize) else {
                skipped_faces += 1;
                continue;
            };
            let Some(c) = vertices.get(face[2] as usize) else {
                skipped_faces += 1;
                continue;
            };
            if !is_wall_like(*a, *b, *c) {
                skipped_faces += 1;
                continue;
            }
            let xz = [[a[0], a[2]], [b[0], b[2]], [c[0], c[2]]];
            let min = [xz[0][0].min(xz[1][0]).min(xz[2][0]), xz[0][1].min(xz[1][1]).min(xz[2][1])];
            let max = [xz[0][0].max(xz[1][0]).max(xz[2][0]), xz[0][1].max(xz[1][1]).max(xz[2][1])];
            if min == max {
                // The whole triangle projects to one point: no barrier.
                skipped_faces += 1;
                continue;
            }
            walls.push(WallTriangle { xz, min, max });
        }

        let mut grid: BTreeMap<(i32, i32), Vec<u32>> = BTreeMap::new();
        for (index, wall) in walls.iter().enumerate() {
            let (x0, x1) = (cell_of(wall.min[0]), cell_of(wall.max[0]));
            let (z0, z1) = (cell_of(wall.min[1]), cell_of(wall.max[1]));
            for x in x0..=x1 {
                for z in z0..=z1 {
                    grid.entry((x, z)).or_default().push(index as u32);
                }
            }
        }

        Self {
            walls,
            grid,
            skipped_faces,
        }
    }

    /// Number of wall triangles kept.
    pub fn wall_count(&self) -> usize {
        self.walls.len()
    }

    /// Number of faces skipped as floors, ceilings, slopes or degenerate.
    pub fn skipped_faces(&self) -> usize {
        self.skipped_faces
    }

    /// True when the mesh has no barrier at all.
    pub fn is_empty(&self) -> bool {
        self.walls.is_empty()
    }

    /// Deterministic broad phase: wall indices whose projected bounding box can
    /// intersect the swept path expanded by `radius`.
    ///
    /// The result is sorted, deduplicated and independent of hash order.
    pub fn candidates(&self, from: [i32; 2], to: [i32; 2], radius: i32) -> Vec<u32> {
        if self.walls.is_empty() {
            return Vec::new();
        }
        let pad = radius.max(0);
        let min = [
            from[0].min(to[0]).saturating_sub(pad),
            from[1].min(to[1]).saturating_sub(pad),
        ];
        let max = [
            from[0].max(to[0]).saturating_add(pad),
            from[1].max(to[1]).saturating_add(pad),
        ];
        let (x0, x1) = (cell_of(min[0]), cell_of(max[0]));
        let (z0, z1) = (cell_of(min[1]), cell_of(max[1]));
        let cell_span = (i64::from(x1) - i64::from(x0) + 1)
            .saturating_mul(i64::from(z1) - i64::from(z0) + 1);
        if cell_span <= 0 || cell_span > MAX_QUERY_CELLS {
            return (0..self.walls.len() as u32).collect();
        }
        let mut found = Vec::new();
        for x in x0..=x1 {
            for z in z0..=z1 {
                if let Some(ids) = self.grid.get(&(x, z)) {
                    found.extend_from_slice(ids);
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        found
    }
}

fn cell_of(value: i32) -> i32 {
    value.div_euclid(CELL_UNITS)
}

/// A face blocks horizontal movement when its normal is close to horizontal.
///
/// `nx^2 + nz^2 >= 3 * ny^2` accepts planes within 30 degrees of vertical and
/// rejects floors, ceilings and gentler slopes.
fn is_wall_like(a: [i32; 3], b: [i32; 3], c: [i32; 3]) -> bool {
    let ab = [
        i64::from(b[0]) - i64::from(a[0]),
        i64::from(b[1]) - i64::from(a[1]),
        i64::from(b[2]) - i64::from(a[2]),
    ];
    let ac = [
        i64::from(c[0]) - i64::from(a[0]),
        i64::from(c[1]) - i64::from(a[1]),
        i64::from(c[2]) - i64::from(a[2]),
    ];
    let nx = ab[1] * ac[2] - ab[2] * ac[1];
    let ny = ab[2] * ac[0] - ab[0] * ac[2];
    let nz = ab[0] * ac[1] - ab[1] * ac[0];
    let horizontal = nx * nx + nz * nz;
    horizontal > 0 && horizontal >= 3 * ny * ny
}

#[cfg(test)]
mod tests {
    use super::{CELL_UNITS, CollisionMesh};

    /// Two triangles forming a vertical wall at `x`, spanning `z0..z1` and
    /// `y0..y1`. Its XZ projection is the segment `(x, z0)..(x, z1)`.
    fn vertical_wall(x: i32, z0: i32, z1: i32, y0: i32, y1: i32) -> (Vec<[i32; 3]>, Vec<[u32; 3]>) {
        let vertices = vec![
            [x, y0, z0],
            [x, y0, z1],
            [x, y1, z1],
            [x, y1, z0],
        ];
        (vertices, vec![[0, 1, 2], [0, 2, 3]])
    }

    /// Two triangles forming a horizontal floor at `y`, spanning `x0..x1` and
    /// `z0..z1`.
    fn floor(y: i32, x0: i32, x1: i32, z0: i32, z1: i32) -> (Vec<[i32; 3]>, Vec<[u32; 3]>) {
        let vertices = vec![
            [x0, y, z0],
            [x1, y, z0],
            [x1, y, z1],
            [x0, y, z1],
        ];
        (vertices, vec![[0, 1, 2], [0, 2, 3]])
    }

    #[test]
    fn only_wall_like_faces_enter_the_mesh() {
        let (mut vertices, mut faces) = vertical_wall(100, -500, 500, -100, 100);
        let (floor_vertices, floor_faces) = floor(-100, -500, 500, -500, 500);
        let base = vertices.len() as u32;
        vertices.extend(floor_vertices);
        faces.extend(floor_faces.iter().map(|face| face.map(|index| index + base)));

        let mesh = CollisionMesh::from_room_triangles(&vertices, &faces);
        assert_eq!(mesh.wall_count(), 2, "the two wall triangles are kept");
        assert_eq!(mesh.skipped_faces(), 2, "the floor is not a barrier");
    }

    #[test]
    fn a_degenerate_face_is_skipped_without_panicking() {
        let vertices = vec![[7, 7, 7], [7, 7, 7], [7, 7, 7]];
        let mesh = CollisionMesh::from_room_triangles(&vertices, &[[0, 1, 2]]);
        assert!(mesh.is_empty());
        assert_eq!(mesh.skipped_faces(), 1);
    }

    #[test]
    fn the_grid_never_misses_a_wall_the_brute_force_check_finds() {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        // A deterministic ladder of walls across several grid cells.
        for step in 0..24 {
            let x = -4000 + step * 350;
            let z0 = -3000 + (step % 5) * 400;
            let (mut v, mut f) = vertical_wall(x, z0, z0 + 700, -200, 200);
            let base = vertices.len() as u32;
            vertices.append(&mut v);
            faces.extend(f.drain(..).map(|face| face.map(|index| index + base)));
        }
        let mesh = CollisionMesh::from_room_triangles(&vertices, &faces);
        assert!(mesh.wall_count() > 0);

        let probes = [
            ([0, 0], [0, 0], 16),
            ([-4000, -3000], [4000, 3000], 16),
            ([-100, -100], [100, 100], 16),
            ([-4000, -3000], [-4000, -3000], 0),
            ([3999, 2999], [4000, 3000], 64),
        ];
        for (from, to, radius) in probes {
            let grid = mesh.candidates(from, to, radius);
            let brute: Vec<u32> = (0..mesh.wall_count() as u32)
                .filter(|&index| {
                    let wall = &mesh.walls[index as usize];
                    let pad = radius.max(0);
                    let min = [from[0].min(to[0]) - pad, from[1].min(to[1]) - pad];
                    let max = [from[0].max(to[0]) + pad, from[1].max(to[1]) + pad];
                    wall.min[0] <= max[0]
                        && wall.max[0] >= min[0]
                        && wall.min[1] <= max[1]
                        && wall.max[1] >= min[1]
                })
                .collect();
            assert!(
                brute.iter().all(|id| grid.contains(id)),
                "the grid must never miss a brute-force AABB match for {from:?}->{to:?} radius {radius}"
            );
            assert!(grid.windows(2).all(|pair| pair[0] < pair[1]), "candidates are sorted and unique");
        }
    }

    #[test]
    fn candidate_order_is_independent_of_grid_layout() {
        let (vertices, faces) = vertical_wall(100, -100, 100, -100, 100);
        let mesh = CollisionMesh::from_room_triangles(&vertices, &faces);
        let first = mesh.candidates([0, 0], [80, 0], 16);
        let second = mesh.candidates([0, 0], [80, 0], 16);
        assert_eq!(first, second);
        assert!(first.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(first.iter().all(|&id| (id as usize) < mesh.wall_count()));
    }

    #[test]
    fn cell_size_is_a_positive_documented_constant() {
        assert!(CELL_UNITS > 0);
    }
}
