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
//! # Vertical movement (gravity, ground and landing)
//!
//! Faces are classified once by the direction of their geometric normal:
//!
//! - **wall**: the normal is within about 30 degrees of horizontal
//!   (`nx^2 + nz^2 >= 3 * ny^2`), the existing horizontal barrier;
//! - **walkable floor**: the normal points up and is within about 30 degrees of
//!   vertical (`ny > 0 && ny^2 >= 3 * (nx^2 + nz^2)`); gentle slopes count as
//!   walkable;
//! - **ceiling**: the same near-vertical test with the normal pointing down
//!   (`ny < 0`); ceilings are classified but not resolved here because this
//!   milestone has no upward motion (see below);
//! - **other**: steeper slopes and degenerate faces are kept out.
//!
//! The winding sign is the only normal-direction source the parser provides, and
//! a plane supports a body from either side. Ground detection therefore uses
//! **every** near-horizontal triangle, floor-facing or ceiling-facing: the
//! up/down split is a diagnostic, not a decision about which triangles can be
//! ground. A ceiling-facing triangle above the player is never selected because
//! [`CollisionMesh::ground_height`] only looks at or below the feet.
//!
//! Ground detection is a deterministic integer point query over the floor
//! triangles. [`CollisionMesh::ground_height`] returns the highest walkable
//! surface at `(x, z)` that is not above the query ceiling, and
//! [`CollisionMesh::vertical_step`] integrates one gravity tick and either lands
//! the player exactly on that surface (stopping the fall) or lets them keep
//! falling. Because the player's `Y` is an integer world unit, a grounded player
//! rests exactly on the surface every tick and cannot jitter or sink.
//!
//! Gravity is level-geometry-driven: a mesh with **no** floor triangles (the
//! synthetic fixtures, or a unit without geometry) leaves `Y` untouched. That
//! keeps no-geometry worlds free of a meaningless ground plane and keeps the
//! synthetic determinism goldens stable.
//!
//! Ceilings block nothing in this slice: the only vertical producer is gravity,
//! so the player never moves upward. Ceiling triangles are still classified so
//! the next slice can resolve them without a second parse.
//!
//! # Honesty
//!
//! This is a correct, deterministic sweep-and-slide plus a gravity/ground
//! resolver. It is **not** a reproduction of the original's exact four-step
//! probe/resolve/commit solver; the original stage 8 behaviour is UNKNOWN and is
//! not claimed here. The gravity constants and the player height are documented
//! synthetic values, not retail values.

use std::collections::BTreeMap;

/// Fixed-point subunits per world unit.
pub const SUBUNITS: i64 = 1024;

/// Broad-phase cell edge, in world units.
const CELL_UNITS: i32 = 512;

/// Maximum cells a single broad-phase query may walk before it falls back to
/// the complete wall list. A normal player sweep spans one or two cells.
const MAX_QUERY_CELLS: i64 = 1 << 18;

/// Maximum slide iterations for one sweep.
const MAX_SLIDE_ITERATIONS: usize = 4;

/// Extra precision used when projecting a leftover delta onto a tangent.
const PROJECT_SCALE: i128 = 1 << 20;

/// Gravity acceleration, in fixed-point subunits per tick squared.
///
/// One world unit per tick squared is a documented synthetic value; the
/// original acceleration is UNKNOWN and is not claimed.
pub const GRAVITY_SUBUNITS_PER_TICK2: i64 = SUBUNITS;

/// Terminal downward speed, in fixed-point subunits per tick (16 world units).
///
/// The cap bounds the per-tick delta and keeps a long fall from overflowing.
pub const MAX_FALL_SUBUNITS_PER_TICK: i64 = 16 * SUBUNITS;

/// Largest upward step a grounded player snaps onto in one tick.
///
/// A walkable slope may stand a few units higher between two ticks; this
/// tolerance keeps the player on the slope instead of treating it as a ledge.
pub const MAX_STEP_UP_UNITS: i32 = 4;

/// One floor triangle kept for ground detection, with its geometric normal.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FloorTriangle {
    vertices: [[i32; 3]; 3],
    normal: [i64; 3],
}

impl FloorTriangle {
    /// Surface height at the fixed-point point `(px, pz)`, or `None` when the
    /// point is outside the projected triangle.
    ///
    /// The point-in-triangle test is the sign of three edge functions in the
    /// `XZ` plane. The surface height comes from the triangle plane:
    /// `y = ay - (nx * (x - ax) + nz * (z - az)) / ny`. Division is integer
    /// truncation, which is deterministic; `ny` is non-zero for a floor.
    fn height_at(&self, px: i64, pz: i64) -> Option<i64> {
        let [a, b, c] = self.vertices;
        let ax = i64::from(a[0]) * SUBUNITS;
        let az = i64::from(a[2]) * SUBUNITS;
        let bx = i64::from(b[0]) * SUBUNITS;
        let bz = i64::from(b[2]) * SUBUNITS;
        let cx = i64::from(c[0]) * SUBUNITS;
        let cz = i64::from(c[2]) * SUBUNITS;

        let mut positive = false;
        let mut negative = false;
        for (x0, z0, x1, z1) in [(ax, az, bx, bz), (bx, bz, cx, cz), (cx, cz, ax, az)] {
            let edge = i128::from(x1 - x0) * i128::from(pz - z0)
                - i128::from(z1 - z0) * i128::from(px - x0);
            if edge > 0 {
                positive = true;
            } else if edge < 0 {
                negative = true;
            }
        }
        // Mixed signs: outside. All zero: the triangle projects to a line, so it
        // has no surface.
        if (positive && negative) || (!positive && !negative) {
            return None;
        }

        let [nx, ny, nz] = self.normal;
        if ny == 0 {
            return None;
        }
        let ay = i64::from(a[1]) * SUBUNITS;
        let numerator = i128::from(nx) * i128::from(px - ax)
            + i128::from(nz) * i128::from(pz - az);
        Some(saturate_i64(i128::from(ay) - numerator / i128::from(ny)))
    }
}

/// Result of one vertical tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerticalStep {
    /// New feet height in integer world units.
    pub y: i32,
    /// Downward velocity after the step, in fixed-point subunits per tick.
    /// Negative means falling; zero means grounded.
    pub velocity_subunits: i64,
    /// True when the step ended resting on a walkable floor.
    pub grounded: bool,
}

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

/// Room collision mesh: wall triangles plus a deterministic uniform grid, and
/// the horizontal (near-horizontal) triangles used by the vertical resolver.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollisionMesh {
    walls: Vec<WallTriangle>,
    /// Every near-horizontal triangle, regardless of which way its normal
    /// points. A plane supports a body from either side and the shipped mesh
    /// carries no explicit up flag, so ground detection uses this whole set.
    surfaces: Vec<FloorTriangle>,
    up_facing: usize,
    down_facing: usize,
    grid: BTreeMap<(i32, i32), Vec<u32>>,
    skipped_faces: usize,
}

impl CollisionMesh {
    /// Build the collision mesh from integer room triangles.
    ///
    /// Every face is classified once from its geometric normal: walls feed the
    /// horizontal grid, floors feed ground detection, ceilings are counted, and
    /// steeper slopes or degenerate faces are skipped. The grid is filled in
    /// ascending wall index, so each cell's `Vec<u32>` is in source order.
    pub fn from_room_triangles(vertices: &[[i32; 3]], faces: &[[u32; 3]]) -> Self {
        let mut walls = Vec::new();
        let mut surfaces = Vec::new();
        let mut up_facing = 0usize;
        let mut down_facing = 0usize;
        let mut skipped_faces = 0usize;
        for face in faces {
            let (Some(a), Some(b), Some(c)) = (
                vertices.get(face[0] as usize),
                vertices.get(face[1] as usize),
                vertices.get(face[2] as usize),
            ) else {
                skipped_faces += 1;
                continue;
            };
            match classify_face(*a, *b, *c) {
                FaceClass::Wall => {
                    let xz = [[a[0], a[2]], [b[0], b[2]], [c[0], c[2]]];
                    let min = [
                        xz[0][0].min(xz[1][0]).min(xz[2][0]),
                        xz[0][1].min(xz[1][1]).min(xz[2][1]),
                    ];
                    let max = [
                        xz[0][0].max(xz[1][0]).max(xz[2][0]),
                        xz[0][1].max(xz[1][1]).max(xz[2][1]),
                    ];
                    if min == max {
                        // The whole triangle projects to one point: no barrier.
                        skipped_faces += 1;
                        continue;
                    }
                    walls.push(WallTriangle { xz, min, max });
                }
                FaceClass::Floor => {
                    up_facing += 1;
                    surfaces.push(FloorTriangle {
                        vertices: [*a, *b, *c],
                        normal: face_normal(*a, *b, *c),
                    });
                    skipped_faces += 1;
                }
                FaceClass::Ceiling => {
                    down_facing += 1;
                    surfaces.push(FloorTriangle {
                        vertices: [*a, *b, *c],
                        normal: face_normal(*a, *b, *c),
                    });
                    skipped_faces += 1;
                }
                FaceClass::Other => skipped_faces += 1,
            }
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
            surfaces,
            up_facing,
            down_facing,
            grid,
            skipped_faces,
        }
    }

    /// Number of wall triangles kept.
    pub fn wall_count(&self) -> usize {
        self.walls.len()
    }

    /// Number of near-horizontal triangles whose normal points up.
    ///
    /// This is a diagnostic split. Ground detection uses every near-horizontal
    /// surface, because the winding sign is the only normal-direction source and
    /// a plane supports a body from either side.
    pub fn floor_count(&self) -> usize {
        self.up_facing
    }

    /// Number of near-horizontal triangles whose normal points down.
    ///
    /// Classified for the next slice's ceiling resolver; this slice never moves
    /// upward, so ceilings block nothing.
    pub fn ceiling_count(&self) -> usize {
        self.down_facing
    }

    /// Number of near-horizontal surfaces usable as ground or ceiling.
    pub fn surface_count(&self) -> usize {
        self.surfaces.len()
    }

    /// True when the mesh carries at least one near-horizontal surface, i.e. the
    /// level supplies ground geometry and gravity may act.
    pub fn has_ground(&self) -> bool {
        !self.surfaces.is_empty()
    }

    /// Number of faces not kept as horizontal wall barriers: floors, ceilings,
    /// steeper slopes and degenerate faces.
    pub fn skipped_faces(&self) -> usize {
        self.skipped_faces
    }

    /// True when the mesh has no wall barrier at all.
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

    /// Resolve one horizontal movement against the mesh and return the end
    /// position.
    ///
    /// `from` and the returned position are integer world units. Only `X` and
    /// `Z` are resolved; `Y` is translated directly because this milestone has
    /// no vertical movement. When the mesh is empty, or the delta has no `X`/`Z`
    /// component, the result is exactly `from + delta`: a zero-input tick never
    /// moves or collides.
    ///
    /// The sweep is a fixed-point 2D circle cast in XZ. It stops before the
    /// earliest contact and then projects the leftover delta onto that contact's
    /// tangent, so a diagonal push slides along the wall. At most
    /// [`MAX_SLIDE_ITERATIONS`] contacts are resolved, which bounds the tick
    /// cost and keeps the result deterministic.
    pub fn sweep(&self, from: [i32; 3], delta: [i32; 3], radius: i32) -> [i32; 3] {
        let direct = add_world(from, delta);
        if self.walls.is_empty() || (delta[0] == 0 && delta[2] == 0) {
            return direct;
        }

        let radius = radius.max(0);
        let fixed_radius = i64::from(radius) * SUBUNITS;
        let mut current = [i64::from(from[0]) * SUBUNITS, i64::from(from[2]) * SUBUNITS];
        let mut remaining = [i64::from(delta[0]) * SUBUNITS, i64::from(delta[2]) * SUBUNITS];

        for _ in 0..MAX_SLIDE_ITERATIONS {
            let candidate = [current[0] + remaining[0], current[1] + remaining[1]];
            let Some(contact) = self.first_contact(current, candidate, remaining, fixed_radius) else {
                current = candidate;
                break;
            };
            let resolved = advance(current, remaining, contact);
            let leftover = [candidate[0] - resolved[0], candidate[1] - resolved[1]];
            let slid = project_onto_tangent(leftover, contact.tangent);
            if resolved != current {
                current = resolved;
            }
            if slid[0] == 0 && slid[1] == 0 {
                break;
            }
            remaining = slid;
        }

        [to_world(current[0]), direct[1], to_world(current[1])]
    }

    /// Highest horizontal surface at `(x, z)` that is at or below `max_y`, in
    /// integer world units.
    ///
    /// Every near-horizontal triangle is queried, in source order; the result is
    /// a maximum, so iteration order cannot change it. A point outside every
    /// projected surface returns `None`, which is a ledge or a hole.
    pub fn ground_height(&self, x: i32, z: i32, max_y: i32) -> Option<i32> {
        if self.surfaces.is_empty() {
            return None;
        }
        let px = i64::from(x) * SUBUNITS;
        let pz = i64::from(z) * SUBUNITS;
        let mut best: Option<i32> = None;
        for floor in &self.surfaces {
            let Some(height) = floor.height_at(px, pz) else {
                continue;
            };
            let world = to_world(height);
            if world <= max_y && best.is_none_or(|current| world > current) {
                best = Some(world);
            }
        }
        best
    }

    /// Integrate one gravity tick and resolve the player against the floors.
    ///
    /// Downward velocity is negative and accumulates [`GRAVITY_SUBUNITS_PER_TICK2`]
    /// per tick, capped at [`MAX_FALL_SUBUNITS_PER_TICK`]. The candidate height
    /// is `from.y + velocity`; when it reaches a floor at or below the feet (up
    /// to [`MAX_STEP_UP_UNITS`] above them, so a walkable slope does not read as
    /// a ledge), the player lands exactly on that surface, the velocity resets to
    /// zero and `grounded` is `true`. Otherwise the candidate stands and the
    /// fall continues.
    ///
    /// A mesh with no surfaces leaves `from.y` untouched and reports
    /// `grounded = false`: there is no ground geometry to resolve against.
    pub fn vertical_step(&self, from: [i32; 3], velocity_subunits: i64) -> VerticalStep {
        if self.surfaces.is_empty() {
            return VerticalStep {
                y: from[1],
                velocity_subunits: 0,
                grounded: false,
            };
        }
        let velocity = velocity_subunits
            .saturating_sub(GRAVITY_SUBUNITS_PER_TICK2)
            .max(-MAX_FALL_SUBUNITS_PER_TICK);
        let candidate_y = to_world(
            i64::from(from[1])
                .saturating_mul(SUBUNITS)
                .saturating_add(velocity),
        );
        let max_support = from[1].saturating_add(MAX_STEP_UP_UNITS);
        match self.ground_height(from[0], from[2], max_support) {
            Some(surface) if candidate_y <= surface => VerticalStep {
                y: surface,
                velocity_subunits: 0,
                grounded: true,
            },
            _ => VerticalStep {
                y: candidate_y,
                velocity_subunits: velocity,
                grounded: false,
            },
        }
    }

    /// Earliest contact along one horizontal sweep, using the broad phase.
    fn first_contact(
        &self,
        from: [i64; 2],
        to: [i64; 2],
        delta: [i64; 2],
        radius: i64,
    ) -> Option<Contact> {
        let world_radius = ((radius + SUBUNITS - 1) / SUBUNITS) as i32;
        let query_from = [to_world_floor(from[0]), to_world_floor(from[1])];
        let query_to = [to_world_ceil(to[0]), to_world_ceil(to[1])];
        let ids = self.candidates(query_from, query_to, world_radius);
        let mut best: Option<Contact> = None;
        for id in ids {
            let wall = &self.walls[id as usize];
            for edge in 0..3 {
                let a = wall.xz[edge];
                let b = wall.xz[(edge + 1) % 3];
                if let Some(contact) = edge_contact(from, delta, radius, a, b) {
                    if best.is_none_or(|current| contact.is_earlier_than(current)) {
                        best = Some(contact);
                    }
                }
            }
        }
        best
    }
}

/// One resolved contact: a rational `t = num/den` in `0..=1` plus the tangent
/// direction used to slide.
#[derive(Debug, Clone, Copy)]
struct Contact {
    num: i128,
    den: i128,
    tangent: [i128; 2],
}

impl Contact {
    fn is_earlier_than(self, other: Self) -> bool {
        self.num * other.den < other.num * self.den
    }
}

/// Earliest contact between a moving circle (center `p`, delta `d`, radius `r`)
/// and one projected wall edge `a..b`. All values are fixed-point subunits.
fn edge_contact(
    p: [i64; 2],
    d: [i64; 2],
    r: i64,
    a: [i32; 2],
    b: [i32; 2],
) -> Option<Contact> {
    let a = [i64::from(a[0]) * SUBUNITS, i64::from(a[1]) * SUBUNITS];
    let b = [i64::from(b[0]) * SUBUNITS, i64::from(b[1]) * SUBUNITS];
    let edge = [b[0] - a[0], b[1] - a[1]];
    let edge_len2 = i128::from(edge[0]) * i128::from(edge[0])
        + i128::from(edge[1]) * i128::from(edge[1]);
    if edge_len2 == 0 {
        return None;
    }
    let m = [p[0] - a[0], p[1] - a[1]];
    let normal = [-edge[1], edge[0]];
    let s0 = i128::from(m[0]) * i128::from(normal[0]) + i128::from(m[1]) * i128::from(normal[1]);
    let sd = i128::from(d[0]) * i128::from(normal[0]) + i128::from(d[1]) * i128::from(normal[1]);

    let mut best: Option<Contact> = None;

    // Infinite line: |sized distance| = r * |normal|.
    if sd != 0 {
        let normal_len = isqrt_ceil(edge_len2);
        let big_r = i128::from(r) * normal_len;
        let signed = if s0 > big_r {
            if sd >= 0 {
                None
            } else {
                Some((big_r - s0, sd))
            }
        } else if s0 < -big_r {
            if sd <= 0 {
                None
            } else {
                Some((-big_r - s0, sd))
            }
        } else {
            // Already within the radius slab: block only if moving deeper.
            let deeper = (s0 > 0 && sd < 0) || (s0 < 0 && sd > 0);
            deeper.then_some((0i128, 1i128))
        };
        if let Some((num, den)) = signed {
            let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
            if den > 0 && num >= 0 && num <= den {
                // Contact point must fall within the segment, not its infinite line.
                let dot_de = i128::from(d[0]) * i128::from(edge[0])
                    + i128::from(d[1]) * i128::from(edge[1]);
                let dot_me = i128::from(m[0]) * i128::from(edge[0])
                    + i128::from(m[1]) * i128::from(edge[1]);
                let u_num = dot_me * den + num * dot_de;
                if u_num >= 0 && u_num <= edge_len2 * den {
                    best = Some(Contact {
                        num,
                        den,
                        tangent: [i128::from(edge[0]), i128::from(edge[1])],
                    });
                }
            }
        }
    }

    // Endpoint caps: circle vs point at both ends of the segment.
    for point in [a, b] {
        let mq = [p[0] - point[0], p[1] - point[1]];
        let a_coef = i128::from(d[0]) * i128::from(d[0]) + i128::from(d[1]) * i128::from(d[1]);
        if a_coef == 0 {
            continue;
        }
        let b_coef = 2 * (i128::from(mq[0]) * i128::from(d[0]) + i128::from(mq[1]) * i128::from(d[1]));
        let c_coef = i128::from(mq[0]) * i128::from(mq[0])
            + i128::from(mq[1]) * i128::from(mq[1])
            - i128::from(r) * i128::from(r);
        let candidate = if c_coef <= 0 {
            // Already touching the cap: block now and let the tangent slide.
            Some((0i128, 1i128, [-i128::from(mq[1]), i128::from(mq[0])]))
        } else {
            let discriminant = b_coef * b_coef - 4 * a_coef * c_coef;
            if discriminant < 0 {
                None
            } else {
                let root = isqrt_ceil(discriminant);
                let num = -b_coef - root;
                let den = 2 * a_coef;
                let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
                if den > 0 && num >= 0 && num <= den {
                    let normal = [
                        i128::from(mq[0]) * den + num * i128::from(d[0]),
                        i128::from(mq[1]) * den + num * i128::from(d[1]),
                    ];
                    Some((num, den, [-normal[1], normal[0]]))
                } else {
                    None
                }
            }
        };
        if let Some((num, den, tangent)) = candidate {
            let contact = Contact {
                num,
                den,
                tangent,
            };
            if best.is_none_or(|current| contact.is_earlier_than(current)) {
                best = Some(contact);
            }
        }
    }

    best
}

/// Advance `p` by `t * d`, truncating toward zero so the result never passes
/// the contact point.
fn advance(p: [i64; 2], d: [i64; 2], contact: Contact) -> [i64; 2] {
    let sx = i128::from(d[0]) * contact.num / contact.den;
    let sz = i128::from(d[1]) * contact.num / contact.den;
    [
        p[0].saturating_add(saturate_i64(sx)),
        p[1].saturating_add(saturate_i64(sz)),
    ]
}

/// Keep only the component of `v` along `tangent`.
fn project_onto_tangent(v: [i64; 2], tangent: [i128; 2]) -> [i64; 2] {
    let tt = tangent[0] * tangent[0] + tangent[1] * tangent[1];
    if tt == 0 {
        return [0, 0];
    }
    let vt = i128::from(v[0]) * tangent[0] + i128::from(v[1]) * tangent[1];
    let factor = vt * PROJECT_SCALE / tt;
    [
        saturate_i64(tangent[0] * factor / PROJECT_SCALE),
        saturate_i64(tangent[1] * factor / PROJECT_SCALE),
    ]
}

/// Deterministic integer square root, rounded up.
///
/// `isqrt_ceil` is used for every irrational length so contacts are computed
/// slightly early: the sweep stays strictly on the near side of the surface.
fn isqrt_ceil(value: i128) -> i128 {
    if value <= 0 {
        return 0;
    }
    let value = value as u128;
    let mut low = 0u128;
    let mut high = 1u128;
    while high.saturating_mul(high) < value {
        high <<= 1;
    }
    while low + 1 < high {
        let mid = (low + high) / 2;
        if mid * mid < value {
            low = mid;
        } else {
            high = mid;
        }
    }
    high as i128
}

fn add_world(from: [i32; 3], delta: [i32; 3]) -> [i32; 3] {
    [
        from[0].saturating_add(delta[0]),
        from[1].saturating_add(delta[1]),
        from[2].saturating_add(delta[2]),
    ]
}

/// Round a fixed-point value to the nearest world unit, half away from zero.
fn to_world(value: i64) -> i32 {
    let rounded = if value >= 0 {
        (value + SUBUNITS / 2) / SUBUNITS
    } else {
        (value - SUBUNITS / 2) / SUBUNITS
    };
    saturate_i32(i128::from(rounded))
}

fn to_world_floor(value: i64) -> i32 {
    saturate_i32(i128::from(value.div_euclid(SUBUNITS)))
}

fn to_world_ceil(value: i64) -> i32 {
    saturate_i32(i128::from((value + SUBUNITS - 1).div_euclid(SUBUNITS)))
}

fn saturate_i64(value: i128) -> i64 {
    value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn saturate_i32(value: i128) -> i32 {
    value.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

fn cell_of(value: i32) -> i32 {
    value.div_euclid(CELL_UNITS)
}

/// Geometric face normal (not normalized), `ab x ac`, in integer units.
fn face_normal(a: [i32; 3], b: [i32; 3], c: [i32; 3]) -> [i64; 3] {
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
    [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ]
}

/// How a room face participates in collision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaceClass {
    /// Near-vertical: a horizontal barrier.
    Wall,
    /// Near-horizontal with an upward normal: a walkable surface.
    Floor,
    /// Near-horizontal with a downward normal: a ceiling (classified, unused).
    Ceiling,
    /// Steeper slope or degenerate face: kept out.
    Other,
}

/// Classify a face by the direction of its geometric normal.
///
/// The wall test is the existing one: within about 30 degrees of vertical
/// (`horizontal >= 3 * ny^2`). The floor/ceiling tests are its mirror: within
/// about 30 degrees of vertical normal (`vertical >= 3 * horizontal`), split by
/// the sign of `ny`. Squared magnitudes use `i128` so large room coordinates
/// cannot overflow.
fn classify_face(a: [i32; 3], b: [i32; 3], c: [i32; 3]) -> FaceClass {
    let [nx, ny, nz] = face_normal(a, b, c);
    let horizontal =
        i128::from(nx) * i128::from(nx) + i128::from(nz) * i128::from(nz);
    let vertical = i128::from(ny) * i128::from(ny);
    if horizontal > 0 && horizontal >= 3 * vertical {
        FaceClass::Wall
    } else if vertical > 0 && vertical >= 3 * horizontal {
        if ny > 0 {
            FaceClass::Floor
        } else {
            FaceClass::Ceiling
        }
    } else {
        FaceClass::Other
    }
}

#[cfg(test)]
mod tests {
    use super::{CELL_UNITS, CollisionMesh, MAX_STEP_UP_UNITS};

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

    /// Two triangles forming a horizontal plane at `y`, spanning `x0..x1` and
    /// `z0..z1`. This winding yields a downward normal, so it classifies as a
    /// ceiling; it is used by the "floor is not a barrier" regression test.
    fn floor(y: i32, x0: i32, x1: i32, z0: i32, z1: i32) -> (Vec<[i32; 3]>, Vec<[u32; 3]>) {
        let vertices = vec![
            [x0, y, z0],
            [x1, y, z0],
            [x1, y, z1],
            [x0, y, z1],
        ];
        (vertices, vec![[0, 1, 2], [0, 2, 3]])
    }

    /// Two triangles forming a walkable floor at `y` with an upward normal,
    /// spanning `x0..x1` and `z0..z1`.
    fn floor_up(y: i32, x0: i32, x1: i32, z0: i32, z1: i32) -> (Vec<[i32; 3]>, Vec<[u32; 3]>) {
        let vertices = vec![
            [x0, y, z0],
            [x0, y, z1],
            [x1, y, z1],
            [x1, y, z0],
        ];
        (vertices, vec![[0, 1, 2], [0, 2, 3]])
    }

    fn append(vertices: &mut Vec<[i32; 3]>, faces: &mut Vec<[u32; 3]>, mut v: Vec<[i32; 3]>, mut f: Vec<[u32; 3]>) {
        let base = vertices.len() as u32;
        vertices.append(&mut v);
        faces.extend(f.drain(..).map(|face| face.map(|index| index + base)));
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

    fn wall_mesh() -> CollisionMesh {
        let (vertices, faces) = vertical_wall(100, -500, 500, -100, 100);
        CollisionMesh::from_room_triangles(&vertices, &faces)
    }

    #[test]
    fn an_empty_mesh_moves_freely() {
        let end = CollisionMesh::default().sweep([1, 2, 3], [4, -1, 4], 16);
        assert_eq!(end, [5, 1, 7]);
    }

    #[test]
    fn a_floor_only_mesh_does_not_block_horizontal_movement() {
        let (vertices, faces) = floor(-100, -500, 500, -500, 500);
        let mesh = CollisionMesh::from_room_triangles(&vertices, &faces);
        assert!(mesh.is_empty());
        assert_eq!(mesh.sweep([0, 0, 0], [40, 0, 0], 16), [40, 0, 0]);
    }

    #[test]
    fn a_sweep_stops_before_a_wall() {
        let mesh = wall_mesh();
        let end = mesh.sweep([0, 0, 0], [200, 0, 0], 16);
        let contact_x = 100 - 16;
        assert!(end[0] < 100, "the endpoint must stay on the near side: {end:?}");
        assert!(
            (contact_x - 2..=contact_x).contains(&end[0]),
            "the sweep stops at the radius, got {end:?}"
        );
        assert_eq!(end[1], 0);
        assert_eq!(end[2], 0);
    }

    #[test]
    fn repeated_forward_ticks_never_cross_the_wall() {
        let mesh = wall_mesh();
        let mut position = [0, 0, 0];
        for _ in 0..100 {
            position = mesh.sweep(position, [4, 0, 0], 16);
            assert!(position[0] < 100, "crossed the wall: {position:?}");
        }
        assert!((80..=84).contains(&position[0]), "final {position:?}");
    }

    #[test]
    fn a_diagonal_push_slides_along_the_wall_and_keeps_progress() {
        let mesh = wall_mesh();
        let mut position = [0, 0, 0];
        for _ in 0..60 {
            position = mesh.sweep(position, [4, 0, 4], 16);
            assert!(position[0] < 100, "crossed the wall: {position:?}");
        }
        assert!(
            position[2] > 200,
            "the push must keep making progress along the wall: {position:?}"
        );
        assert!((80..=84).contains(&position[0]), "final {position:?}");
    }

    #[test]
    fn the_same_sweep_script_replays_identically() {
        fn run() -> Vec<[i32; 3]> {
            let mesh = wall_mesh();
            let mut position = [0, 0, 0];
            let mut observed = Vec::new();
            for tick in 0..80 {
                let delta = match tick % 4 {
                    0 => [4, 0, 4],
                    1 => [4, 0, 0],
                    2 => [4, 0, -2],
                    _ => [0, 0, 4],
                };
                position = mesh.sweep(position, delta, 16);
                observed.push(position);
            }
            observed
        }
        assert_eq!(run(), run());
    }

    #[test]
    fn broad_phase_and_brute_force_choose_the_same_earliest_contact() {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for step in 0..24 {
            let x = -4000 + step * 350;
            let z0 = -3000 + (step % 5) * 400;
            let (mut v, mut f) = vertical_wall(x, z0, z0 + 700, -200, 200);
            let base = vertices.len() as u32;
            vertices.append(&mut v);
            faces.extend(f.drain(..).map(|face| face.map(|index| index + base)));
        }
        let mesh = CollisionMesh::from_room_triangles(&vertices, &faces);
        let probes: [([i64; 2], [i64; 2], i64); 5] = [
            ([0, 0], [4 * 1024, 4 * 1024], 16 * 1024),
            ([-4000 * 1024, 0], [4000 * 1024, 0], 16 * 1024),
            ([100 * 1024, 0], [200 * 1024, 0], 16 * 1024),
            ([-1000 * 1024, 1000 * 1024], [0, 0], 8 * 1024),
            ([3000 * 1024, -3000 * 1024], [3000 * 1024, 3000 * 1024], 32 * 1024),
        ];
        for (from, delta, radius) in probes {
            let to = [from[0] + delta[0], from[1] + delta[1]];
            let grid = mesh.first_contact(from, to, delta, radius);
            let mut brute: Option<super::Contact> = None;
            for wall in &mesh.walls {
                for edge in 0..3 {
                    let a = wall.xz[edge];
                    let b = wall.xz[(edge + 1) % 3];
                    if let Some(contact) = super::edge_contact(from, delta, radius, a, b)
                        && brute.is_none_or(|current| contact.is_earlier_than(current))
                    {
                        brute = Some(contact);
                    }
                }
            }
            match (grid, brute) {
                (Some(a), Some(b)) => {
                    assert_eq!((a.num, a.den), (b.num, b.den), "probe {from:?}->{to:?}");
                }
                (None, None) => {}
                other => panic!("broad and narrow phase disagree for {from:?}->{to:?}: {other:?}"),
            }
        }
    }

    #[test]
    fn faces_are_classified_as_wall_floor_ceiling_or_other() {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        let (v, f) = vertical_wall(100, -100, 100, -100, 100);
        append(&mut vertices, &mut faces, v, f);
        let (v, f) = floor_up(0, -100, 100, -100, 100);
        append(&mut vertices, &mut faces, v, f);
        let (v, f) = floor(200, -100, 100, -100, 100);
        append(&mut vertices, &mut faces, v, f);

        let mesh = CollisionMesh::from_room_triangles(&vertices, &faces);
        assert_eq!(mesh.wall_count(), 2, "the vertical quad is a barrier");
        assert_eq!(mesh.floor_count(), 2, "the upward plane is walkable");
        assert_eq!(mesh.ceiling_count(), 2, "the downward plane is a ceiling");
        assert_eq!(mesh.skipped_faces(), 4, "floors and ceilings are not walls");
        assert!(mesh.has_ground());
    }

    #[test]
    fn a_slope_steeper_than_the_threshold_is_kept_out() {
        // Normal (100, -100, 0): neither horizontal nor vertical dominates.
        let vertices = vec![[0, 0, 0], [10, 10, 0], [0, 0, 10]];
        let mesh = CollisionMesh::from_room_triangles(&vertices, &[[0, 1, 2]]);
        assert_eq!(mesh.wall_count(), 0);
        assert_eq!(mesh.floor_count(), 0);
        assert_eq!(mesh.ceiling_count(), 0);
        assert_eq!(mesh.skipped_faces(), 1);
        assert!(!mesh.has_ground());
    }

    #[test]
    fn ground_height_picks_the_highest_floor_at_or_below_the_ceiling() {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        let (v, f) = floor_up(0, -100, 100, -100, 100);
        append(&mut vertices, &mut faces, v, f);
        let (v, f) = floor_up(100, -100, 100, -100, 100);
        append(&mut vertices, &mut faces, v, f);
        let mesh = CollisionMesh::from_room_triangles(&vertices, &faces);

        assert_eq!(mesh.ground_height(0, 0, 200), Some(100));
        assert_eq!(mesh.ground_height(0, 0, 50), Some(0));
        assert_eq!(mesh.ground_height(0, 0, -10), None);
        assert_eq!(mesh.ground_height(5000, 0, 200), None, "outside every floor");
    }

    fn floor_mesh(y: i32) -> CollisionMesh {
        let (vertices, faces) = floor_up(y, -500, 500, -500, 500);
        CollisionMesh::from_room_triangles(&vertices, &faces)
    }

    #[test]
    fn a_player_above_the_floor_falls_and_lands_on_it() {
        let mesh = floor_mesh(0);
        let mut position = [0, 50, 0];
        let mut velocity = 0i64;
        let mut grounded = false;
        for _ in 0..200 {
            let step = mesh.vertical_step(position, velocity);
            position[1] = step.y;
            velocity = step.velocity_subunits;
            assert!(
                position[1] >= 0,
                "the player must never pass through the floor: {position:?}"
            );
            if step.grounded {
                grounded = true;
                break;
            }
        }
        assert!(grounded, "the fall must end on the floor");
        assert_eq!(position[1], 0);
        assert_eq!(velocity, 0);
    }

    #[test]
    fn a_grounded_player_does_not_sink_or_jitter() {
        let mesh = floor_mesh(0);
        let mut position = [0, 0, 0];
        let mut velocity = 0i64;
        for tick in 0..300 {
            let step = mesh.vertical_step(position, velocity);
            assert!(step.grounded, "tick {tick} lost the ground");
            assert_eq!(step.y, 0, "tick {tick} moved the resting height");
            assert_eq!(step.velocity_subunits, 0, "tick {tick} kept falling");
            position[1] = step.y;
            velocity = step.velocity_subunits;
        }
    }

    #[test]
    fn walking_off_a_ledge_starts_a_fall_and_landing_stops_it() {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        // Upper floor at y = 0 over x in -500..=100.
        let (v, f) = floor_up(0, -500, 100, -500, 500);
        append(&mut vertices, &mut faces, v, f);
        // Lower floor at y = -100 over x in 100..=500.
        let (v, f) = floor_up(-100, 100, 500, -500, 500);
        append(&mut vertices, &mut faces, v, f);
        let mesh = CollisionMesh::from_room_triangles(&vertices, &faces);

        // Resting at x = 0 on the upper floor.
        assert_eq!(mesh.ground_height(0, 0, MAX_STEP_UP_UNITS), Some(0));
        // At x = 300 only the lower floor remains.
        assert_eq!(mesh.ground_height(300, 0, MAX_STEP_UP_UNITS), Some(-100));

        let mut position = [0, 0, 0];
        let mut velocity = 0i64;
        let resting = mesh.vertical_step(position, velocity);
        assert!(resting.grounded);
        assert_eq!(resting.y, 0);

        // Step horizontally off the ledge, then fall.
        position = [300, resting.y, 0];
        velocity = resting.velocity_subunits;
        let mut fell = false;
        let mut landed = false;
        for _ in 0..200 {
            let step = mesh.vertical_step(position, velocity);
            assert!(step.y >= -100, "fell through the lower floor: {}", step.y);
            if !step.grounded {
                fell = true;
            }
            position[1] = step.y;
            velocity = step.velocity_subunits;
            if step.grounded {
                landed = true;
                assert_eq!(step.y, -100, "must land on the lower floor");
                break;
            }
        }
        assert!(fell, "stepping off the ledge must start a fall");
        assert!(landed, "the fall must stop on the lower floor");
        assert_eq!(velocity, 0);
    }

    #[test]
    fn a_mesh_without_floors_leaves_y_untouched() {
        let mesh = wall_mesh();
        assert!(!mesh.has_ground());
        let step = mesh.vertical_step([0, 42, 0], 0);
        assert_eq!(step.y, 42);

        let mesh = CollisionMesh::default();
        let step = mesh.vertical_step([0, -7, 0], -5000);
        assert_eq!(step.y, -7);
        assert_eq!(step.velocity_subunits, 0);
        assert!(!step.grounded);
    }

    #[test]
    fn the_same_vertical_script_replays_identically() {
        fn run() -> Vec<(i32, i64, bool)> {
            let mesh = floor_mesh(0);
            let mut position = [0, 80, 0];
            let mut velocity = 0i64;
            let mut observed = Vec::new();
            for _ in 0..40 {
                let step = mesh.vertical_step(position, velocity);
                position[1] = step.y;
                velocity = step.velocity_subunits;
                observed.push((step.y, step.velocity_subunits, step.grounded));
            }
            observed
        }
        assert_eq!(run(), run());
    }
}
