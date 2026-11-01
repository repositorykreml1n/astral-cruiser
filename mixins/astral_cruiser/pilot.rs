// What the pilot's keys and view ask of the ship, and where a new ship appears.
use super::astral_core::{CollisionQuery, ShipInput, ShipPose};
use glam::{DQuat, DVec3, IVec3};
use std::f64::consts::{PI, TAU};

/// The fastest the ship turns toward the view, in radians a second.
const MAX_TURN_RATE: f64 = 0.6;
/// Turn rate per radian of error, so the ship eases onto its heading.
const TURN_GAIN: f64 = 2.0;
/// Far enough ahead that the player is clear of the 160 long hull.
const SPAWN_AHEAD: f64 = 130.0;
const SPAWN_ABOVE: f64 = 30.0;
/// Blocks a tick a ship caught in the ground climbs.
const RISE_PER_TICK: f64 = 1.5;
/// The pilot's view: far enough back for the whole 160 long hull to fit the
/// default view, and above it to see its decks.
pub const CHASE_BEHIND: f64 = 176.0;
const CHASE_ABOVE: f64 = 30.0;
/// Where the view from the ship itself is taken: just off the nose, on its axis.
pub const SHIP_EYE: DVec3 = DVec3::new(0.0, 1.0, 84.0);
/// How much further mobs are drawn for the pilot: the game reckons from the
/// camera, and the chase camera hangs further back than a mob's usual range.
const PILOT_MOB_REACH: f64 = 4.0;
/// No part of the hull is further from the ship's middle than this.
const ROOF_REACH: f64 = 110.0;
/// The speed lines start at, in blocks a second, the grid they hang on, how many
/// of its cells around the ship they fill, how near the ship none hang, and how
/// long a moment of its way each one shows.
const STREAK_SPEED: f64 = 200.0;
const STREAK_CELL: f64 = 48.0;
const STREAK_REACH: i32 = 5;
const STREAK_CLEAR: f64 = 110.0;
const STREAK_SECONDS: f64 = 0.12;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotKeys {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub boost: bool,
}

fn axis(positive: bool, negative: bool) -> f64 {
    f64::from(u8::from(positive)) - f64::from(u8::from(negative))
}

/// The ship's turn about Y, in radians, from its nose direction.
pub fn yaw_of(rotation: DQuat) -> f64 {
    let nose = rotation * DVec3::Z;
    nose.x.atan2(nose.z)
}

/// The turn from `from` to `to` the short way round.
fn shortest(from: f64, to: f64) -> f64 {
    (to - from + PI).rem_euclid(TAU) - PI
}

/// The flight input for held keys, with the ship turning toward where the
/// pilot looks. `look_yaw` is a player's yaw in degrees: 0 faces +Z, 90 faces -X.
pub fn flight_input(keys: &PilotKeys, rotation: DQuat, look_yaw: f32) -> ShipInput {
    let heading = -f64::from(look_yaw).to_radians();
    let yaw_rate = if heading.is_finite() {
        (shortest(yaw_of(rotation), heading) * TURN_GAIN).clamp(-MAX_TURN_RATE, MAX_TURN_RATE)
    } else {
        0.0
    };
    ShipInput {
        thrust: DVec3::new(
            axis(keys.left, keys.right),
            axis(keys.up, keys.down),
            axis(keys.forward, keys.back),
        ),
        yaw_rate,
        boost: keys.boost,
    }
}

/// One tick of a ship caught in the ground climbing out by itself: where it goes
/// next, or `None` when it is clear or has no room left above.
pub fn rise_out<C: CollisionQuery + ?Sized>(
    pose: &ShipPose,
    half: DVec3,
    ceiling: f64,
    collision: &C,
) -> Option<ShipPose> {
    if collision.is_clear(pose, half) || pose.position.y + half.y + RISE_PER_TICK > ceiling {
        return None;
    }
    Some(ShipPose {
        position: pose.position + DVec3::Y * RISE_PER_TICK,
        rotation: pose.rotation,
    })
}

/// A new ship: ahead of the player and above them, its nose the way they face.
pub fn spawn_pose(player: DVec3, yaw: f32) -> ShipPose {
    let heading = -f64::from(yaw).to_radians();
    let rotation = DQuat::from_rotation_y(heading);
    let position = player + rotation * DVec3::Z * SPAWN_AHEAD + DVec3::Y * SPAWN_ABOVE;
    ShipPose::new(position, rotation)
}

/// Where the camera stands for a view along a player's yaw and pitch of the
/// ship at `center`: behind it, or ahead of it looking back.
#[allow(dead_code)] // For the tests.
pub fn chase_camera(center: DVec3, yaw: f32, pitch: f32, front: bool) -> DVec3 {
    chase_camera_at(center, yaw, pitch, front, CHASE_BEHIND)
}

/// The same from `behind` blocks back.
pub fn chase_camera_at(center: DVec3, yaw: f32, pitch: f32, front: bool, behind: f64) -> DVec3 {
    let side = if front { 1.0 } else { -1.0 };
    center + look_along(yaw, pitch) * (side * behind) + DVec3::Y * CHASE_ABOVE
}

/// The way a player's yaw and pitch look, as a unit vector.
pub fn look_along(yaw: f32, pitch: f32) -> DVec3 {
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    DVec3::new(-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos())
}

/// The hull around a point as world-aligned boxes for a player's collision:
/// `[x, y, z, x, y, z]`, `cell` wide, from the hull's solid heights at each spot.
/// The heights stay exact however the ship has turned; its sides step by a cell.
pub fn solid_cells(
    pose: &ShipPose,
    around: DVec3,
    reach: f64,
    cell: f64,
    spans: impl Fn(f64, f64) -> Vec<(f64, f64)>,
) -> Vec<[f64; 6]> {
    let snap = |v: f64| (v / cell).floor() * cell;
    let count = (reach / cell).ceil() as i32;
    let mut cells = Vec::new();
    for ix in -count..=count {
        for iz in -count..=count {
            let x = snap(around.x) + f64::from(ix) * cell;
            let z = snap(around.z) + f64::from(iz) * cell;
            let spot = DVec3::new(x + cell / 2.0, pose.position.y, z + cell / 2.0);
            let local = pose.world_to_local(spot);
            for (low, high) in spans(local.x, local.z) {
                let (low, high) = (pose.position.y + low, pose.position.y + high);
                if high >= around.y - reach && low <= around.y + reach {
                    cells.push([x, low, z, x + cell, high, z + cell]);
                }
            }
        }
    }
    cells
}

/// A turret's turn and elevation, in radians, to look at `target` from `mount`,
/// both in ship coordinates.
pub fn turret_angles(target: DVec3, mount: DVec3) -> [f64; 2] {
    let to = target - mount;
    [to.x.atan2(to.z), to.y.atan2(to.x.hypot(to.z))]
}

/// `from` moved toward the angle `to`, the short way round, by at most `step`.
pub fn ease_angle(from: f64, to: f64, step: f64) -> f64 {
    from + shortest(from, to).clamp(-step, step)
}

/// The share `alpha` of the way from one angle to another, the short way round.
pub fn between_angles(from: f64, to: f64, alpha: f64) -> f64 {
    from + shortest(from, to) * alpha
}

/// The height of the ship's top over a spot in the world, if it is over that spot.
/// `spans` gives the hull's solid heights at a point of the ship.
pub fn roof_at(
    pose: &ShipPose,
    x: f64,
    z: f64,
    spans: impl Fn(f64, f64) -> Vec<(f64, f64)>,
) -> Option<f64> {
    let spot = DVec3::new(x, pose.position.y, z);
    if spot.distance(pose.position) > ROOF_REACH {
        return None;
    }
    let local = pose.world_to_local(spot);
    let top = spans(local.x, local.z).iter().map(|&(_, top)| top).fold(None, |best: Option<f64>, top| {
        Some(best.map_or(top, |b| b.max(top)))
    });
    top.map(|top| pose.position.y + top)
}

/// Lines of light about a fast ship, as `(from, to)`. Each hangs still in the air
/// for the ship to rush past, as long as the way it covers in a moment.
pub fn streaks(center: DVec3, velocity: DVec3) -> Vec<(DVec3, DVec3)> {
    if velocity.length() < STREAK_SPEED {
        return Vec::new();
    }
    let middle = (center / STREAK_CELL).floor().as_ivec3();
    let tail = velocity * STREAK_SECONDS;
    let mut lines = Vec::new();
    for x in -STREAK_REACH..=STREAK_REACH {
        for y in -STREAK_REACH..=STREAK_REACH {
            for z in -STREAK_REACH..=STREAK_REACH {
                let cell = middle + IVec3::new(x, y, z);
                let [a, b, c] = cell.to_array().map(|part| i64::from(part) as u64);
                let mixed = a.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
                    ^ c.wrapping_mul(0x1656_67B1_9E37_79F9);
                let hash = (mixed ^ (mixed >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                let hash = hash ^ (hash >> 32);
                if hash % 4 != 0 {
                    continue;
                }
                let part = |shift: u32| ((hash >> shift) & 0xFF) as f64 / 255.0;
                let spot = DVec3::new(part(8), part(16), part(24));
                let at = (cell.as_dvec3() + spot) * STREAK_CELL;
                if at.distance(center) > STREAK_CLEAR {
                    lines.push((at, at - tail));
                }
            }
        }
    }
    lines
}

/// Where someone stands to the hull below or around them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Footing {
    Clear,
    /// On the hull or just above it, and moved along with it.
    Carried,
    /// Inside it: the ship flew into them. The hull's top at their spot.
    Buried(f64),
}

/// `y` is the feet's height and `spans` the hull's solid heights at that spot.
pub fn footing(y: f64, spans: &[(f64, f64)], gap: f64) -> Footing {
    const SKIN: f64 = 1e-3;
    if let Some(&(_, top)) = spans.iter().find(|s| s.0 < y && y < s.1 - SKIN) {
        return Footing::Buried(top);
    }
    if spans.iter().any(|s| s.1 - SKIN <= y && y - s.1 <= gap) {
        return Footing::Carried;
    }
    Footing::Clear
}

/// What a block is to someone beamed down onto it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arrival {
    Open,
    Ground,
    Danger,
    Unknown,
}

/// Where the airlock's beam sets someone down: on top of the first ground under
/// the door. `None` when that is dangerous, not loaded, or nothing at all.
pub fn landing_y(door_y: i32, bottom_y: i32, at: impl Fn(i32) -> Arrival) -> Option<i32> {
    for y in (bottom_y..=door_y).rev() {
        match at(y) {
            Arrival::Open => {}
            Arrival::Ground => return Some(y + 1),
            Arrival::Danger | Arrival::Unknown => return None,
        }
    }
    None
}

/// Whether the door reaches `player`: the beam works at any height, so only the
/// distance along the ground counts.
pub fn near_door(player: DVec3, door: DVec3, reach: f64) -> bool {
    (player - door).with_y(0.0).length() <= reach
}

/// Something the player asks of the ship.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Spawn,
    Remove,
    Board,
    Enter,
    Leave,
}

/// Where the ship is for the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShipIs {
    Missing,
    Elsewhere,
    Here,
}

/// Why `step` can't be done, if it can't.
pub fn refusal(step: Step, aboard: bool, ship: ShipIs) -> Option<&'static str> {
    const NO_SHIP: &str = "Astral Cruiser: no ship here. Use /astral spawn.";
    const FAR: &str = "Astral Cruiser: the ship is in another dimension.";
    const STILL_ABOARD: &str = "Astral Cruiser: leave the ship first.";
    match (step, ship) {
        (Step::Spawn | Step::Remove, _) if aboard => Some(STILL_ABOARD),
        (Step::Spawn, _) => None,
        (Step::Remove | Step::Board | Step::Enter | Step::Leave, ShipIs::Missing) => Some(NO_SHIP),
        (Step::Remove, _) => None,
        (Step::Enter, _) if aboard => Some("Astral Cruiser: you are already aboard."),
        (Step::Leave, _) if !aboard => Some("Astral Cruiser: you are not aboard."),
        (Step::Board | Step::Enter, ShipIs::Elsewhere) => Some(FAR),
        _ => None,
    }
}

/// The game's scale for how far off entities are drawn, for the pilot's view.
pub fn mob_reach(scale: f64, piloting: bool) -> f64 {
    if piloting { scale * PILOT_MOB_REACH } else { scale }
}
