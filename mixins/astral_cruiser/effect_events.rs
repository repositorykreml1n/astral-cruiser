// Window-owned effect timing. Points stay in world coordinates after emission.
use super::astral_arsenal::Weapon;
use super::astral_fx_recipe::Phase;
use glam::DVec3;

#[derive(Clone, Copy, Debug)]
pub struct Emission {
    pub weapon: Weapon,
    pub phase: Phase,
    pub at: DVec3,
    pub dir: DVec3,
    pub due: f32,
}

struct Projectile {
    weapon: Weapon, from: DVec3, to: DVec3, seconds: f32, born: f32, last: f32,
}

#[derive(Default)]
pub struct EffectEvents {
    queued: Vec<Emission>,
    flights: Vec<Projectile>,
    launches: Vec<(u64, f32)>,
    waves: Vec<(u64, f32)>,
    emitted: Vec<Emission>,
}

fn first(seen: &mut Vec<(u64, f32)>, id: u64, now: f32) -> bool {
    seen.retain(|(_, last)| now - *last < 120.0);
    if let Some((_, last)) = seen.iter_mut().find(|(key, _)| *key == id) {
        *last = now;
        return false;
    }
    seen.push((id, now));
    true
}

impl EffectEvents {
    pub fn launch_once(&mut self, id: u64, weapon: Weapon, at: DVec3, target: DVec3, now: f32) {
        if first(&mut self.launches, id, now) {
            self.phase(weapon, Phase::Launch, at, (target - at).normalize_or_zero(), now);
        }
    }

    pub fn first_wave(&mut self, id: u64, now: f32) -> bool { first(&mut self.waves, id, now) }

    pub fn phase(&mut self, weapon: Weapon, phase: Phase, at: DVec3, dir: DVec3, due: f32) {
        self.queued.push(Emission { weapon, phase, at, dir, due });
    }

    pub fn impact(&mut self, weapon: Weapon, at: DVec3, dir: DVec3, now: f32) {
        self.phase(weapon, Phase::Impact, at, dir, now);
        self.phase(weapon, Phase::After, at, dir, now + 1.5);
    }

    pub fn projectile(&mut self, weapon: Weapon, from: DVec3, to: DVec3, seconds: f32, now: f32) {
        if seconds.is_finite() && seconds > 0.0 {
            self.flights.push(Projectile { weapon, from, to, seconds, born: now, last: now });
        }
    }

    pub fn advance(&mut self, now: f32) -> &[Emission] {
        let out = &mut self.emitted;
        out.clear();
        self.queued.retain(|event| {
            if event.due <= now { out.push(*event); false } else { true }
        });
        self.flights.retain_mut(|p| {
            let age = now - p.born;
            if age >= p.seconds { return false; }
            // A stalled frame emits at the current point, never a catch-up burst.
            if now - p.last >= 0.1 - 1e-6 {
                p.last = now;
                out.push(Emission { weapon: p.weapon, phase: Phase::Flight,
                    at: p.from.lerp(p.to, (age / p.seconds).clamp(0.0, 1.0) as f64),
                    dir: (p.to - p.from).normalize_or_zero(), due: now });
            }
            true
        });
        out.as_slice()
    }
}
