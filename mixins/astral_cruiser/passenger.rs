// A person on foot inside the ship, in ship coordinates: the ship's own motion
// never enters here, so it cannot push anyone through a wall.
use super::astral_layout::ShipLayout;
use glam::DVec3;

const WALK_SPEED: f64 = 4.3;
const SPRINT_SPEED: f64 = 5.6;
const JUMP_SPEED: f64 = 8.4;
const GRAVITY: f64 = 32.0;
const BODY_RADIUS: f64 = 0.3;
const BODY_HEIGHT: f64 = 1.8;
/// How far above a floor still counts as standing on it.
const FOOTING: f64 = 1e-6;
/// The tallest thing walked up without a jump, as a player's own step.
const STEP: f64 = 0.6;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WalkKeys {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sprint: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Passenger {
    /// Where the feet are, in ship coordinates.
    pub local: DVec3,
    /// Vertical speed, up.
    pub rise: f64,
}

fn axis(positive: bool, negative: bool) -> f64 {
    f64::from(u8::from(positive)) - f64::from(u8::from(negative))
}

impl Passenger {
    pub fn at(local: DVec3) -> Self {
        Self { local, rise: 0.0 }
    }

    fn fits(layout: &ShipLayout, at: DVec3) -> bool {
        layout.capsule_clear(at, BODY_RADIUS, BODY_HEIGHT)
    }

    fn floor(&self, layout: &ShipLayout) -> Option<f64> {
        let at = self.local;
        layout.standing_height(at.x, at.z, BODY_RADIUS, at.y + FOOTING)
    }

    fn walk(&mut self, keys: &WalkKeys, heading: f64, layout: &ShipLayout, dt: f64) {
        let forward = DVec3::new(heading.sin(), 0.0, heading.cos());
        let left = DVec3::new(heading.cos(), 0.0, -heading.sin());
        let wish = forward * axis(keys.forward, keys.back) + left * axis(keys.left, keys.right);
        let speed = if keys.sprint { SPRINT_SPEED } else { WALK_SPEED };
        let step = wish.normalize_or_zero() * speed * dt;
        // Each axis alone, so a wall met at an angle is slid along.
        for part in [DVec3::new(step.x, 0.0, 0.0), DVec3::new(0.0, 0.0, step.z)] {
            let ahead = self.local + part;
            if Self::fits(layout, ahead) {
                self.local = ahead;
            } else if let Some(up) = Self::stepped_up(layout, ahead) {
                self.local = up;
            }
        }
    }

    /// `ahead` on top of what stands there, if that is a step and not a wall.
    fn stepped_up(layout: &ShipLayout, ahead: DVec3) -> Option<DVec3> {
        let top = layout.standing_height(ahead.x, ahead.z, BODY_RADIUS, ahead.y + STEP)?;
        let up = DVec3::new(ahead.x, top, ahead.z);
        (top > ahead.y && Self::fits(layout, up)).then_some(up)
    }

    fn fall(&mut self, keys: &WalkKeys, floor: f64, dt: f64, layout: &ShipLayout) {
        if self.local.y <= floor + FOOTING && self.rise <= 0.0 {
            self.local.y = floor;
            self.rise = if keys.jump { JUMP_SPEED } else { 0.0 };
            if self.rise == 0.0 {
                return;
            }
        }
        self.rise -= GRAVITY * dt;
        let next = self.local + DVec3::Y * (self.rise * dt);
        if next.y <= floor {
            self.local.y = floor;
            self.rise = 0.0;
        } else if Self::fits(layout, next) {
            self.local = next;
        } else {
            self.rise = 0.0;
        }
    }

    /// One tick on foot. `heading` is the way the person faces, as a turn about
    /// the ship's Y axis from its nose. Someone not over a room stays put.
    pub fn tick(&mut self, keys: &WalkKeys, heading: f64, layout: &ShipLayout, dt: f64) {
        if !dt.is_finite() || dt <= 0.0 || !heading.is_finite() {
            return;
        }
        if self.floor(layout).is_none() {
            return;
        }
        self.walk(keys, heading, layout, dt);
        if let Some(floor) = self.floor(layout) {
            self.fall(keys, floor, dt, layout);
        }
    }
}
