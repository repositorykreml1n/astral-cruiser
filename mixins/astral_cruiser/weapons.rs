// The ship's guns as rules: what fires when, what it costs, and what a shot meets
// first. The server runs this; beams and sparks only show what happened here.
use glam::DVec3;

pub const TURRETS: usize = 8;
const NOSE_MOUNTS: [usize; 2] = [8, 9];
const BELLY_MOUNT: usize = 10;
const MAIN_MOUNT: usize = 11;

const FULL_ENERGY: f64 = 100.0;
const RECHARGE: f64 = 9.0;
const COOLING: f64 = 0.12;
/// Once overheated, the guns stay locked until the heat falls to this.
const SAFE_HEAT: f64 = 0.35;

const TURRET_INTERVAL: f64 = 0.1;
const TURRET_ENERGY: f64 = 1.2;
const TURRET_HEAT: f64 = 0.012;
const TURRET_DAMAGE: f64 = 5.0;

const NOSE_INTERVAL: f64 = 0.3;
const NOSE_ENERGY: f64 = 4.0;
const NOSE_HEAT: f64 = 0.05;
const NOSE_DAMAGE: f64 = 16.0;

const BELLY_ENERGY: f64 = 10.0;
const BELLY_HEAT: f64 = 0.22;
const BELLY_DAMAGE: f64 = 30.0;

const MAIN_CHARGE: f64 = 2.0;
const MAIN_BURN: f64 = 1.5;
const MAIN_ENERGY: f64 = 35.0;
const MAIN_HEAT: f64 = 0.6;
const MAIN_DAMAGE: f64 = 200.0;
const MAIN_COOLDOWN: f64 = 6.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponKind {
    #[default]
    Turrets,
    NosePulse,
    BellyBeam,
    MainBeam,
}

impl WeaponKind {
    pub const ALL: [Self; 4] = [Self::Turrets, Self::NosePulse, Self::BellyBeam, Self::MainBeam];

    pub fn name(self) -> &'static str {
        match self {
            Self::Turrets => "Turrets",
            Self::NosePulse => "Nose guns",
            Self::BellyBeam => "Belly beam",
            Self::MainBeam => "Main beam",
        }
    }

    /// The next weapon along, round and round.
    pub fn stepped(self, by: i32) -> Self {
        let at = Self::ALL.iter().position(|&k| k == self).unwrap_or(0) as i32;
        Self::ALL[(at + by).rem_euclid(Self::ALL.len() as i32) as usize]
    }
}

/// Something leaving a gun this tick, from one of the ship's hardpoints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub kind: WeaponKind,
    pub mount: usize,
    pub damage: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Battery {
    pub energy: f64,
    pub heat: f64,
    /// Overheated: nothing fires until the guns have cooled.
    pub locked: bool,
    /// The main beam's charge, 0 to 1.
    pub charge: f64,
    /// How much longer a fired main beam burns, in seconds.
    pub burning: f64,
    turret_wait: f64,
    nose_wait: f64,
    main_wait: f64,
    next_turret: usize,
    next_nose: usize,
}

impl Default for Battery {
    fn default() -> Self {
        Self::new()
    }
}

impl Battery {
    pub fn new() -> Self {
        Self::restored(FULL_ENERGY, 0.0)
    }

    pub fn restored(energy: f64, heat: f64) -> Self {
        Self {
            energy: energy.clamp(0.0, FULL_ENERGY),
            heat: heat.clamp(0.0, 1.0),
            locked: false,
            charge: 0.0,
            burning: 0.0,
            turret_wait: 0.0,
            nose_wait: 0.0,
            main_wait: 0.0,
            next_turret: 0,
            next_nose: 0,
        }
    }

    /// The share of a full battery left, 0 to 1.
    pub fn level(&self) -> f64 {
        self.energy / FULL_ENERGY
    }

    fn spend(&mut self, energy: f64, heat: f64) -> bool {
        if self.energy < energy {
            return false;
        }
        self.energy -= energy;
        self.heat = (self.heat + heat).min(1.0);
        self.locked = self.locked || self.heat >= 1.0;
        true
    }

    fn settle(&mut self, dt: f64, firing: bool) {
        self.turret_wait = (self.turret_wait - dt).max(0.0);
        self.nose_wait = (self.nose_wait - dt).max(0.0);
        self.main_wait = (self.main_wait - dt).max(0.0);
        self.heat = (self.heat - COOLING * dt).max(0.0);
        if !firing {
            self.energy = (self.energy + RECHARGE * dt).min(FULL_ENERGY);
        }
        self.locked = self.locked && self.heat > SAFE_HEAT;
    }

    fn fire_turret(&mut self, shots: &mut Vec<Shot>) {
        if self.turret_wait <= 1e-9 && self.spend(TURRET_ENERGY, TURRET_HEAT) {
            self.turret_wait = TURRET_INTERVAL;
            shots.push(Shot {
                kind: WeaponKind::Turrets,
                mount: self.next_turret,
                damage: TURRET_DAMAGE,
            });
            self.next_turret = (self.next_turret + 1) % TURRETS;
        }
    }

    fn fire_nose(&mut self, shots: &mut Vec<Shot>) {
        if self.nose_wait <= 1e-9 && self.spend(NOSE_ENERGY, NOSE_HEAT) {
            self.nose_wait = NOSE_INTERVAL;
            shots.push(Shot {
                kind: WeaponKind::NosePulse,
                mount: NOSE_MOUNTS[self.next_nose],
                damage: NOSE_DAMAGE,
            });
            self.next_nose = 1 - self.next_nose;
        }
    }

    fn fire_belly(&mut self, dt: f64, shots: &mut Vec<Shot>) {
        if self.spend(BELLY_ENERGY * dt, BELLY_HEAT * dt) {
            shots.push(Shot {
                kind: WeaponKind::BellyBeam,
                mount: BELLY_MOUNT,
                damage: BELLY_DAMAGE * dt,
            });
        }
    }

    fn charge_main(&mut self, dt: f64) {
        if self.main_wait > 0.0 || self.energy < MAIN_ENERGY {
            return;
        }
        self.charge += dt / MAIN_CHARGE;
        if self.charge >= 1.0 - 1e-9 && self.spend(MAIN_ENERGY, MAIN_HEAT) {
            self.charge = 0.0;
            self.burning = MAIN_BURN;
            self.main_wait = MAIN_BURN + MAIN_COOLDOWN;
        }
    }

    fn burn_main(&mut self, dt: f64, shots: &mut Vec<Shot>) {
        if self.burning > 1e-9 {
            self.burning = (self.burning - dt).max(0.0);
            shots.push(Shot {
                kind: WeaponKind::MainBeam,
                mount: MAIN_MOUNT,
                damage: MAIN_DAMAGE * dt,
            });
        }
    }

    /// One tick of the guns, with `selected` under the trigger and `firing` held.
    /// A main beam once fired burns out whatever the trigger does.
    pub fn tick(&mut self, selected: WeaponKind, firing: bool, dt: f64) -> Vec<Shot> {
        let mut shots = Vec::new();
        if !dt.is_finite() || dt <= 0.0 {
            return shots;
        }
        let firing = firing && !self.locked;
        self.settle(dt, firing || self.burning > 0.0);
        if !firing || selected != WeaponKind::MainBeam {
            self.charge = 0.0;
        }
        if firing {
            match selected {
                WeaponKind::Turrets => self.fire_turret(&mut shots),
                WeaponKind::NosePulse => self.fire_nose(&mut shots),
                WeaponKind::BellyBeam => self.fire_belly(dt, &mut shots),
                WeaponKind::MainBeam => self.charge_main(dt),
            }
        }
        self.burn_main(dt, &mut shots);
        shots
    }
}

/// `wanted`, turned back to the edge of a cone around `axis` if it lies outside
/// it. Both are unit vectors; `cone` is the half angle in radians.
pub fn within_cone(axis: DVec3, wanted: DVec3, cone: f64) -> DVec3 {
    let angle = axis.dot(wanted).clamp(-1.0, 1.0).acos();
    if angle <= cone {
        return wanted;
    }
    let across = (wanted - axis * axis.dot(wanted)).normalize_or_zero();
    if across == DVec3::ZERO {
        return axis;
    }
    (axis * cone.cos() + across * cone.sin()).normalize()
}

/// What a block is to a shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Clear,
    Solid,
    Unloaded,
}

/// A body a shot can strike.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub id: u32,
    pub min: DVec3,
    pub max: DVec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Struck {
    Nothing,
    Block,
    Body(u32),
}

/// Where a shot ended and on what.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub point: DVec3,
    pub struck: Struck,
}

/// How far along a ray it enters a box, if it does.
fn entry(from: DVec3, dir: DVec3, min: DVec3, max: DVec3) -> Option<f64> {
    let (mut near, mut far) = (0.0_f64, f64::MAX);
    for axis in 0..3 {
        if dir[axis].abs() < 1e-12 {
            if from[axis] < min[axis] || from[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let (a, b) = ((min[axis] - from[axis]) / dir[axis], (max[axis] - from[axis]) / dir[axis]);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far).then_some(near)
}

/// How far a shot gets through the blocks: the distance, and whether it ended on
/// a solid one. It walks the ray block by block, so it misses none.
fn through_blocks(
    from: DVec3,
    dir: DVec3,
    reach: f64,
    cell: &impl Fn(i32, i32, i32) -> Cell,
) -> (f64, bool) {
    let mut at = from.floor().as_ivec3();
    let step = dir.signum().as_ivec3();
    let mut next = DVec3::ZERO;
    let mut each = DVec3::ZERO;
    for axis in 0..3 {
        each[axis] = if dir[axis] == 0.0 { f64::MAX } else { (1.0 / dir[axis]).abs() };
        let edge = if dir[axis] > 0.0 { from[axis].floor() + 1.0 } else { from[axis].floor() };
        next[axis] = if dir[axis] == 0.0 { f64::MAX } else { (edge - from[axis]) / dir[axis] };
    }
    let mut travelled = 0.0;
    while travelled <= reach {
        match cell(at.x, at.y, at.z) {
            Cell::Clear => {}
            Cell::Solid => return (travelled, true),
            Cell::Unloaded => return (travelled, false),
        }
        let axis = if next.x <= next.y && next.x <= next.z {
            0
        } else if next.y <= next.z {
            1
        } else {
            2
        };
        travelled = next[axis];
        next[axis] += each[axis];
        at[axis] += step[axis];
    }
    (reach, false)
}

/// How far the pilot's sight must look to cover the guns' `range` from the ship,
/// as the camera may hang far behind it.
pub fn sight_reach(eye: DVec3, ship: DVec3, range: f64) -> f64 {
    eye.distance(ship) + range
}

/// What a shot from `from` along the unit vector `dir` meets first, within `reach`.
pub fn first_hit(
    from: DVec3,
    dir: DVec3,
    reach: f64,
    cell: impl Fn(i32, i32, i32) -> Cell,
    bodies: &[Body],
) -> Hit {
    if !from.is_finite() || !dir.is_finite() || dir == DVec3::ZERO {
        return Hit { point: from, struck: Struck::Nothing };
    }
    let (blocks, solid) = through_blocks(from, dir, reach, &cell);
    let nearest = bodies
        .iter()
        .filter_map(|b| entry(from, dir, b.min, b.max).map(|d| (d, b.id)))
        .filter(|&(d, _)| d <= blocks)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    match nearest {
        Some((distance, id)) => Hit { point: from + dir * distance, struck: Struck::Body(id) },
        None if solid => Hit { point: from + dir * blocks, struck: Struck::Block },
        None => Hit { point: from + dir * blocks.min(reach), struck: Struck::Nothing },
    }
}
