// Bounded, deterministic visual debris; velocities are blocks per second.
use glam::DVec3;
use std::collections::VecDeque;
pub const CAP: usize = 1500;
#[derive(Clone, Debug)]
pub struct Particle {
    pub pos: DVec3, pub vel: DVec3, pub spin: DVec3, pub block: u32,
    pub life: f64, pub heat: f64, pub angular: DVec3,
}
#[derive(Default)]
pub struct Debris { pub particles: VecDeque<Particle> }
pub fn random(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*seed >> 11) as f64 / (1u64 << 53) as f64
}
impl Debris {
    pub fn spawn(&mut self, cell: [i32; 3], block: u32, center: DVec3, strength: f64, mut seed: u64) {
        let pos = DVec3::new(cell[0] as f64 + 0.5, cell[1] as f64 + 0.5, cell[2] as f64 + 0.5);
        let mut outward = DVec3::new(pos.x - center.x, 0.0, pos.z - center.z).normalize_or_zero();
        if outward == DVec3::ZERO { let a = random(&mut seed) * std::f64::consts::TAU; outward = DVec3::new(a.cos(), 0.0, a.sin()); }
        let power = strength.clamp(0.1, 2.0);
        let vel = outward * (8.0 + random(&mut seed) * 24.0) * power + DVec3::Y * (12.0 + random(&mut seed) * 24.0) * power;
        let angular = DVec3::new(random(&mut seed) - 0.5, random(&mut seed) - 0.5, random(&mut seed) - 0.5) * 8.0;
        if self.particles.len() >= CAP { self.particles.pop_front(); }
        self.particles.push_back(Particle { pos, vel, spin: DVec3::ZERO, block, life: 2.0 + random(&mut seed) * 3.0, heat: 1.0, angular });
    }
    pub fn tick(&mut self, dt: f64) {
        if !dt.is_finite() || dt <= 0.0 { return; }
        self.particles.retain_mut(|p| {
            p.life -= dt;
            if p.life <= 0.0 { return false; }
            // Exact linear-drag integration with constant gravity.
            let drag = (-0.45 * dt).exp();
            let terminal = DVec3::NEG_Y * (24.0 / 0.45);
            p.pos += terminal * dt + (p.vel - terminal) * ((1.0 - drag) / 0.45);
            p.vel = terminal + (p.vel - terminal) * drag;
            p.spin += p.angular * dt;
            p.heat = (p.heat - dt * 0.28).max(0.0);
            true
        });
    }
}
