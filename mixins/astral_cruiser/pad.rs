// Hard-light landing pad under portal exits when opening over the void.
use glam::DVec3;

pub const PAD_HALF: f64 = 4.5;
pub const PROBE: i32 = 24;

/// A solid light platform holding players arriving where there is no ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub center: DVec3,
    pub half: f64,
}

/// Spawns a pad if the 9x9 column under the exit point has no solid blocks
/// down to the probe depth.
pub fn pad_for(exit_feet: DVec3, solid: impl Fn(i32, i32, i32) -> bool) -> Option<Pad> {
    let cx = exit_feet.x.floor() as i32;
    let cz = exit_feet.z.floor() as i32;
    let feet_y = exit_feet.y.floor() as i32;

    for dy in 1..=PROBE {
        let y = feet_y - dy;
        for dx in -4..=4 {
            for dz in -4..=4 {
                if solid(cx + dx, y, cz + dz) {
                    return None;
                }
            }
        }
    }

    Some(Pad {
        center: exit_feet,
        half: PAD_HALF,
    })
}

/// The axis-aligned bounding box of the pad: 9x9 wide and 0.5 blocks thick.
pub fn pad_box(pad: &Pad) -> ([f64; 3], [f64; 3]) {
    (
        [pad.center.x - pad.half, pad.center.y - 0.5, pad.center.z - pad.half],
        [pad.center.x + pad.half, pad.center.y, pad.center.z + pad.half],
    )
}

/// Keeps the pad alive while the portal remains open or someone is standing on it.
pub fn pad_stays(portal_open: bool, standing_on: bool) -> bool {
    portal_open || standing_on
}
