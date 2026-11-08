// Orbital strike rules: damage falloff, knockback impulses, fire column spots,
// and surface finding for impact effects.
use glam::DVec3;

/// Computes blast damage with optional linear falloff to zero at the radius edge.
pub fn falloff_damage(amount: f64, radius: f64, distance: f64, falloff: bool) -> f64 {
    if distance > radius || radius <= 0.0 {
        0.0
    } else if !falloff {
        amount
    } else {
        amount * (1.0 - distance / radius)
    }
}

/// Velocity added to a mob at `target`: zero when 3D distance > radius, otherwise
/// pushing horizontally away and upward.
pub fn knockback(at: DVec3, radius: f64, strength: f64, target: DVec3) -> DVec3 {
    let d = at.distance(target);
    if d > radius || radius <= 0.0 {
        return DVec3::ZERO;
    }
    let dx = target.x - at.x;
    let dz = target.z - at.z;
    let h_len = dx.hypot(dz);
    let dir = if h_len <= 1e-9 {
        DVec3::X
    } else {
        DVec3::new(dx / h_len, 0.0, dz / h_len)
    };
    let scale = strength * (1.0 - d / radius);
    dir * scale * 1.5 + DVec3::Y * scale * 0.6
}

#[derive(Clone, Copy, Debug)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0xda942042e4dd58b5 } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Generates up to `count` distinct block columns in the outer ring of the blast
/// where fire can be ignited.
pub fn fire_spots(at: DVec3, radius: f64, seed: u64, count: usize) -> Vec<(i32, i32)> {
    if radius <= 0.0 || count == 0 {
        return Vec::new();
    }
    let mut spots = Vec::with_capacity(count);
    let mut rng = Rng::new(seed);
    let min_r = 0.75 * radius;
    let max_tries = count * 50;

    for _ in 0..max_tries {
        if spots.len() == count {
            break;
        }
        let angle = rng.next_f64() * std::f64::consts::TAU;
        let r = min_r + rng.next_f64() * (radius - min_r);
        let x = (at.x + r * angle.cos()).floor() as i32;
        let z = (at.z + r * angle.sin()).floor() as i32;
        let cx = x as f64 + 0.5;
        let cz = z as f64 + 0.5;
        let d = (cx - at.x).hypot(cz - at.z);
        if d >= min_r && d <= radius && !spots.contains(&(x, z)) {
            spots.push((x, z));
        }
    }

    spots
}

/// Finds the highest solid block in `bottom..=top` with air directly above it.
pub fn surface_y(top: i32, bottom: i32, solid: impl Fn(i32) -> bool) -> Option<i32> {
    for y in (bottom..=top).rev() {
        if solid(y) && !solid(y + 1) {
            return Some(y);
        }
    }
    None
}
