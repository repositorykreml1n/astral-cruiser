// The lance follows the aim across the ground, leaving a segment to excavate.
use glam::DVec3;

pub const LANCE_SPEED: f64 = 20.0;
pub const LANCE_WIDTH: f64 = 4.0;
pub const LANCE_DEPTH: f64 = 8.0;
pub const LANCE_DAMAGE: f64 = 30.0;
pub const LANCE_RADIUS: f64 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lance {
    pub at: DVec3,
}

impl Lance {
    pub fn start(at: DVec3) -> Self {
        Self { at }
    }

    /// Move toward the aim without overshooting; height follows it at once.
    pub fn follow(&mut self, aim: DVec3, dt: f64) -> (DVec3, DVec3) {
        let from = self.at;
        let dx = aim.x - from.x;
        let dz = aim.z - from.z;
        let distance = dx.hypot(dz);
        let step = LANCE_SPEED * dt.max(0.0);
        self.at = if distance <= step {
            aim
        } else {
            let scale = step / distance;
            DVec3::new(from.x + dx * scale, aim.y, from.z + dz * scale)
        };
        (from, self.at)
    }
}
