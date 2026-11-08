// Retractable strike batteries. All positions and aiming are ship-local.
use glam::{DQuat, DVec3};
use super::astral_arsenal::{spec, Weapon};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Battery { Emitter, Railgun, MissilePods, Discharger, Silo }
pub const ALL: [Battery; 5] = [Battery::Emitter, Battery::Railgun, Battery::MissilePods, Battery::Discharger, Battery::Silo];
pub fn battery(w: Weapon) -> Battery {
    match w {
        Weapon::Ion | Weapon::Lance | Weapon::Drill | Weapon::Grid => Battery::Emitter,
        Weapon::Rod => Battery::Railgun,
        Weapon::Barrage | Weapon::Cluster | Weapon::Missiles | Weapon::Meteors => Battery::MissilePods,
        Weapon::Chain | Weapon::BlackHole | Weapon::Cryo | Weapon::PlasmaWall => Battery::Discharger,
        Weapon::Antimatter | Weapon::SolarPillar | Weapon::Tectonic | Weapon::Erasure | Weapon::Supernova => Battery::Silo,
    }
}
impl Battery {
    pub fn mount(self) -> DVec3 {
        match self {
            Self::Emitter => DVec3::new(0.0, -18.1, 12.0),
            Self::Railgun => DVec3::new(0.0, -15.1, 55.0),
            Self::MissilePods => DVec3::new(43.0, -8.1, -28.0),
            Self::Discharger => DVec3::new(0.0, -14.1, -54.0),
            Self::Silo => DVec3::new(0.0, -18.1, -12.0),
        }
    }
    pub fn length(self) -> f64 { match self { Self::Railgun => 12.0, Self::Silo => 9.0, Self::MissilePods => 5.0, _ => 6.0 } }
    pub fn width(self) -> f64 { match self { Self::Silo => 5.0, Self::MissilePods => 3.0, _ => 2.2 } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage { Hidden, HatchOpening, Extending, Ready, Charging, Firing, Retracting }
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub mount: DVec3, pub pivot: DVec3, pub rotation: DQuat,
    pub muzzle: DVec3, pub direction: DVec3,
    pub hatch: f64, pub extension: f64, pub glow: f64, pub recoil: f64,
}
#[derive(Clone, Debug)]
pub struct Gun {
    pub battery: Battery, pub stage: Stage,
    elapsed: f64, idle: f64, yaw: f64, pitch: f64,
    charge_time: f64, pending: Option<Weapon>,
}
impl Gun {
    pub fn new(battery: Battery) -> Self {
        Self { battery, stage: Stage::Hidden, elapsed: 0.0, idle: 0.0,
            yaw: 0.0, pitch: std::f64::consts::FRAC_PI_2, charge_time: 0.0, pending: None }
    }
    fn enter(&mut self, stage: Stage) { self.stage = stage; self.elapsed = 0.0; }
    pub fn deploy(&mut self) {
        self.idle = 0.0;
        if self.stage == Stage::Hidden || self.stage == Stage::Retracting { self.enter(Stage::HatchOpening); }
    }
    #[allow(dead_code)]
    pub fn charge(&mut self, weapon: Weapon) {
        self.deploy();
        self.pending = Some(weapon);
        if self.stage == Stage::Ready { self.start_charge(); }
    }
    #[allow(dead_code)]
    fn start_charge(&mut self) {
        if let Some(w) = self.pending.take() {
            self.charge_time = spec(w).charge;
            self.enter(Stage::Charging);
        }
    }
    // Authoritative strike age keeps charge and launch aligned with the server.
    pub fn sync(&mut self, weapon: Weapon, age: f64) {
        self.deploy();
        if self.stage == Stage::Ready || self.stage == Stage::Charging || self.stage == Stage::Firing {
            self.charge_time = spec(weapon).charge;
            if age < self.charge_time {
                self.stage = Stage::Charging; self.elapsed = age.max(0.0);
            } else if age < self.charge_time + 0.25 {
                self.stage = Stage::Firing; self.elapsed = age - self.charge_time;
            } else { self.enter(Stage::Ready); }
        }
    }
    pub fn fire(&mut self) { self.deploy(); if self.pose().extension >= 1.0 { self.enter(Stage::Firing); } }
    pub fn tick(&mut self, dt: f64, target: Option<DVec3>, held: bool) {
        if !dt.is_finite() || dt < 0.0 { return; }
        if held { self.deploy(); }
        if let Some(target) = target {
            let dir = (target - self.pose().pivot).normalize_or_zero();
            if dir.length_squared() > 0.0 {
                let yaw = dir.x.atan2(dir.z);
                let pitch = (-dir.y).clamp(-1.0, 1.0).asin();
                let turn = |now: f64, wanted: f64| {
                    let delta = (wanted - now + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
                    now + delta.clamp(-1.8 * dt, 1.8 * dt)
                };
                self.yaw = turn(self.yaw, yaw); self.pitch = turn(self.pitch, pitch);
            }
        }
        let mut left = dt;
        // Consume excess time across boundaries, independent of frame rate.
        for _ in 0..8 {
            let duration = match self.stage {
                Stage::Hidden => break,
                Stage::HatchOpening => 0.5, Stage::Extending => 1.0,
                Stage::Charging => self.charge_time, Stage::Firing => 0.25,
                Stage::Retracting => 1.5,
                Stage::Ready => {
                    if self.pending.is_some() { self.start_charge(); continue; }
                    if !held { self.idle += left; }
                    if self.idle >= 5.0 { left = self.idle - 5.0; self.enter(Stage::Retracting); continue; }
                    break;
                }
            };
            let step = left.min((duration - self.elapsed).max(0.0));
            self.elapsed += step; left -= step;
            if self.elapsed + 1e-9 < duration { break; }
            let next = match self.stage {
                Stage::HatchOpening => Stage::Extending,
                Stage::Extending | Stage::Firing => { self.idle = 0.0; Stage::Ready },
                Stage::Charging => Stage::Firing,
                Stage::Retracting => Stage::Hidden,
                other => other,
            };
            self.enter(next);
        }
    }
    pub fn pose(&self) -> Pose {
        let (hatch, extension) = match self.stage {
            Stage::Hidden => (0.0, 0.0),
            Stage::HatchOpening => ((self.elapsed / 0.5).min(1.0), 0.0),
            Stage::Extending => (1.0, self.elapsed.min(1.0)),
            Stage::Retracting => ((1.0 - (self.elapsed - 1.0) / 0.5).clamp(0.0, 1.0), (1.0 - self.elapsed).max(0.0)),
            _ => (1.0, 1.0),
        };
        let recoil = if self.stage == Stage::Firing { (1.0 - self.elapsed / 0.25).max(0.0) } else { 0.0 };
        let glow = if self.stage == Stage::Charging { (self.elapsed / self.charge_time.max(0.001)).clamp(0.0, 1.0) } else { recoil };
        let mount = self.battery.mount();
        let pivot = mount + DVec3::NEG_Y * (2.0 + 4.0 * extension);
        let rotation = DQuat::from_rotation_y(self.yaw) * DQuat::from_rotation_x(self.pitch);
        let direction = rotation * DVec3::Z;
        Pose { mount, pivot, rotation, direction, muzzle: pivot + direction * (self.battery.length() * extension - recoil), hatch, extension, glow, recoil }
    }
}
#[derive(Clone, Debug)]
pub struct Guns(pub [Gun; 5]);
impl Default for Guns { fn default() -> Self { Self(ALL.map(Gun::new)) } }
impl Guns {
    pub fn muzzle(&self, w: Weapon) -> DVec3 { self.0[battery(w) as usize].pose().muzzle }
}
