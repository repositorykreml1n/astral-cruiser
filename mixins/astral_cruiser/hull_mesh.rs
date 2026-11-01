// The hull as triangles for the GPU, built once from the layout. Each face has
// its own vertices so edges stay sharp, and carries the part's material for the
// shader.
use super::astral_layout::{Face, Material, Part, Prop, Room, Shape, ShipLayout, WeaponMountKind};
use glam::{DQuat, DVec3, Vec3};
use std::f32::consts::TAU;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HullVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub material: u32,
    /// A door leaf: how far this vertex moves as the door opens.
    pub slide: [f32; 3],
    /// A door leaf: the door's middle, which it opens for.
    pub door: [f32; 3],
}

/// The corners of each face by (right, up, front) as signs and sides, wound
/// counter-clockwise seen from outside the part.
const FACES: [[[i8; 3]; 4]; 6] = [
    [[-1, -1, 1], [1, -1, 1], [1, 1, 1], [-1, 1, 1]],
    [[1, -1, -1], [-1, -1, -1], [-1, 1, -1], [1, 1, -1]],
    [[1, -1, 1], [1, -1, -1], [1, 1, -1], [1, 1, 1]],
    [[-1, -1, -1], [-1, -1, 1], [-1, 1, 1], [-1, 1, -1]],
    [[-1, 1, -1], [-1, 1, 1], [1, 1, 1], [1, 1, -1]],
    [[-1, -1, -1], [1, -1, -1], [1, -1, 1], [-1, -1, 1]],
];

/// The sides of a round pillar.
const SEGMENTS: usize = 16;

fn corner(part: &Part, [side, up, front]: [i8; 3]) -> Vec3 {
    let y = if up > 0 { part.max.y } else { part.min.y };
    let z = if front > 0 { part.max.z } else { part.min.z };
    let middle = (part.min.x + part.max.x) / 2.0;
    let x = middle + f64::from(side) * part.half_width(y, z);
    Vec3::new(x as f32, y as f32, z as f32)
}

fn triangle(out: &mut Vec<HullVertex>, corners: [Vec3; 3], material: u32) {
    let [a, b, c] = corners;
    let normal = (b - a).cross(c - a).normalize_or_zero().to_array();
    out.extend(corners.map(|p| HullVertex {
        position: p.to_array(),
        normal,
        material,
        slide: [0.0; 3],
        door: [0.0; 3],
    }));
}

fn quad(out: &mut Vec<HullVertex>, [a, b, c, d]: [Vec3; 4], material: u32) {
    triangle(out, [a, b, c], material);
    triangle(out, [a, c, d], material);
}

pub fn part_vertices(part: &Part) -> Vec<HullVertex> {
    let material = part.material as u32;
    let mut out = Vec::with_capacity(36);
    for face in &FACES {
        quad(&mut out, face.map(|c| corner(part, c)), material);
    }
    out
}

pub fn hull_vertices(layout: &ShipLayout) -> Vec<HullVertex> {
    layout.parts.iter().flat_map(part_vertices).collect()
}

/// A corner of a prop's box, on a sloped top where that has fallen.
fn box_corner(prop: &Prop, [right, up, front]: [i8; 3]) -> Vec3 {
    let x = if right > 0 { prop.max.x } else { prop.min.x };
    let z = if front > 0 { prop.max.z } else { prop.min.z };
    let y = if up < 0 {
        prop.min.y
    } else {
        match prop.shape {
            Shape::Slope { face, low } => {
                let fallen = match face {
                    Face::Left => right > 0,
                    Face::Right => right < 0,
                    Face::Front => front > 0,
                    Face::Back => front < 0,
                };
                if fallen { low } else { prop.max.y }
            }
            _ => prop.max.y,
        }
    };
    Vec3::new(x as f32, y as f32, z as f32)
}

fn column_vertices(prop: &Prop, out: &mut Vec<HullVertex>) {
    let material = prop.material as u32;
    let (low, high) = (prop.min.as_vec3(), prop.max.as_vec3());
    let middle = (low + high) / 2.0;
    let half = (high - low) / 2.0;
    let rim = |i: usize, y: f32| {
        let turn = TAU * i as f32 / SEGMENTS as f32;
        Vec3::new(middle.x + half.x * turn.cos(), y, middle.z + half.z * turn.sin())
    };
    for i in 0..SEGMENTS {
        let next = i + 1;
        quad(
            out,
            [rim(next, low.y), rim(i, low.y), rim(i, high.y), rim(next, high.y)],
            material,
        );
        let top = Vec3::new(middle.x, high.y, middle.z);
        let bottom = Vec3::new(middle.x, low.y, middle.z);
        triangle(out, [top, rim(next, high.y), rim(i, high.y)], material);
        triangle(out, [bottom, rim(i, low.y), rim(next, low.y)], material);
    }
}

pub fn prop_vertices(prop: &Prop) -> Vec<HullVertex> {
    let mut out = Vec::new();
    match prop.shape {
        Shape::Hidden => {}
        Shape::Column => column_vertices(prop, &mut out),
        Shape::Block | Shape::Slope { .. } => {
            for face in &FACES {
                quad(&mut out, face.map(|c| box_corner(prop, c)), prop.material as u32);
            }
        }
    }
    let (slide, door) = (prop.slide.as_vec3().to_array(), prop.door.as_vec3().to_array());
    for vertex in &mut out {
        vertex.slide = slide;
        vertex.door = door;
    }
    out
}

/// One turret: a domed base that turns and twin barrels that also elevate. The
/// vertex shader moves them about the mount, so each vertex says whose it is:
/// `slide` holds the turret's number from 1, then 1 on the barrels.
fn turret_model(number: usize, mount: Vec3) -> Vec<HullVertex> {
    let at = mount.as_dvec3();
    let block = |shape, min: DVec3, max: DVec3| Prop {
        min: at + min,
        max: at + max,
        material: Material::Turret,
        shape,
        solid: false,
        slide: DVec3::ZERO,
        door: DVec3::ZERO,
    };
    let base = block(Shape::Column, DVec3::new(-1.6, 0.0, -1.6), DVec3::new(1.6, 0.9, 1.6));
    let cap = block(Shape::Column, DVec3::new(-1.1, 0.9, -1.1), DVec3::new(1.1, 1.5, 1.1));
    let barrels = [-0.5, 0.5].map(|x| {
        block(Shape::Block, DVec3::new(x - 0.16, 0.95, 0.4), DVec3::new(x + 0.16, 1.27, 4.8))
    });
    let mark = |prop: &Prop, barrel: f32| {
        let mut vertices = prop_vertices(prop);
        for vertex in &mut vertices {
            vertex.slide = [number as f32, barrel, 0.0];
            vertex.door = mount.to_array();
        }
        vertices
    };
    let mut out = mark(&base, 0.0);
    out.extend(mark(&cap, 0.0));
    out.extend(barrels.iter().flat_map(|barrel| mark(barrel, 1.0)));
    out
}

pub fn turret_vertices(layout: &ShipLayout) -> Vec<HullVertex> {
    layout
        .hardpoints
        .iter()
        .filter(|h| h.kind == WeaponMountKind::Turret)
        .enumerate()
        .flat_map(|(index, h)| turret_model(index + 1, h.position.as_vec3()))
        .collect()
}

/// A shot's light, in world coordinates: a beam from gun to hit, and a flash where
/// it ended. No beam at zero width, no flash at zero size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bolt {
    pub from: DVec3,
    pub to: DVec3,
    pub width: f32,
    pub flash: f32,
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoltVertex {
    pub position: [f32; 3],
    /// Across the beam, or across and up the flash, from -1 to 1.
    pub across: [f32; 2],
    pub color: [f32; 4],
}

/// The most vertices a frame's shots may take: the size of their GPU buffer.
pub const BOLT_VERTICES: usize = 6 * 512;
/// A beam never looks thinner than this, per block from the camera: the pilot's
/// view hangs far behind the guns.
const BOLT_LEAST_WIDTH: f32 = 0.0015;

fn bolt_quad(out: &mut Vec<BoltVertex>, corners: [(Vec3, [f32; 2]); 4], color: [f32; 4]) {
    for index in [0, 1, 2, 0, 2, 3] {
        let (position, across) = corners[index];
        out.push(BoltVertex { position: position.to_array(), across, color });
    }
}

/// A spiral of light hanging square behind the ship, turned by `spin` radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swirl {
    pub middle: DVec3,
    pub rotation: DQuat,
    pub radius: f64,
    pub spin: f64,
    pub power: f32,
}

/// A swirl's quad from the camera. `across` holds the spot on it, -1 to 1.
pub fn swirl_vertices(swirl: &Swirl, camera: DVec3) -> Vec<BoltVertex> {
    if swirl.power <= 0.0 {
        return Vec::new();
    }
    let middle = (swirl.middle - camera).as_vec3();
    let turn = swirl.rotation * DQuat::from_rotation_z(swirl.spin);
    let right = (turn * DVec3::X * swirl.radius).as_vec3();
    let up = (turn * DVec3::Y * swirl.radius).as_vec3();
    let corners = [
        (middle - right - up, [-1.0, -1.0]),
        (middle + right - up, [1.0, -1.0]),
        (middle + right + up, [1.0, 1.0]),
        (middle - right + up, [-1.0, 1.0]),
    ];
    let mut out = Vec::with_capacity(6);
    bolt_quad(&mut out, corners, [WARP_DEEP[0], WARP_DEEP[1], WARP_DEEP[2], swirl.power]);
    out
}

/// The rings of a gathering jump: how far behind the stern, how wide at full
/// charge, how fast each turns against `spin`, in how many arcs.
const WARP_RINGS: [(f64, f64, f64, u32); 3] =
    [(30.0, 40.0, 1.0, 3), (55.0, 62.0, -0.7, 4), (85.0, 84.0, 0.45, 5)];
const WARP_ARC_SEGMENTS: u32 = 7;
/// How much of a ring its arcs fill.
const WARP_ARC_SHARE: f64 = 0.62;
const WARP_SHARDS: u32 = 30;
/// Where the shards start, spiralling in from far behind the stern.
const WARP_SHARD_FROM: f64 = 240.0;
const WARP_SHARD_WIDE: f64 = 95.0;
const WARP_DEEP: [f32; 3] = [0.55, 0.14, 1.0];
const WARP_PALE: [f32; 3] = [0.95, 0.45, 1.0];

/// The pieces of light that turn behind the stern as a jump gathers. `spin` keeps
/// growing as they turn; `power` is the charge, 0 to 1.
pub fn warp_bolts(stern: DVec3, rotation: DQuat, spin: f64, power: f32) -> Vec<Bolt> {
    if power <= 0.0 {
        return Vec::new();
    }
    let share = f64::from(power);
    let at = |behind: f64, wide: f64, angle: f64| {
        stern + rotation * DVec3::new(angle.cos() * wide, angle.sin() * wide, -behind)
    };
    let mut bolts = Vec::new();
    for (ring, &(behind, wide, rate, arcs)) in WARP_RINGS.iter().enumerate() {
        let wide = wide * (0.3 + 0.7 * share);
        let [r, g, b] = if ring == 1 { WARP_PALE } else { WARP_DEEP };
        for arc in 0..arcs {
            let first = spin * rate + std::f64::consts::TAU * f64::from(arc) / f64::from(arcs);
            let length = std::f64::consts::TAU * WARP_ARC_SHARE / f64::from(arcs);
            for step in 0..WARP_ARC_SEGMENTS {
                let along = |at_step: u32| f64::from(at_step) / f64::from(WARP_ARC_SEGMENTS);
                bolts.push(Bolt {
                    from: at(behind, wide, first + length * along(step)),
                    to: at(behind, wide, first + length * along(step + 1)),
                    width: 0.5 + 0.9 * power,
                    flash: 0.0,
                    color: [r, g, b, 0.35 + 0.65 * power],
                });
            }
        }
    }
    for shard in 0..WARP_SHARDS {
        let seed = f64::from(shard) / f64::from(WARP_SHARDS);
        // Each shard's way in, over and over, quicker near the stern.
        let way = (seed + spin * 0.07).fract();
        let near = way * way;
        let angle = seed * 53.0 + spin * (1.0 + near * 2.5);
        let wide = WARP_SHARD_WIDE * (1.0 - near) * (0.4 + 0.6 * share) + 6.0;
        let behind = WARP_SHARD_FROM + (20.0 - WARP_SHARD_FROM) * near;
        let [r, g, b] = if shard % 3 == 0 { WARP_PALE } else { WARP_DEEP };
        bolts.push(Bolt {
            from: at(behind, wide, angle),
            to: at(behind - 4.0 - 10.0 * near, wide * 0.97, angle + 0.14),
            width: 0.45,
            flash: 0.0,
            color: [r, g, b, power * (1.0 - near as f32 * 0.4)],
        });
    }
    let [r, g, b] = WARP_PALE;
    bolts.push(Bolt {
        from: at(26.0, 0.0, 0.0),
        to: at(26.0, 0.0, 0.0),
        width: 0.0,
        flash: 3.0 + 13.0 * power * power,
        color: [r, g, b, power * 0.7],
    });
    bolts
}

/// The wave a ship coming out of a jump sends round itself. `age` runs from 0 to 1.
pub fn arrival_ring(middle: DVec3, rotation: DQuat, age: f32) -> Vec<Bolt> {
    if age >= 1.0 {
        return Vec::new();
    }
    const SEGMENTS: u32 = 48;
    let wide = 70.0 + 230.0 * f64::from(age);
    let point = |step: u32| {
        let angle = std::f64::consts::TAU * f64::from(step) / f64::from(SEGMENTS);
        middle + rotation * DVec3::new(angle.cos() * wide, angle.sin() * wide, 0.0)
    };
    let [r, g, b] = WARP_PALE;
    (0..SEGMENTS)
        .map(|step| Bolt {
            from: point(step),
            to: point(step + 1),
            width: 2.4,
            flash: 0.0,
            color: [r, g, b, (1.0 - age) * (1.0 - age)],
        })
        .collect()
}

/// The quads of this frame's shots, from the camera, turned to face it.
pub fn bolt_vertices(bolts: &[Bolt], camera: DVec3) -> Vec<BoltVertex> {
    let mut out = Vec::new();
    for bolt in bolts {
        if out.len() + 12 > BOLT_VERTICES {
            break;
        }
        let (from, to) = ((bolt.from - camera).as_vec3(), (bolt.to - camera).as_vec3());
        if bolt.width > 0.0 {
            let side = (to - from).cross(from).normalize_or_zero();
            let near = side * bolt.width.max(from.length() * BOLT_LEAST_WIDTH);
            let far = side * bolt.width.max(to.length() * BOLT_LEAST_WIDTH);
            let corners = [
                (from - near, [-1.0, 0.0]),
                (from + near, [1.0, 0.0]),
                (to + far, [1.0, 0.0]),
                (to - far, [-1.0, 0.0]),
            ];
            bolt_quad(&mut out, corners, bolt.color);
        }
        if bolt.flash > 0.0 {
            let look = to.normalize_or_zero();
            let right = look.cross(Vec3::Y).normalize_or(Vec3::X) * bolt.flash;
            let up = right.cross(look).normalize_or(Vec3::Y) * bolt.flash;
            let corners = [
                (to - right - up, [-1.0, -1.0]),
                (to + right - up, [1.0, -1.0]),
                (to + right + up, [1.0, 1.0]),
                (to - right + up, [-1.0, 1.0]),
            ];
            bolt_quad(&mut out, corners, bolt.color);
        }
    }
    out
}

/// Everything standing in the rooms.
pub fn furniture_vertices(layout: &ShipLayout) -> Vec<HullVertex> {
    layout.props.iter().flat_map(prop_vertices).collect()
}

/// A rectangle in the two coordinates along a surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

fn cut(piece: Rect, hole: &Rect) -> Vec<Rect> {
    let low = [piece.min[0].max(hole.min[0]), piece.min[1].max(hole.min[1])];
    let high = [piece.max[0].min(hole.max[0]), piece.max[1].min(hole.max[1])];
    if low[0] >= high[0] || low[1] >= high[1] {
        return vec![piece];
    }
    let mut left = Vec::with_capacity(4);
    if piece.min[0] < low[0] {
        left.push(Rect { min: piece.min, max: [low[0], piece.max[1]] });
    }
    if high[0] < piece.max[0] {
        left.push(Rect { min: [high[0], piece.min[1]], max: piece.max });
    }
    if piece.min[1] < low[1] {
        left.push(Rect { min: [low[0], piece.min[1]], max: [high[0], low[1]] });
    }
    if high[1] < piece.max[1] {
        left.push(Rect { min: [low[0], high[1]], max: [high[0], piece.max[1]] });
    }
    left
}

/// What remains of a rectangle with the holes taken out.
pub fn subtract(rect: Rect, holes: &[Rect]) -> Vec<Rect> {
    holes.iter().fold(vec![rect], |pieces, hole| {
        pieces.into_iter().flat_map(|piece| cut(piece, hole)).collect()
    })
}

const Y: usize = 1;

/// The two axes along a face across `axis`, so that the first crossed with the
/// second points along `axis`.
fn along(axis: usize) -> (usize, usize) {
    ((axis + 1) % 3, (axis + 2) % 3)
}

fn span(min: glam::DVec3, max: glam::DVec3, axis: usize) -> Rect {
    let (u, v) = along(axis);
    Rect {
        min: [min[u] as f32, min[v] as f32],
        max: [max[u] as f32, max[v] as f32],
    }
}

/// A face of a room looking into it, in the room's material for that side.
fn inward_quad(out: &mut Vec<HullVertex>, axis: usize, plane: f32, positive_side: bool, rect: Rect) {
    let (u, v) = along(axis);
    let point = |a: f32, b: f32| {
        let mut p = [0.0; 3];
        p[axis] = plane;
        p[u] = a;
        p[v] = b;
        Vec3::from_array(p)
    };
    let (a, b, c, d) = (
        point(rect.min[0], rect.min[1]),
        point(rect.max[0], rect.min[1]),
        point(rect.max[0], rect.max[1]),
        point(rect.min[0], rect.max[1]),
    );
    let material = match (axis, positive_side) {
        (Y, false) => Material::Deck,
        (Y, true) => Material::Ceiling,
        _ => Material::Wall,
    } as u32;
    // Counter-clockwise from inside: the far side of the room faces back along the axis.
    if positive_side {
        quad(out, [a, d, c, b], material);
    } else {
        quad(out, [a, b, c, d], material);
    }
}

/// Where another room opens through this face, on the side the face looks away from.
fn opening(room: &Room, other: &Room, axis: usize, plane: f64, positive_side: bool) -> Option<Rect> {
    let beyond = if positive_side {
        other.max[axis] > plane && other.min[axis] <= plane
    } else {
        other.min[axis] < plane && other.max[axis] >= plane
    };
    let overlap = room.min.max(other.min);
    let top = room.max.min(other.max);
    beyond.then(|| span(overlap, top, axis))
}

fn room_faces(out: &mut Vec<HullVertex>, layout: &ShipLayout, index: usize) {
    let room = &layout.rooms[index];
    for axis in 0..3 {
        for positive_side in [false, true] {
            let plane = if positive_side { room.max[axis] } else { room.min[axis] };
            let holes: Vec<Rect> = if axis == Y {
                // Rooms that overlap share one floor and one ceiling.
                layout.rooms[..index]
                    .iter()
                    .filter(|other| other.min.y == room.min.y)
                    .map(|other| span(other.min, other.max, axis))
                    .collect()
            } else {
                let rooms = layout
                    .rooms
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != index)
                    .filter_map(|(_, other)| opening(room, other, axis, plane, positive_side));
                let windows = layout
                    .parts
                    .iter()
                    .filter(|p| p.material == Material::Glass)
                    .filter(|p| p.min[axis] <= plane && plane <= p.max[axis])
                    .map(|p| span(p.min, p.max, axis));
                rooms.chain(windows).collect()
            };
            for piece in subtract(span(room.min, room.max, axis), &holes) {
                inward_quad(out, axis, plane as f32, positive_side, piece);
            }
        }
    }
}

/// Floors, walls and ceilings of every room, facing in. Openings between rooms
/// and the windows stay open.
pub fn interior_vertices(layout: &ShipLayout) -> Vec<HullVertex> {
    let mut out = Vec::new();
    for index in 0..layout.rooms.len() {
        room_faces(&mut out, layout, index);
    }
    out
}
