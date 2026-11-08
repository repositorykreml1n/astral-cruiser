// How many effect records a strike may spend, and how many of them are worth
// drawing from far away.
use super::astral_arsenal::Weapon;
use horizon_client::gfx::astral_fx_instance::Prim;
use glam::DVec3;

/// What one burst of the ship's own guns may spend.
pub const LASER_CAP: u32 = 300;

/// What one strike may spend over all its phases.
pub fn cap(w: Weapon) -> u32 {
    if w.is_super() {
        6000
    } else if matches_heavy(w) {
        2500
    } else {
        1200
    }
}

fn matches_heavy(w: Weapon) -> bool {
    w == Weapon::Rod || w == Weapon::BlackHole || w == Weapon::Tectonic
}

/// The share of small particles kept at this distance from the camera.
pub fn distance_share(d: f64) -> f32 {
    if d < 600.0 {
        1.0
    } else if d < 1200.0 {
        0.5
    } else {
        0.0
    }
}

/// Whether a primitive is still drawn when the small particles are not.
pub fn keeps_far(p: Prim) -> bool {
    p == Prim::Fireball || p == Prim::Shell || p == Prim::Beam
}

/// Cull individual spawn points too: a recipe's spread may cross this boundary.
pub fn within_spawn_range(prim: Prim, at: DVec3, camera: DVec3) -> bool {
    keeps_far(prim) || at.distance_squared(camera) <= 1500.0 * 1500.0
}
