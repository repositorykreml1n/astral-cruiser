// Timed orbital strike plan events.
// Pure module depending only on std, glam, and sibling modules astral_arsenal and astral_orbit.

use glam::DVec3;
use super::astral_arsenal::{spec, Weapon, WaveSound};
use super::astral_orbit::{Carve, Event, Shown};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timed {
    pub at: f64,
    pub event: Event,
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

pub fn plan(w: Weapon, target: DVec3, ship: DVec3, seed: u64) -> Vec<Timed> {
    let mut plan = Vec::new();
    let mut rng = Rng::new(seed);
    match w {
        Weapon::Ion | Weapon::Barrage | Weapon::Rod | Weapon::Lance => {
            // Empty for the 4 classic weapons
        }
        Weapon::Antimatter => {
            // charge 1.5: Show{Mushroom, target, 16} at 0; at 1.5: Impact{radius 100}, Carve FlatBowl{r 100, depth 40}, Damage{r 120, amount 400, falloff true}, Ignite{r 120}; waves at 1.5+delays.
            plan.push(Timed {
                at: 0.0,
                event: Event::Show { fx: Shown::Mushroom, at: target, seconds: 16.0 },
            });
            plan.push(Timed {
                at: 1.5,
                event: Event::Impact { at: target, weapon: Weapon::Antimatter, radius: 100.0 },
            });
            plan.push(Timed {
                at: 1.5,
                event: Event::Carve(Carve::FlatBowl { center: target, radius: 100.0, depth: 40.0 }),
            });
            plan.push(Timed {
                at: 1.5,
                event: Event::Damage { at: target, radius: 120.0, amount: 400.0, falloff: true },
            });
            plan.push(Timed {
                at: 1.5,
                event: Event::Ignite { at: target, radius: 120.0 },
            });
            for wave in spec(Weapon::Antimatter).waves {
                plan.push(Timed {
                    at: wave.delay,
                    event: Event::Wave {
                        center: target,
                        radius: wave.radius,
                        strength: wave.strength,
                        damaging: wave.damaging,
                        sound: wave.sound,
                    },
                });
            }
        }
        Weapon::SolarPillar => {
            // charge 1.0: Show{Pillar, target, 10} at 0; at 1.0: Carve Shaft{r 25}, Carve Rim{r 25, thickness 3}, Damage{r 30, 300}, Ignite{r 40}; waves.
            plan.push(Timed {
                at: 0.0,
                event: Event::Show { fx: Shown::Pillar, at: target, seconds: 10.0 },
            });
            plan.push(Timed {
                at: 1.0,
                event: Event::Carve(Carve::Shaft { center: target, radius: 25.0 }),
            });
            plan.push(Timed {
                at: 1.0,
                event: Event::Carve(Carve::Rim { center: target, radius: 25.0, thickness: 3.0 }),
            });
            plan.push(Timed {
                at: 1.0,
                event: Event::Damage { at: target, radius: 30.0, amount: 300.0, falloff: false },
            });
            plan.push(Timed {
                at: 1.0,
                event: Event::Ignite { at: target, radius: 40.0 },
            });
            for wave in spec(Weapon::SolarPillar).waves {
                plan.push(Timed {
                    at: wave.delay,
                    event: Event::Wave {
                        center: target,
                        radius: wave.radius,
                        strength: wave.strength,
                        damaging: wave.damaging,
                        sound: wave.sound,
                    },
                });
            }
        }
        Weapon::Tectonic => {
            // charge 0.5: Show{Quake, target, 8} at 0; at 0.5: Carve Fissures{count 10, length 150, depth 20, width 3}, Carve Uplift{rings [(40,6),(85,8),(130,10)]}, Damage{r 60, 120, falloff}; waves.
            plan.push(Timed {
                at: 0.0,
                event: Event::Show { fx: Shown::Quake, at: target, seconds: 8.0 },
            });
            plan.push(Timed {
                at: 0.5,
                event: Event::Carve(Carve::Fissures {
                    center: target,
                    count: 10,
                    length: 150.0,
                    depth: 20.0,
                    width: 3.0,
                }),
            });
            plan.push(Timed {
                at: 0.5,
                event: Event::Carve(Carve::Uplift {
                    center: target,
                    rings: [(40.0, 6.0), (85.0, 8.0), (130.0, 10.0)],
                }),
            });
            plan.push(Timed {
                at: 0.5,
                event: Event::Damage { at: target, radius: 60.0, amount: 120.0, falloff: true },
            });
            for wave in spec(Weapon::Tectonic).waves {
                plan.push(Timed {
                    at: wave.delay,
                    event: Event::Wave {
                        center: target,
                        radius: wave.radius,
                        strength: wave.strength,
                        damaging: wave.damaging,
                        sound: wave.sound,
                    },
                });
            }
        }
        Weapon::Meteors => {
            // 40 Shell{from = target + (horizontal offset 80 along +x rotated by seed, y = target.y + 300), to = random point within r 60 of target, seconds 1.2, weapon Meteors} every 0.15 s from 0; each followed at +1.2 by Carve Sphere{r 5}, Ignite{r 4}, Damage{r 6, 40, falloff}, Impact{radius 5}; final medium wave at 6.0.
            let angle = (seed as f64).rem_euclid(std::f64::consts::TAU);
            let from = DVec3::new(
                target.x + 80.0 * angle.cos(),
                target.y + 300.0,
                target.z + 80.0 * angle.sin(),
            );
            for i in 0..40 {
                let t = i as f64 * 0.15;
                let a = rng.next_f64() * std::f64::consts::TAU;
                let r = rng.next_f64().sqrt() * 60.0;
                let to = DVec3::new(target.x + r * a.cos(), target.y, target.z + r * a.sin());
                plan.push(Timed {
                    at: t,
                    event: Event::Shell { from, to, seconds: 1.2, weapon: Weapon::Meteors },
                });
                plan.push(Timed {
                    at: t + 1.2,
                    event: Event::Carve(Carve::Sphere { center: to, radius: 5.0 }),
                });
                plan.push(Timed {
                    at: t + 1.2,
                    event: Event::Ignite { at: to, radius: 4.0 },
                });
                plan.push(Timed {
                    at: t + 1.2,
                    event: Event::Damage { at: to, radius: 6.0, amount: 40.0, falloff: true },
                });
                plan.push(Timed {
                    at: t + 1.2,
                    event: Event::Impact { at: to, weapon: Weapon::Meteors, radius: 5.0 },
                });
            }
            plan.push(Timed {
                at: 6.0,
                event: Event::Wave {
                    center: target,
                    radius: 150.0,
                    strength: 0.5,
                    damaging: false,
                    sound: WaveSound::Medium,
                },
            });
        }
        Weapon::Cluster => {
            // 60 Shell from ship (seconds 0.8) at i*0.025 to grid-with-jitter points in r 40; each impact: Sphere r 3, Damage r 4 30 falloff, Impact r 3; wave at 1.5.
            for i in 0..60 {
                let t = i as f64 * 0.025;
                let phi = i as f64 * 2.399963229728653;
                let r_base = (i as f64 / 60.0).sqrt() * 38.0;
                let r_jitter = (rng.next_f64() - 0.5) * 2.0;
                let r = (r_base + r_jitter).clamp(0.0, 40.0);
                let to = DVec3::new(target.x + r * phi.cos(), target.y, target.z + r * phi.sin());
                plan.push(Timed {
                    at: t,
                    event: Event::Shell { from: ship, to, seconds: 0.8, weapon: Weapon::Cluster },
                });
                plan.push(Timed {
                    at: t + 0.8,
                    event: Event::Carve(Carve::Sphere { center: to, radius: 3.0 }),
                });
                plan.push(Timed {
                    at: t + 0.8,
                    event: Event::Damage { at: to, radius: 4.0, amount: 30.0, falloff: true },
                });
                plan.push(Timed {
                    at: t + 0.8,
                    event: Event::Impact { at: to, weapon: Weapon::Cluster, radius: 3.0 },
                });
            }
            plan.push(Timed {
                at: 1.5,
                event: Event::Wave {
                    center: target,
                    radius: 150.0,
                    strength: 0.5,
                    damaging: false,
                    sound: WaveSound::Medium,
                },
            });
        }
        Weapon::Cryo => {
            // at 0.5 Show{Frost, target, 2}, Freeze{r 50}, Damage{r 50, 20, falloff}; wave at 0.5.
            plan.push(Timed {
                at: 0.5,
                event: Event::Show { fx: Shown::Frost, at: target, seconds: 2.0 },
            });
            plan.push(Timed {
                at: 0.5,
                event: Event::Freeze { at: target, radius: 50.0 },
            });
            plan.push(Timed {
                at: 0.5,
                event: Event::Damage { at: target, radius: 50.0, amount: 20.0, falloff: true },
            });
            plan.push(Timed {
                at: 0.5,
                event: Event::Wave {
                    center: target,
                    radius: 120.0,
                    strength: 0.5,
                    damaging: false,
                    sound: WaveSound::Frost,
                },
            });
        }
        Weapon::BlackHole => {
            // at 0 Show{BlackHole, target+20y, 8}, Pull{r 60, strength 1, seconds 4}, Carve Sphere{r 12}; at 4.0 Carve Sphere{r 18}, Damage{r 25, 200, falloff}, Knockback{r 60, strength 2}, Impact{radius 18}; wave at 4.0.
            let hole = DVec3::new(target.x, target.y + 20.0, target.z);
            plan.push(Timed {
                at: 0.0,
                event: Event::Show { fx: Shown::BlackHole, at: hole, seconds: 8.0 },
            });
            plan.push(Timed {
                at: 0.0,
                event: Event::Pull { at: target, radius: 60.0, strength: 1.0, seconds: 4.0 },
            });
            plan.push(Timed {
                at: 0.0,
                event: Event::Carve(Carve::Sphere { center: target, radius: 12.0 }),
            });
            plan.push(Timed {
                at: 4.0,
                event: Event::Carve(Carve::Sphere { center: target, radius: 18.0 }),
            });
            plan.push(Timed {
                at: 4.0,
                event: Event::Damage { at: target, radius: 25.0, amount: 200.0, falloff: true },
            });
            plan.push(Timed {
                at: 4.0,
                event: Event::Knockback { at: target, radius: 60.0, strength: 2.0 },
            });
            plan.push(Timed {
                at: 4.0,
                event: Event::Impact { at: target, weapon: Weapon::BlackHole, radius: 18.0 },
            });
            plan.push(Timed {
                at: 4.0,
                event: Event::Wave {
                    center: target,
                    radius: 200.0,
                    strength: 0.7,
                    damaging: false,
                    sound: WaveSound::Large,
                },
            });
        }
        Weapon::Chain => {
            // at 0 Chain{r 80, hops 12, damage 30}.
            plan.push(Timed {
                at: 0.0,
                event: Event::Chain { at: target, radius: 80.0, hops: 12, damage: 30.0 },
            });
        }
        Weapon::PlasmaWall => {
            // line of length 120 through target perpendicular to the ship->target horizontal direction: at 0 Show{Wall, target, 4}, Carve Trench{from,to, width 4, depth 6}, Ignite along the line (5 Ignite r 6 spaced), Damage along line (5 Damage r 4, 60); small wave at 0.5.
            let dx = target.x - ship.x;
            let dz = target.z - ship.z;
            let hlen = (dx * dx + dz * dz).sqrt();
            let (perp_x, perp_z) = if hlen > 1e-6 {
                (-dz / hlen, dx / hlen)
            } else {
                (0.0, 1.0)
            };
            let from = DVec3::new(target.x - 60.0 * perp_x, target.y, target.z - 60.0 * perp_z);
            let to = DVec3::new(target.x + 60.0 * perp_x, target.y, target.z + 60.0 * perp_z);

            plan.push(Timed {
                at: 0.0,
                event: Event::Show { fx: Shown::Wall, at: target, seconds: 4.0 },
            });
            plan.push(Timed {
                at: 0.0,
                event: Event::Carve(Carve::Trench { from, to, width: 4.0, depth: 6.0 }),
            });
            for k in 0..5 {
                let pt = from.lerp(to, k as f64 / 4.0);
                plan.push(Timed {
                    at: 0.0,
                    event: Event::Ignite { at: pt, radius: 6.0 },
                });
                plan.push(Timed {
                    at: 0.0,
                    event: Event::Damage { at: pt, radius: 4.0, amount: 60.0, falloff: false },
                });
            }
            plan.push(Timed {
                at: 0.5,
                event: Event::Wave {
                    center: target,
                    radius: 80.0,
                    strength: 0.3,
                    damaging: false,
                    sound: WaveSound::Small,
                },
            });
        }
        Weapon::Missiles => {
            // 8 Seek{from ship, near target, radius 64, seconds 1.5 + i*0.1} at i*0.12; small wave at 2.5.
            for i in 0..8 {
                let t = i as f64 * 0.12;
                plan.push(Timed {
                    at: t,
                    event: Event::Seek {
                        from: ship,
                        near: target,
                        radius: 64.0,
                        seconds: 1.5 + (i as f64) * 0.1,
                    },
                });
            }
            plan.push(Timed {
                at: 2.5,
                event: Event::Wave {
                    center: target,
                    radius: 60.0,
                    strength: 0.3,
                    damaging: false,
                    sound: WaveSound::Small,
                },
            });
        }
        Weapon::Drill => {
            // at 0 Show{Drill, target, 3}, Carve Shaft{r 4}, Damage{r 4, 80}.
            plan.push(Timed {
                at: 0.0,
                event: Event::Show { fx: Shown::Drill, at: target, seconds: 3.0 },
            });
            plan.push(Timed {
                at: 0.0,
                event: Event::Carve(Carve::Shaft { center: target, radius: 4.0 }),
            });
            plan.push(Timed {
                at: 0.0,
                event: Event::Damage { at: target, radius: 4.0, amount: 80.0, falloff: false },
            });
        }
        Weapon::Grid => {
            // at 0 Show{GridBeams, target, 3}, Carve Grid{half 30, step 12, width 2, depth 8}, Damage at each of the 25 grid intersections r 3 amount 40; small wave at 1.0.
            plan.push(Timed {
                at: 0.0,
                event: Event::Show { fx: Shown::GridBeams, at: target, seconds: 3.0 },
            });
            plan.push(Timed {
                at: 0.0,
                event: Event::Carve(Carve::Grid {
                    center: target,
                    half: 30.0,
                    step: 12.0,
                    width: 2.0,
                    depth: 8.0,
                }),
            });
            for ix in -2..=2 {
                for iz in -2..=2 {
                    let pt = DVec3::new(
                        target.x + ix as f64 * 12.0,
                        target.y,
                        target.z + iz as f64 * 12.0,
                    );
                    plan.push(Timed {
                        at: 0.0,
                        event: Event::Damage { at: pt, radius: 3.0, amount: 40.0, falloff: false },
                    });
                }
            }
            plan.push(Timed {
                at: 1.0,
                event: Event::Wave {
                    center: target,
                    radius: 80.0,
                    strength: 0.3,
                    damaging: false,
                    sound: WaveSound::Small,
                },
            });
        }
        Weapon::Erasure => {
            // shared opening for both: at 0 Show{Scan, target, 10.0}
            plan.push(Timed {
                at: 0.0,
                event: Event::Show {
                    fx: Shown::Scan,
                    at: target,
                    seconds: 10.0,
                },
            });
            // Erasure: at 0 also Show{Erasure, target, 22}
            plan.push(Timed {
                at: 0.0,
                event: Event::Show {
                    fx: Shown::Erasure,
                    at: target,
                    seconds: 22.0,
                },
            });
            // at 10.0 Carve::Erase{center: target, radius 120, plane: target.y - 20}: a flat plain cut 20 deep
            plan.push(Timed {
                at: 10.0,
                event: Event::Carve(Carve::Erase {
                    center: target,
                    radius: 120.0,
                    plane: target.y - 20.0,
                }),
            });
            // Damage r 120 amount 200 falloff false
            plan.push(Timed {
                at: 10.0,
                event: Event::Damage {
                    at: target,
                    radius: 120.0,
                    amount: 200.0,
                    falloff: false,
                },
            });
            // Wave per spec
            for wave in spec(Weapon::Erasure).waves {
                plan.push(Timed {
                    at: wave.delay,
                    event: Event::Wave {
                        center: target,
                        radius: wave.radius,
                        strength: wave.strength,
                        damaging: wave.damaging,
                        sound: wave.sound,
                    },
                });
            }
        }
        Weapon::Supernova => {
            // shared opening for both: at 0 Show{Scan, target, 10.0}
            plan.push(Timed {
                at: 0.0,
                event: Event::Show {
                    fx: Shown::Scan,
                    at: target,
                    seconds: 10.0,
                },
            });
            // Supernova: at 0 Show{Supernova, target + (0, 60, 0), 24}
            plan.push(Timed {
                at: 0.0,
                event: Event::Show {
                    fx: Shown::Supernova,
                    at: target + DVec3::new(0.0, 60.0, 0.0),
                    seconds: 24.0,
                },
            });
            // at 10.0 Impact radius 90
            plan.push(Timed {
                at: 10.0,
                event: Event::Impact {
                    at: target,
                    weapon: Weapon::Supernova,
                    radius: 90.0,
                },
            });
            // Carve::FlatBowl{r 90, depth 50}
            plan.push(Timed {
                at: 10.0,
                event: Event::Carve(Carve::FlatBowl {
                    center: target,
                    radius: 90.0,
                    depth: 50.0,
                }),
            });
            // Damage r 120 amount 500 falloff
            plan.push(Timed {
                at: 10.0,
                event: Event::Damage {
                    at: target,
                    radius: 120.0,
                    amount: 500.0,
                    falloff: true,
                },
            });
            // Ignite r 140
            plan.push(Timed {
                at: 10.0,
                event: Event::Ignite {
                    at: target,
                    radius: 140.0,
                },
            });
            // waves per spec
            for wave in spec(Weapon::Supernova).waves {
                plan.push(Timed {
                    at: wave.delay,
                    event: Event::Wave {
                        center: target,
                        radius: wave.radius,
                        strength: wave.strength,
                        damaging: wave.damaging,
                        sound: wave.sound,
                    },
                });
            }
        }
    }

    // Pair the upward blast with its exact footprint and timestamp. BlackHole
    // clears only on collapse; its opening sphere is intentionally excluded.
    let height = match w {
        Weapon::Antimatter | Weapon::Supernova => 64.0,
        Weapon::BlackHole => 32.0,
        Weapon::Meteors | Weapon::Cluster => 12.0,
        _ => 0.0,
    };
    if height > 0.0 {
        let mut with_clear = Vec::with_capacity(plan.len());
        for timed in plan {
            if w != Weapon::BlackHole || timed.at == 4.0 {
                match timed.event {
                    Event::Carve(Carve::Sphere { center, radius }) |
                    Event::Carve(Carve::FlatBowl { center, radius, .. }) => {
                        with_clear.push(Timed { at: timed.at, event: Event::Carve(Carve::Clear { center, radius, height }) });
                    }
                    _ => {}
                }
            }
            with_clear.push(timed);
        }
        plan = with_clear;
    }
    plan.sort_by(|a, b| a.at.total_cmp(&b.at));
    plan
}
