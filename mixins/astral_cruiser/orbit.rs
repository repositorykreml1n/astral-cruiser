// Orbital strikes: targeting, timing, cooldowns, concurrent strikes, and state machines.
// The ship hovers above the world and calls down heavy strikes.
use glam::DVec3;

pub use super::astral_arsenal::{WaveSound, Weapon, WeaponSpec};
pub use super::astral_strike_plans::Timed;

pub fn spec(weapon: Weapon) -> WeaponSpec {
    super::astral_arsenal::spec(weapon)
}

pub const RANGE: f64 = 384.0;
pub const MAX_ACTIVE: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Carve {
    Clear { center: DVec3, radius: f64, height: f64 },
    Bowl { center: DVec3, radius: f64, depth: f64 },
    Sphere { center: DVec3, radius: f64 },
    FlatBowl { center: DVec3, radius: f64, depth: f64 },
    Shaft { center: DVec3, radius: f64 },
    Fissures { center: DVec3, count: u32, length: f64, depth: f64, width: f64 },
    Trench { from: DVec3, to: DVec3, width: f64, depth: f64 },
    Grid { center: DVec3, half: f64, step: f64, width: f64, depth: f64 },
    Rim { center: DVec3, radius: f64, thickness: f64 },
    Uplift { center: DVec3, rings: [(f64, f64); 3] },
    Erase { center: DVec3, radius: f64, plane: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Mushroom,
    Pillar,
    Quake,
    BlackHole,
    Frost,
    GridBeams,
    Drill,
    Wall,
    Scan,
    Supernova,
    Erasure,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Source { id: u64, weapon: Weapon, center: DVec3 },
    Damage { at: DVec3, radius: f64, amount: f64, falloff: bool },
    Carve(Carve),
    Knockback { at: DVec3, radius: f64, strength: f64 },
    Ignite { at: DVec3, radius: f64 },
    LanceTick { aim: DVec3, dt: f64 },
    Shell { from: DVec3, to: DVec3, seconds: f64, weapon: Weapon },
    Impact { at: DVec3, weapon: Weapon, radius: f64 },
    Wave { center: DVec3, radius: f64, strength: f64, damaging: bool, sound: WaveSound },
    Freeze { at: DVec3, radius: f64 },
    Pull { at: DVec3, radius: f64, strength: f64, seconds: f64 },
    Chain { at: DVec3, radius: f64, hops: u32, damage: f64 },
    Seek { from: DVec3, near: DVec3, radius: f64, seconds: f64 },
    Show { fx: Shown, at: DVec3, seconds: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refusal {
    Cooling { left: f64 },
    NoEnergy,
    OutOfRange,
    Busy,
}

/// A strike under way: which weapon, where it lands if it has one spot, and how long it has run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrikeView {
    pub weapon: Weapon,
    pub target: Option<DVec3>,
    pub elapsed: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0xda942042e4dd58b5 } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[derive(Clone, Copy, Debug)]
struct BarrageShell {
    from: DVec3,
    to: DVec3,
    spawn_time: f64,
    impact_time: f64,
    spawned: bool,
    impacted: bool,
}

#[derive(Clone, Debug)]
enum ActiveStrike {
    Ion {
        target: DVec3,
        elapsed: f64,
        carve_step: usize,
    },
    Barrage {
        target: DVec3,
        shells: Vec<BarrageShell>,
        elapsed: f64,
    },
    Rod {
        target: DVec3,
        from: DVec3,
        elapsed: f64,
    },
    Lance {
        elapsed: f64,
    },
    Planned {
        weapon: Weapon,
        target: DVec3,
        plan: Vec<Timed>,
        next: usize,
        elapsed: f64,
    },
}

#[derive(Clone, Debug)]
pub struct Orbit {
    selected: Weapon,
    cooldowns: [f64; super::astral_arsenal::COUNT],
    rng: Rng,
    active_strikes: Vec<ActiveStrike>,
    strike_ids: Vec<u64>,
    next_id: u64,
}

impl Orbit {
    pub fn new(seed: u64) -> Self {
        Self {
            selected: Weapon::Ion,
            cooldowns: [0.0; super::astral_arsenal::COUNT],
            rng: Rng::new(seed),
            active_strikes: Vec::new(),
            strike_ids: Vec::new(),
            next_id: 0,
        }
    }

    pub fn select(&mut self, weapon: Weapon) {
        self.selected = weapon;
    }

    pub fn selected(&self) -> Weapon {
        self.selected
    }

    pub fn cooldown(&self, weapon: Weapon) -> f64 {
        self.cooldowns[weapon as usize]
    }

    pub fn busy(&self) -> bool {
        !self.active_strikes.is_empty()
    }

    /// The strikes under way, as the window draws them. A lance has no fixed target.
    pub fn active(&self) -> Vec<StrikeView> {
        self.active_strikes
            .iter()
            .map(|s| match s {
                ActiveStrike::Ion { target, elapsed, .. } => StrikeView {
                    weapon: Weapon::Ion,
                    target: Some(*target),
                    elapsed: *elapsed,
                },
                ActiveStrike::Barrage { target, elapsed, .. } => StrikeView {
                    weapon: Weapon::Barrage,
                    target: Some(*target),
                    elapsed: *elapsed,
                },
                ActiveStrike::Rod { target, elapsed, .. } => StrikeView {
                    weapon: Weapon::Rod,
                    target: Some(*target),
                    elapsed: *elapsed,
                },
                ActiveStrike::Lance { elapsed } => StrikeView {
                    weapon: Weapon::Lance,
                    target: None,
                    elapsed: *elapsed,
                },
                ActiveStrike::Planned { weapon, target, elapsed, .. } => StrikeView {
                    weapon: *weapon,
                    target: Some(*target),
                    elapsed: *elapsed,
                },
            })
            .collect()
    }

    pub fn fire(&mut self, target: DVec3, ship: DVec3, energy: &mut f64) -> Result<(), Refusal> {
        if !target.is_finite() || !ship.is_finite() {
            return Err(Refusal::OutOfRange);
        }
        let lance_already_active = self.selected == Weapon::Lance
            && self.active_strikes.iter().any(|s| matches!(s, ActiveStrike::Lance { .. }));
        if self.active_strikes.len() >= MAX_ACTIVE || lance_already_active {
            return Err(Refusal::Busy);
        }
        let dx = target.x - ship.x;
        let dz = target.z - ship.z;
        let horizontal_dist = (dx * dx + dz * dz).sqrt();
        if horizontal_dist > RANGE {
            return Err(Refusal::OutOfRange);
        }
        let cd = self.cooldown(self.selected);
        if cd > 1e-9 {
            return Err(Refusal::Cooling { left: cd });
        }
        let strike_spec = super::astral_arsenal::spec(self.selected);
        if *energy < strike_spec.energy {
            return Err(Refusal::NoEnergy);
        }
        *energy -= strike_spec.energy;
        self.cooldowns[self.selected as usize] = strike_spec.cooldown;

        self.next_id = self.next_id.wrapping_add(1);
        self.strike_ids.push(self.next_id);
        match self.selected {
            Weapon::Ion => {
                self.active_strikes.push(ActiveStrike::Ion {
                    target,
                    elapsed: 0.0,
                    carve_step: 0,
                });
            }
            Weapon::Barrage => {
                let mut shells = Vec::with_capacity(24);
                for i in 0..24 {
                    let angle = self.rng.next_f64() * std::f64::consts::TAU;
                    let r = self.rng.next_f64().sqrt() * 32.0;
                    let to = DVec3::new(
                        target.x + r * angle.cos(),
                        target.y,
                        target.z + r * angle.sin(),
                    );
                    let spawn_time = i as f64 * (4.0 / 24.0);
                    let impact_time = spawn_time + 1.5;
                    shells.push(BarrageShell {
                        from: ship,
                        to,
                        spawn_time,
                        impact_time,
                        spawned: false,
                        impacted: false,
                    });
                }
                self.active_strikes.push(ActiveStrike::Barrage {
                    target,
                    shells,
                    elapsed: 0.0,
                });
            }
            Weapon::Rod => {
                self.active_strikes.push(ActiveStrike::Rod {
                    target,
                    from: ship,
                    elapsed: 0.0,
                });
            }
            Weapon::Lance => {
                self.active_strikes.push(ActiveStrike::Lance {
                    elapsed: 0.0,
                });
            }
            planned_weapon => {
                let seed = self.rng.next_u64();
                let plan = super::astral_strike_plans::plan(planned_weapon, target, ship, seed);
                self.active_strikes.push(ActiveStrike::Planned {
                    weapon: planned_weapon,
                    target,
                    plan,
                    next: 0,
                    elapsed: 0.0,
                });
            }
        }
        Ok(())
    }

    pub fn tick(
        &mut self,
        dt: f64,
        holding: bool,
        aim: DVec3,
        ship: DVec3,
        energy: &mut f64,
    ) -> Vec<Event> {
        for cd in &mut self.cooldowns {
            if *cd > 0.0 {
                *cd = (*cd - dt).max(0.0);
            }
        }

        let mut events = Vec::new();
        let mut finished_indices = Vec::new();

        for (idx, strike) in self.active_strikes.iter_mut().enumerate() {
            let start = events.len();
            let (weapon, center) = match &*strike {
                ActiveStrike::Ion { target, .. } => (Weapon::Ion, *target),
                ActiveStrike::Barrage { target, .. } => (Weapon::Barrage, *target),
                ActiveStrike::Rod { target, .. } => (Weapon::Rod, *target),
                ActiveStrike::Lance { .. } => (Weapon::Lance, aim),
                ActiveStrike::Planned { weapon, target, .. } => (*weapon, *target),
            };
            match strike {
                ActiveStrike::Ion { target, elapsed, carve_step } => {
                    let target = *target;
                    let old_elapsed = *elapsed;
                    *elapsed += dt;

                    // When the charge finishes, beam ignites, knocks back, and emits waves.
                    if old_elapsed < 3.0 && *elapsed >= 3.0 {
                        events.push(Event::Carve(Carve::Clear { center: target, radius: 14.0, height: 32.0 }));
                        events.push(Event::Knockback { at: target, radius: 40.0, strength: 1.0 });
                        events.push(Event::Ignite { at: target, radius: 14.0 });
                        for wave in super::astral_arsenal::spec(Weapon::Ion).waves {
                            events.push(Event::Wave {
                                center: target,
                                radius: wave.radius,
                                strength: wave.strength,
                                damaging: wave.damaging,
                                sound: wave.sound,
                            });
                        }
                    }

                    // Carve bowl deepens at the start of each second of the beam.
                    const CARVE_DEPTHS: [f64; 4] = [4.0, 8.0, 12.0, 16.0];
                    while *carve_step < 4 {
                        let step_time = 3.0 + *carve_step as f64;
                        if *elapsed >= step_time - 1e-6 {
                            events.push(Event::Carve(Carve::Bowl {
                                center: target,
                                radius: 14.0,
                                depth: CARVE_DEPTHS[*carve_step],
                            }));
                            *carve_step += 1;
                        } else {
                            break;
                        }
                    }

                    // Constant beam damage during active 4 seconds (3.0 to 7.0).
                    let active_start = old_elapsed.max(3.0);
                    let active_end = (*elapsed).min(7.0);
                    let active_dt = (active_end - active_start).max(0.0);
                    if active_dt > 0.0 {
                        events.push(Event::Damage {
                            at: target,
                            radius: 16.0,
                            amount: 40.0 * active_dt,
                            falloff: false,
                        });
                    }

                    if *elapsed >= 7.0 - 1e-6 {
                        finished_indices.push(idx);
                    }
                }
                ActiveStrike::Barrage { target, shells, elapsed } => {
                    let target = *target;
                    let old_elapsed = *elapsed;
                    *elapsed += dt;
                    let current_time = *elapsed;

                    for shell in shells.iter_mut() {
                        if !shell.spawned && current_time >= shell.spawn_time {
                            shell.spawned = true;
                            shell.from = ship;
                            events.push(Event::Shell {
                                from: ship,
                                to: shell.to,
                                seconds: 1.5,
                                weapon: Weapon::Barrage,
                            });
                        }
                        if shell.spawned && !shell.impacted && current_time >= shell.impact_time {
                            shell.impacted = true;
                            events.push(Event::Carve(Carve::Clear { center: shell.to, radius: 6.0, height: 12.0 }));
                            events.push(Event::Impact {
                                at: shell.to,
                                weapon: Weapon::Barrage,
                                radius: 6.0,
                            });
                            events.push(Event::Damage {
                                at: shell.to,
                                radius: 8.0,
                                amount: 25.0,
                                falloff: false,
                            });
                            events.push(Event::Carve(Carve::Sphere {
                                center: shell.to,
                                radius: 6.0,
                            }));
                        }
                    }

                    for wave in super::astral_arsenal::spec(Weapon::Barrage).waves {
                        if old_elapsed < wave.delay && *elapsed >= wave.delay {
                            events.push(Event::Wave {
                                center: target,
                                radius: wave.radius,
                                strength: wave.strength,
                                damaging: wave.damaging,
                                sound: wave.sound,
                            });
                        }
                    }

                    if shells.iter().all(|s| s.impacted) && *elapsed >= 5.5 {
                        finished_indices.push(idx);
                    }
                }
                ActiveStrike::Rod { target, from, elapsed } => {
                    let _ = from;
                    let target = *target;
                    let old_elapsed = *elapsed;
                    *elapsed += dt;

                    if old_elapsed < 2.5 && *elapsed >= 2.5 {
                        events.push(Event::Carve(Carve::Clear { center: target, radius: 24.0, height: 32.0 }));
                        events.push(Event::Impact {
                            at: target,
                            weapon: Weapon::Rod,
                            radius: 24.0,
                        });
                        events.push(Event::Damage {
                            at: target,
                            radius: 32.0,
                            amount: 300.0,
                            falloff: true,
                        });
                        events.push(Event::Carve(Carve::Bowl {
                            center: target,
                            radius: 24.0,
                            depth: 32.0,
                        }));
                        events.push(Event::Knockback { at: target, radius: 48.0, strength: 2.0 });
                        for wave in super::astral_arsenal::spec(Weapon::Rod).waves {
                            events.push(Event::Wave {
                                center: target,
                                radius: wave.radius,
                                strength: wave.strength,
                                damaging: wave.damaging,
                                sound: wave.sound,
                            });
                        }
                        finished_indices.push(idx);
                    }
                }
                ActiveStrike::Lance { elapsed } => {
                    let energy_needed = 12.0 * dt;
                    if holding && *elapsed < 8.0 && *energy >= energy_needed {
                        events.push(Event::LanceTick { aim, dt });
                        *energy -= energy_needed;
                        *elapsed += dt;
                        if *elapsed >= 8.0 - 1e-6 {
                            finished_indices.push(idx);
                        }
                    } else {
                        finished_indices.push(idx);
                    }
                }
                ActiveStrike::Planned { weapon, plan, next, elapsed, .. } => {
                    *elapsed += dt;
                    while *next < plan.len() && plan[*next].at <= *elapsed {
                        events.push(plan[*next].event);
                        *next += 1;
                    }
                    let strike_spec = super::astral_arsenal::spec(*weapon);
                    if *next >= plan.len() && *elapsed >= strike_spec.duration {
                        finished_indices.push(idx);
                    }
                }
            }
            if events.len() > start {
                events.insert(start, Event::Source { id: self.strike_ids[idx], weapon, center });
            }
        }

        for &idx in finished_indices.iter().rev() {
            self.active_strikes.remove(idx);
            self.strike_ids.remove(idx);
        }

        events
    }

    pub fn cancel(&mut self) {
        self.active_strikes.clear();
        self.strike_ids.clear();
    }
}
