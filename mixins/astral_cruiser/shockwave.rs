// Shockwave propagation, camera shake noise, and ship push impulses.
use glam::DVec3;

pub const SPEED: f64 = 340.0; // blocks per second
pub const SHAKE_CAP: f64 = 1.0;
pub const LINGER: f64 = 1.5; // seconds a wave lives after its front reached its radius

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wave {
    pub center: DVec3,
    pub start: f64,
    pub radius: f64,
    pub strength: f64,
    pub damaging: bool,
}

impl Wave {
    /// Front position: 0 before start, then SPEED * (now - start), capped at radius.
    pub fn front(&self, now: f64) -> f64 {
        if now <= self.start || self.radius <= 0.0 {
            0.0
        } else {
            (SPEED * (now - self.start)).min(self.radius)
        }
    }

    /// Whether the wave has expired: now >= start + radius / SPEED + LINGER.
    pub fn done(&self, now: f64) -> bool {
        let r = self.radius.max(0.0);
        now >= self.start + r / SPEED + LINGER
    }

    /// Geometric falloff: (1 - dist / radius).max(0)^1.5.
    pub fn falloff(&self, dist: f64) -> f64 {
        if self.radius <= 0.0 || dist < 0.0 {
            return 0.0;
        }
        (1.0 - dist / self.radius).max(0.0).powf(1.5)
    }

    /// Moment the front reaches 3D point `at`: start + dist / SPEED, None if dist > radius.
    pub fn arrival(&self, at: DVec3) -> Option<f64> {
        if self.radius <= 0.0 {
            return None;
        }
        let dist = self.center.distance(at);
        if dist > self.radius {
            None
        } else {
            Some(self.start + dist / SPEED)
        }
    }

    /// Whether the front crossed `at` in the half-open interval `(before, now]`.
    pub fn crossed(&self, at: DVec3, before: f64, now: f64) -> bool {
        match self.arrival(at) {
            Some(t) => t > before && t <= now,
            None => false,
        }
    }

    /// Amplitude of camera shake at `at`: 0 before arrival or beyond radius;
    /// otherwise strength * falloff * exp(-(now - arrival) / 0.5).
    pub fn shake_at(&self, at: DVec3, now: f64) -> f64 {
        let arr = match self.arrival(at) {
            Some(t) => t,
            None => return 0.0,
        };
        if now < arr {
            return 0.0;
        }
        let dist = self.center.distance(at);
        let fo = self.falloff(dist);
        let decay = (-(now - arr) / 0.5).exp();
        self.strength * fo * decay
    }
}

/// Sum of shake amplitudes across all waves, clamped to [0, SHAKE_CAP].
pub fn shake_total(waves: &[Wave], at: DVec3, now: f64) -> f64 {
    let sum: f64 = waves.iter().map(|w| w.shake_at(at, now)).sum();
    sum.clamp(0.0, SHAKE_CAP)
}

fn axis_noise(now: f64, phase: f64) -> f64 {
    const W1: f64 = 13.7 * std::f64::consts::TAU;
    const W2: f64 = 21.3 * std::f64::consts::TAU;
    const W3: f64 = 33.1 * std::f64::consts::TAU;
    let s1 = (W1 * now + phase).sin();
    let s2 = (W2 * now + phase * 2.3).sin();
    let s3 = (W3 * now + phase * 4.1).sin();
    (s1 + s2 + s3) / 3.0
}

/// Smooth camera position and angle offset from time only, scaled by `amount`.
/// Returns (offset: DVec3, yaw_deg: f32, pitch_deg: f32).
/// Each position axis |.| <= 0.6 * amount, yaw and pitch degrees |.| <= 2.5 * amount.
/// `amount` is clamped to [0, SHAKE_CAP].
pub fn shake_offset(amount: f64, now: f64) -> (DVec3, f32, f32) {
    let amt = amount.clamp(0.0, SHAKE_CAP);
    let ox = 0.6 * amt * axis_noise(now, 0.0);
    let oy = 0.6 * amt * axis_noise(now, 1.5);
    let oz = 0.6 * amt * axis_noise(now, 3.0);
    let yaw = (2.5 * amt * axis_noise(now, 4.5)) as f32;
    let pitch = (2.5 * amt * axis_noise(now, 6.0)) as f32;
    (DVec3::new(ox, oy, oz), yaw, pitch)
}

/// Horizontal unit vector from center to ship * 8 * strength * falloff, plus y = 3 * strength * falloff.
/// Zero vector beyond radius; if ship is exactly above center, pushes in +x.
pub fn ship_push(wave: &Wave, ship: DVec3) -> DVec3 {
    if wave.radius <= 0.0 {
        return DVec3::ZERO;
    }
    let dist = wave.center.distance(ship);
    if dist > wave.radius {
        return DVec3::ZERO;
    }
    let dx = ship.x - wave.center.x;
    let dz = ship.z - wave.center.z;
    let h_len = dx.hypot(dz);
    let (ux, uz) = if h_len <= 1e-9 {
        (1.0, 0.0)
    } else {
        (dx / h_len, dz / h_len)
    };
    let fo = wave.falloff(dist);
    let h_mag = 8.0 * wave.strength * fo;
    let y_mag = 3.0 * wave.strength * fo;
    DVec3::new(ux * h_mag, y_mag, uz * h_mag)
}
