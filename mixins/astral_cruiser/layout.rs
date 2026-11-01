// The cruiser's one description, in ship coordinates: +Z is the nose, +Y up and
// +X the pilot's left, the same way round as a player at yaw 0. The hull mesh,
// the walkable rooms and the gun mounts are all read from here.
use super::astral_furnish::{door_leaves, furnishings};
use glam::DVec3;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Hull = 0,
    Armor = 1,
    Trim = 2,
    Light = 3,
    Reactor = 4,
    Engine = 5,
    Glass = 6,
    Deck = 7,
    Wall = 8,
    Ceiling = 9,
    Console = 10,
    Screen = 11,
    Metal = 12,
    Hazard = 13,
    Crate = 14,
    Rail = 15,
    Pipe = 16,
    Seat = 17,
    Holo = 18,
    Grate = 19,
    Cloth = 20,
    Lamp = 21,
    Door = 22,
    Turret = 23,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    Bridge,
    Corridor,
    Reactor,
    Weapons,
    Cargo,
    Airlock,
    Quarters,
    Mess,
    Medbay,
    Doorway,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponMountKind {
    Turret,
    NosePulse,
    BellyBeam,
    MainBeam,
}

/// A hull block: a box whose width shrinks toward its nose and its top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub min: DVec3,
    pub max: DVec3,
    pub material: Material,
    /// The share of the width left at `max.z`.
    pub front_scale: f64,
    /// The share of the width left at `max.y`.
    pub top_scale: f64,
}

impl Part {
    pub fn half_width(&self, y: f64, z: f64) -> f64 {
        let along = (z - self.min.z) / (self.max.z - self.min.z);
        let up = (y - self.min.y) / (self.max.y - self.min.y);
        (self.max.x - self.min.x) / 2.0
            * (1.0 + (self.front_scale - 1.0) * along)
            * (1.0 + (self.top_scale - 1.0) * up)
    }

    fn middle(&self) -> f64 {
        (self.min.x + self.max.x) / 2.0
    }

    pub fn contains(&self, p: DVec3) -> bool {
        (self.min.y..=self.max.y).contains(&p.y)
            && (self.min.z..=self.max.z).contains(&p.z)
            && (p.x - self.middle()).abs() <= self.half_width(p.y, p.z)
    }

    /// The heights at which a vertical line through (x, z) is inside the part.
    fn span(&self, x: f64, z: f64) -> Option<(f64, f64)> {
        if !(self.min.z..=self.max.z).contains(&z) {
            return None;
        }
        let out = (x - self.middle()).abs();
        let (bottom, top) = (self.half_width(self.min.y, z), self.half_width(self.max.y, z));
        if out > bottom {
            return None;
        }
        if out <= top {
            return Some((self.min.y, self.max.y));
        }
        let share = (bottom - out) / (bottom - top);
        Some((self.min.y, self.min.y + (self.max.y - self.min.y) * share))
    }
}

/// The side a sloped top looks toward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Left,
    Right,
    Front,
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Block,
    /// A round pillar in its box.
    Column,
    /// A block whose top falls to `low` on the side it faces, as a desk's does.
    Slope { face: Face, low: f64 },
    /// Stops a body and draws nothing, as the air between a rail's bars.
    Hidden,
}

/// Something standing in a room.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prop {
    pub min: DVec3,
    pub max: DVec3,
    pub material: Material,
    pub shape: Shape,
    /// Stops a body. What is not solid is only seen.
    pub solid: bool,
    /// A door's leaf: how far it moves as the door opens.
    pub slide: DVec3,
    /// A door's leaf: the middle of its door.
    pub door: DVec3,
}

impl Prop {
    /// The same prop on the other side of the ship.
    pub fn mirrored(self) -> Self {
        let shape = match self.shape {
            Shape::Slope { face: Face::Left, low } => Shape::Slope { face: Face::Right, low },
            Shape::Slope { face: Face::Right, low } => Shape::Slope { face: Face::Left, low },
            other => other,
        };
        Self {
            min: DVec3::new(-self.max.x, self.min.y, self.min.z),
            max: DVec3::new(-self.min.x, self.max.y, self.max.z),
            shape,
            slide: self.slide * DVec3::new(-1.0, 1.0, 1.0),
            door: self.door * DVec3::new(-1.0, 1.0, 1.0),
            ..self
        }
    }

    /// The box that stops a body: a round pillar is met inside its corners.
    fn bounds(&self) -> (DVec3, DVec3) {
        if self.shape != Shape::Column {
            return (self.min, self.max);
        }
        let inset = (self.max - self.min) * DVec3::new(0.075, 0.0, 0.075);
        (self.min + inset, self.max - inset)
    }

    fn overlaps(&self, low: DVec3, high: DVec3) -> bool {
        let (min, max) = self.bounds();
        low.cmplt(max).all() && high.cmpgt(min).all()
    }

    /// The top a body of this radius at (x, z) would stand on.
    fn top_under(&self, x: f64, z: f64, radius: f64) -> Option<f64> {
        let (min, max) = self.bounds();
        let over = x + radius > min.x && x - radius < max.x && z + radius > min.z && z - radius < max.z;
        (self.solid && over).then_some(max.y)
    }
}

/// The air of a room. Rooms that touch are open to each other: a wall is the gap
/// between two rooms, and a doorway a small room across that gap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    pub kind: RoomKind,
    pub min: DVec3,
    pub max: DVec3,
    /// A place to stand, on whole-block coordinates.
    pub entry: DVec3,
}

impl Room {
    fn covers(&self, p: DVec3) -> bool {
        p.cmpge(self.min).all() && p.cmple(self.max).all()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hardpoint {
    pub id: u32,
    pub kind: WeaponMountKind,
    pub position: DVec3,
    pub forward: DVec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShipLayout {
    pub parts: Vec<Part>,
    pub rooms: Vec<Room>,
    pub props: Vec<Prop>,
    pub hardpoints: Vec<Hardpoint>,
}

const DECK: f64 = -4.0;
const CEILING: f64 = 2.0;
/// The top of a door.
const LINTEL: f64 = -0.6;
/// The passage to the outer door, past the airlock.
const OUTER_DOOR: [f64; 2] = [20.9, 21.4];

fn part(min: [f64; 3], max: [f64; 3], material: Material) -> Part {
    Part {
        min: DVec3::from_array(min),
        max: DVec3::from_array(max),
        material,
        front_scale: 1.0,
        top_scale: 1.0,
    }
}

fn tapered(min: [f64; 3], max: [f64; 3], material: Material, front: f64, top: f64) -> Part {
    Part {
        front_scale: front,
        top_scale: top,
        ..part(min, max, material)
    }
}

/// The same part on the other side of the ship.
fn mirrored(p: Part) -> Part {
    Part {
        min: DVec3::new(-p.max.x, p.min.y, p.min.z),
        max: DVec3::new(-p.min.x, p.max.y, p.max.z),
        ..p
    }
}

fn hull() -> Vec<Part> {
    use Material::*;
    let mut parts = vec![
        tapered([-40.0, -10.0, -70.0], [40.0, 6.0, 78.0], Hull, 0.12, 0.85),
        tapered([-30.0, 6.0, -66.0], [30.0, 10.0, 54.0], Armor, 0.18, 0.82),
        tapered([-20.0, 10.0, -62.0], [20.0, 13.0, 14.0], Hull, 0.3, 0.8),
        tapered([-6.0, 13.0, -50.0], [6.0, 15.0, -6.0], Armor, 0.75, 0.7),
        tapered([-9.0, 15.0, -42.0], [9.0, 17.0, -20.0], Hull, 0.8, 0.75),
        part([-1.0, 17.0, -36.0], [1.0, 18.0, -30.0], Trim),
        tapered([-26.0, -14.0, -62.0], [26.0, -10.0, 50.0], Armor, 0.22, 1.0),
        tapered([-6.0, -18.0, -20.0], [6.0, -14.0, 46.0], Armor, 0.5, 1.0),
        part([-2.0, -17.0, 46.0], [2.0, -15.0, 66.0], Trim),
        // What holds the belly gun up to the hull along its length.
        part([-1.0, -15.0, 48.0], [1.0, -10.0, 65.0], Armor),
        part([-1.5, -16.6, 66.0], [1.5, -15.4, 66.5], Light),
        part([-2.0, -3.0, 78.0], [2.0, 1.0, 80.0], Light),
        part([-24.0, -8.0, -76.0], [24.0, 4.0, -70.0], Armor),
        part([-20.0, -6.0, -77.0], [-8.0, 2.0, -76.0], Engine),
        part([-6.0, -6.0, -77.0], [6.0, 2.0, -76.0], Engine),
        part([8.0, -6.0, -77.0], [20.0, 2.0, -76.0], Engine),
        part([-0.5, 15.0, -50.0], [0.5, 15.25, -6.0], Light),
        part([-7.0, 10.0, 40.0], [7.0, 11.2, 52.0], Glass),
        part([-8.0, -2.5, 54.0], [8.0, 1.5, 54.2], Glass),
        // The collar of the outer door, on the ship's left side.
        part([21.4, -4.7, -5.6], [23.6, -0.1, -0.4], Trim),
        part([23.6, -4.0, -4.6], [23.8, -0.8, -1.4], Light),
    ];
    let side = [
        tapered([32.0, -8.0, -79.0], [48.0, 6.0, -24.0], Hull, 0.7, 0.8),
        part([24.0, -5.0, -56.0], [36.0, 2.0, -36.0], Armor),
        part([34.0, -6.0, -80.0], [46.0, 4.0, -79.0], Engine),
        part([39.5, 6.0, -70.0], [40.5, 6.2, -30.0], Light),
        // Windows: the walls of the bridge, the cabins and the mess open here.
        part([8.9, -2.0, 46.0], [9.1, 1.2, 53.0], Glass),
        part([11.9, -2.2, 26.0], [12.1, 0.6, 36.0], Glass),
    ];
    parts.extend(side);
    parts.extend(side.map(mirrored));
    parts.extend(turret_mounts().iter().map(|m| {
        part(
            [m[0] - 2.0, m[1], m[2] - 2.0],
            [m[0] + 2.0, m[1] + 1.5, m[2] + 2.0],
            Trim,
        )
    }));
    parts
}

const fn room(kind: RoomKind, min: [f64; 2], max: [f64; 2], entry: [f64; 2]) -> Room {
    Room {
        kind,
        min: DVec3::new(min[0], DECK, min[1]),
        max: DVec3::new(max[0], CEILING, max[1]),
        entry: DVec3::new(entry[0], DECK, entry[1]),
    }
}

/// A doorway across the wall between two rooms.
const fn doorway(min: [f64; 2], max: [f64; 2]) -> Room {
    Room {
        kind: RoomKind::Doorway,
        min: DVec3::new(min[0], DECK, min[1]),
        max: DVec3::new(max[0], LINTEL, max[1]),
        entry: DVec3::new((min[0] + max[0]) / 2.0, DECK, (min[1] + max[1]) / 2.0),
    }
}

/// A room with a door in the spine's left wall, and its twin on the right.
fn off_the_spine(kind: RoomKind, right: RoomKind, near: f64, far: f64, wide: f64, door: f64) -> [Room; 4] {
    let (middle, entry) = ((near + far) / 2.0, (3.0 + wide) / 2.0);
    [
        room(kind, [3.0, near], [wide, far], [entry.round(), middle.round()]),
        room(right, [-wide, near], [-3.0, far], [-entry.round(), middle.round()]),
        doorway([2.5, door - 1.5], [3.0, door + 1.5]),
        doorway([-3.0, door - 1.5], [-2.5, door + 1.5]),
    ]
}

fn rooms() -> Vec<Room> {
    use RoomKind::*;
    let mut rooms = vec![
        room(Bridge, [-9.0, 40.0], [9.0, 54.0], [0.0, 46.0]),
        doorway([-1.5, 39.5], [1.5, 40.0]),
        room(Corridor, [-2.5, -7.5], [2.5, 39.5], [0.0, 30.0]),
        doorway([-1.5, -8.0], [1.5, -7.5]),
        // The reactor hall: the spine crosses it on a catwalk, over a pit.
        Room {
            kind: Reactor,
            min: DVec3::new(-20.0, -9.0, -30.0),
            max: DVec3::new(20.0, 4.0, -8.0),
            entry: DVec3::new(0.0, DECK, -19.0),
        },
        doorway([-1.5, -30.5], [1.5, -30.0]),
        room(Corridor, [-2.5, -42.0], [2.5, -30.5], [0.0, -36.0]),
        doorway([-2.0, -44.0], [2.0, -42.0]),
        Room {
            kind: Cargo,
            min: DVec3::new(-26.0, DECK, -64.0),
            max: DVec3::new(26.0, 4.0, -44.0),
            entry: DVec3::new(0.0, DECK, -46.0),
        },
        // The way out, past the airlock on the left; the sickbay on the right.
        doorway([2.5, -5.0], [3.0, -2.0]),
        room(Corridor, [3.0, -5.0], [14.0, -2.0], [8.0, -4.0]),
        doorway([14.0, -5.0], [14.5, -2.0]),
        room(Airlock, [14.5, -7.0], [20.0, 1.0], [17.0, -3.0]),
        doorway([20.0, -4.5], [21.4, -1.5]),
        doorway([-3.0, -5.0], [-2.5, -2.0]),
        room(Medbay, [-20.0, -7.0], [-3.0, 1.0], [-11.0, -2.0]),
    ];
    rooms.extend(off_the_spine(Weapons, Weapons, 2.0, 20.0, 16.0, 11.0));
    rooms.extend(off_the_spine(Quarters, Mess, 24.0, 38.0, 12.0, 31.0));
    rooms
}

/// Where the turrets stand, on the upper deck: x, the deck's height, z.
const fn turret_mounts() -> [[f64; 3]; 8] {
    [
        [5.5, 10.0, 30.0],
        [-5.5, 10.0, 30.0],
        [10.5, 10.0, 0.0],
        [-10.5, 10.0, 0.0],
        [15.5, 10.0, -30.0],
        [-15.5, 10.0, -30.0],
        [10.0, 13.0, -40.0],
        [-10.0, 13.0, -40.0],
    ]
}

fn hardpoints() -> Vec<Hardpoint> {
    let mut mounts: Vec<(WeaponMountKind, DVec3)> = turret_mounts()
        .iter()
        .map(|m| {
            (
                WeaponMountKind::Turret,
                DVec3::new(m[0], m[1] + 1.5, m[2]),
            )
        })
        .collect();
    mounts.extend([
        (WeaponMountKind::NosePulse, DVec3::new(3.0, -1.0, 78.0)),
        (WeaponMountKind::NosePulse, DVec3::new(-3.0, -1.0, 78.0)),
        (WeaponMountKind::BellyBeam, DVec3::new(0.0, -16.0, 66.5)),
        (WeaponMountKind::MainBeam, DVec3::new(0.0, -1.0, 80.0)),
    ]);
    mounts
        .into_iter()
        .zip(0..)
        .map(|((kind, position), id)| Hardpoint {
            id,
            kind,
            position,
            forward: DVec3::Z,
        })
        .collect()
}

/// Evenly from `low` to `high`, both included, no further apart than `spacing`.
fn steps(low: f64, high: f64, spacing: f64) -> Vec<f64> {
    let count = (((high - low) / spacing).ceil() as usize).max(1);
    (0..=count)
        .map(|i| low + (high - low) * i as f64 / count as f64)
        .collect()
}

impl ShipLayout {
    pub fn cruiser() -> Self {
        let rooms = rooms();
        let mut props = furnishings();
        props.extend(door_leaves(&rooms));
        Self {
            parts: hull(),
            rooms,
            props,
            hardpoints: hardpoints(),
        }
    }

    pub fn half_extents(&self) -> DVec3 {
        DVec3::new(48.0, 18.0, 80.0)
    }

    pub fn room(&self, kind: RoomKind) -> Option<&Room> {
        self.rooms.iter().find(|r| r.kind == kind)
    }

    /// Where the pilot's feet rest, in the chair at the front of the bridge.
    pub fn pilot_seat(&self) -> DVec3 {
        DVec3::new(0.0, -3.6, 50.0)
    }

    /// Where someone stands who got up from the helm.
    pub fn bridge_stand(&self) -> DVec3 {
        DVec3::new(0.0, DECK, 48.2)
    }

    /// Just outside the airlock's outer door.
    pub fn airlock_exit(&self) -> DVec3 {
        DVec3::new(27.0, DECK, -3.0)
    }

    /// Whether someone has walked up to the outer door, at the end of its passage.
    pub fn in_outer_door(&self, at: DVec3) -> bool {
        (OUTER_DOOR[0]..=OUTER_DOOR[1]).contains(&at.x) && (-4.5..=-1.5).contains(&at.z)
    }

    /// The walkable surface under `from_y` at a spot, if a room has one.
    pub fn floor_height(&self, x: f64, z: f64, from_y: f64) -> Option<f64> {
        self.standing_height(x, z, 0.0, from_y)
    }

    /// The same for a body of this radius, which rests on what its edge reaches.
    pub fn standing_height(&self, x: f64, z: f64, radius: f64, from_y: f64) -> Option<f64> {
        let room = self
            .rooms
            .iter()
            .filter(|r| {
                (r.min.x..=r.max.x).contains(&x)
                    && (r.min.z..=r.max.z).contains(&z)
                    && r.min.y <= from_y
            })
            .map(|r| r.min.y)
            .max_by(f64::total_cmp)?;
        let tops = self.props.iter().filter_map(|p| p.top_under(x, z, radius));
        Some(tops.filter(|&top| top <= from_y).fold(room, f64::max))
    }

    fn in_a_room(&self, p: DVec3) -> bool {
        self.rooms.iter().any(|r| r.covers(p))
    }

    fn blocked(&self, low: DVec3, high: DVec3) -> bool {
        self.props.iter().any(|p| p.solid && p.overlaps(low, high))
    }

    /// Whether a standing body of this radius and height fits in the air of
    /// the rooms from its feet at `base` up.
    pub fn capsule_clear(&self, base: DVec3, radius: f64, height: f64) -> bool {
        const LAYER: f64 = 0.5;
        if !base.is_finite()
            || !radius.is_finite()
            || !height.is_finite()
            || radius < 0.0
            || height <= 0.0
        {
            return false;
        }
        let side = DVec3::new(radius, 0.0, radius);
        if self.blocked(base - side, base + side + DVec3::Y * height) {
            return false;
        }
        let layers = (height / LAYER).ceil() as u32;
        (0..=layers).all(|layer| {
            let y = base.y + height * f64::from(layer) / f64::from(layers);
            [-radius, 0.0, radius].iter().all(|&dx| {
                [-radius, 0.0, radius]
                    .iter()
                    .all(|&dz| self.in_a_room(DVec3::new(base.x + dx, y, base.z + dz)))
            })
        })
    }

    /// The solid heights of the hull along a vertical line, lowest first.
    pub fn hull_spans(&self, x: f64, z: f64) -> Vec<(f64, f64)> {
        let mut spans: Vec<(f64, f64)> = self.parts.iter().filter_map(|p| p.span(x, z)).collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut joined: Vec<(f64, f64)> = Vec::with_capacity(spans.len());
        for (low, high) in spans {
            match joined.last_mut() {
                Some(last) if low <= last.1 => last.1 = last.1.max(high),
                _ => joined.push((low, high)),
            }
        }
        joined
    }

    /// Whether a shot from a gun at `from` along the unit vector `along` runs into
    /// the ship. It starts a little clear of the gun, which stands on the hull.
    pub fn ray_meets_hull(&self, from: DVec3, along: DVec3) -> bool {
        const CLEAR_OF_THE_GUN: f64 = 2.5;
        const ACROSS_THE_SHIP: u32 = 190;
        (0..ACROSS_THE_SHIP).any(|step| {
            let at = from + along * (CLEAR_OF_THE_GUN + f64::from(step));
            self.parts.iter().any(|part| part.contains(at))
        })
    }

    /// Points over the outside of the hull, no further apart than `spacing`,
    /// where it meets the world. Those buried in another part are left out.
    pub fn hull_points(&self, spacing: f64) -> Vec<DVec3> {
        let mut points = Vec::new();
        for part in &self.parts {
            let middle = part.middle();
            let (ys, zs) = (
                steps(part.min.y, part.max.y, spacing),
                steps(part.min.z, part.max.z, spacing),
            );
            let across = |y: f64, z: f64| {
                let half = part.half_width(y, z);
                steps(-half, half, spacing).into_iter().map(move |x| DVec3::new(middle + x, y, z))
            };
            for &z in &zs {
                for &y in &ys {
                    let half = part.half_width(y, z);
                    points.push(DVec3::new(middle - half, y, z));
                    points.push(DVec3::new(middle + half, y, z));
                }
                points.extend(across(part.min.y, z));
                points.extend(across(part.max.y, z));
            }
            for &y in &ys {
                points.extend(across(y, part.min.z));
                points.extend(across(y, part.max.z));
            }
        }
        let buried = |p: &DVec3| {
            let inside = |q: &Part| q.contains(*p + DVec3::Y) && q.contains(*p - DVec3::Y)
                && q.contains(*p + DVec3::X) && q.contains(*p - DVec3::X)
                && q.contains(*p + DVec3::Z) && q.contains(*p - DVec3::Z);
            self.parts.iter().any(inside)
        };
        points.retain(|p| !buried(p));
        points
    }
}
