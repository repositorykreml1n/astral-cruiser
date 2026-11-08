// Spectacular visual effects for orbital weapons and shockwave rings.
// Built from light quads (Bolt).
use super::astral_hull::Bolt;
use super::astral_orbit_fx::ground_ring;
use glam::DVec3;
use std::f64::consts::{PI, TAU};

pub const ARSENAL_BOLTS: usize = 3000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArsenalFx {
    /// Antimatter: white flash sphere, then a fire mushroom: stem from the ground to the cap, cap centred at `top` (world ceiling), cap radius up to 80; grows over the first 6/16 of age, then fades. age 0..1 over 16 s.
    Mushroom { at: DVec3, top: f64, age: f32 },
    /// Solar pillar: plasma column from `top` (world top) down to `bottom` (world floor); radius grows to `radius`; spiralling filaments, ground splash ring. age 0..1 over 10 s.
    Pillar { at: DVec3, top: f64, bottom: f64, radius: f64, age: f32 },
    /// Tectonic hammer: glowing radial cracks of `length` from `at` (seeded by `seed`), dust and rock chunks thrown 30..60 blocks up along them. age 0..1 over 8 s.
    Quake { at: DVec3, length: f64, seed: u64, age: f32 },
    /// A falling burning meteor with a long fiery tail; progress 0..1.
    Meteor { from: DVec3, to: DVec3, progress: f32 },
    /// A homing missile with a smoke/flame trail curving from `from` to `to`; progress 0..1.
    Missile { from: DVec3, to: DVec3, progress: f32 },
    /// Chain lightning: jagged arcs through points[0..count]; flickers; age 0..1 over 1.5 s.
    Chain { points: [DVec3; 13], count: u8, age: f32 },
    /// Black hole hovering at `at` (already 20 blocks above ground): dark core, bright accretion disc, debris spiralling in; collapse 0..1 = the final implosion flash. age 0..1 over 8 s.
    BlackHole { at: DVec3, age: f32, collapse: f32 },
    /// Cryo bomb: icy blue expanding dome and frost shards to `radius`. age 0..1 over 2 s.
    Frost { at: DVec3, radius: f64, age: f32 },
    /// Plasma wall: a line of magenta plasma fire from `from` to `to`, flames 10..20 high. age 0..1 over 4 s.
    Wall { from: DVec3, to: DVec3, age: f32 },
    /// Orbital grid: green beams sweeping along grid lines (square of half-size `half`, spacing `step`). age 0..1 over 3 s.
    Grid { at: DVec3, half: f64, step: f64, age: f32 },
    /// Drill beam: thin intense teal beam from `from` down to `to` with sparks at the bottom. age 0..1 over 3 s.
    Drill { from: DVec3, to: DVec3, age: f32 },
    /// Shockwave: ground ring of dust at radius `front` around `at`, brightness by strength; `super_` adds a translucent hemispherical shell of radius `front`.
    WaveRing { at: DVec3, front: f64, strength: f32, super_: bool },
    /// Supernova: a small sun grows for 10 s (layered glowing sphere, flares, rays); at 10/24 collapse then expand into a huge plasma sphere shell (radius up to 150) and fade. age 0..1 over 24 s.
    Supernova { at: DVec3, age: f32 },
    /// Erasure: glowing dust columns rising from the area of `radius` and dissolving into light, white-cyan. age 0..1 over 22 s (columns after 10/22).
    Erasure { at: DVec3, radius: f64, age: f32 },
}

/// A deterministic pseudo-random float generator in 0.0..1.0 derived from a seed.
fn splitmix64(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^= z >> 31;
    (z as f64) / (u64::MAX as f64)
}

/// A horizontal ring at height `y` made of `segments` bolts.
fn ring_at_y(center: DVec3, y: f64, radius: f64, segments: u32, width: f32, color: [f32; 4]) -> Vec<Bolt> {
    if segments == 0 || radius <= 0.0 {
        return Vec::new();
    }
    let point = |step: u32| {
        let angle = TAU * f64::from(step) / f64::from(segments);
        DVec3::new(center.x + angle.cos() * radius, y, center.z + angle.sin() * radius)
    };
    (0..segments)
        .map(|step| Bolt {
            from: point(step),
            to: point((step + 1) % segments),
            width,
            flash: 0.0,
            color,
        })
        .collect()
}

/// Constructs the light quads (Bolt) for all active arsenal effects this frame,
/// capped at `ARSENAL_BOLTS`.
pub fn arsenal_bolts(fx: &[ArsenalFx]) -> Vec<Bolt> {
    let mut out = Vec::new();
    for effect in fx {
        if out.len() >= ARSENAL_BOLTS {
            break;
        }
        match *effect {
            ArsenalFx::Mushroom { at, top, age } => {
                let age_clamped = age.clamp(0.0, 1.0);
                let grow_t = (age_clamped / (6.0 / 16.0)).min(1.0);
                let fade_t = if age_clamped <= 6.0 / 16.0 {
                    1.0
                } else {
                    ((1.0 - age_clamped) / (10.0 / 16.0)).max(0.0)
                };
                let alpha = fade_t.clamp(0.0, 1.0);
                if alpha <= 0.0 {
                    continue;
                }

                let cap_y = at.y + (top - at.y) * f64::from(grow_t);
                let cap_r = 80.0 * f64::from(grow_t);

                // 1. Initial white/violet flash sphere near ground during early expansion
                if age_clamped < 0.25 {
                    let flash_sz = (1.0 - age_clamped * 4.0).max(0.0) * 110.0;
                    out.push(Bolt {
                        from: at,
                        to: at,
                        width: 0.0,
                        flash: flash_sz,
                        color: [0.95, 0.92, 1.0, alpha],
                    });
                    out.push(Bolt {
                        from: at,
                        to: at,
                        width: 0.0,
                        flash: flash_sz * 0.7,
                        color: [0.75, 0.25, 0.95, alpha * 0.8],
                    });
                }

                // 2. Central pillar/stem from ground to cap_y
                let stem_top_pt = DVec3::new(at.x, cap_y, at.z);
                // White-violet hot core
                out.push(Bolt {
                    from: at,
                    to: stem_top_pt,
                    width: 5.0 * grow_t + 1.2,
                    flash: 0.0,
                    color: [0.95, 0.92, 1.0, alpha],
                });
                // Orange fire column
                out.push(Bolt {
                    from: at,
                    to: stem_top_pt,
                    width: 14.0 * grow_t + 2.5,
                    flash: 0.0,
                    color: [1.0, 0.50, 0.08, alpha * 0.85],
                });
                // Violet outer glow column
                out.push(Bolt {
                    from: at,
                    to: stem_top_pt,
                    width: 25.0 * grow_t + 4.0,
                    flash: 0.0,
                    color: [0.75, 0.20, 0.85, alpha * 0.45],
                });

                // 3. Updraft fire filaments curling up along stem
                const STEM_FILAMENTS: usize = 4;
                const STEM_STEPS: usize = 8;
                for i in 0..STEM_FILAMENTS {
                    let base_ang = (i as f64) * TAU / (STEM_FILAMENTS as f64) + f64::from(age_clamped) * 8.0;
                    let r_stem = (4.0 + 3.0 * (i as f64)) * f64::from(grow_t);
                    for step in 0..STEM_STEPS {
                        let t0 = step as f64 / STEM_STEPS as f64;
                        let t1 = (step + 1) as f64 / STEM_STEPS as f64;
                        let ang0 = base_ang + t0 * TAU * 2.0;
                        let ang1 = base_ang + t1 * TAU * 2.0;
                        let p0 = at.lerp(stem_top_pt, t0) + DVec3::new(ang0.cos() * r_stem, 0.0, ang0.sin() * r_stem);
                        let p1 = at.lerp(stem_top_pt, t1) + DVec3::new(ang1.cos() * r_stem, 0.0, ang1.sin() * r_stem);
                        let col = if i % 2 == 0 {
                            [1.0, 0.45, 0.05, alpha * 0.75]
                        } else {
                            [0.85, 0.30, 0.95, alpha * 0.65]
                        };
                        out.push(Bolt {
                            from: p0,
                            to: p1,
                            width: 1.5,
                            flash: 0.0,
                            color: col,
                        });
                    }
                }

                // 4. Mushroom Cap centered at cap_y (reaching `top` when grow_t == 1.0)
                out.push(Bolt {
                    from: stem_top_pt,
                    to: stem_top_pt,
                    width: 0.0,
                    flash: (cap_r * 0.55) as f32,
                    color: [1.0, 0.85, 0.60, alpha],
                });
                out.push(Bolt {
                    from: stem_top_pt,
                    to: stem_top_pt,
                    width: 0.0,
                    flash: (cap_r * 0.85) as f32,
                    color: [0.85, 0.30, 0.95, alpha * 0.50],
                });

                // Horizontal cap rings
                // Cap crown ring at cap_y (endpoint y = cap_y)
                let ring_crown = ground_ring(
                    DVec3::new(at.x, cap_y - 0.3, at.z),
                    cap_r * 0.60,
                    24,
                    3.0,
                    [1.0, 0.75, 0.30, alpha * 0.85],
                );
                out.extend(ring_crown);
                // Outer rim ring
                let ring_rim = ground_ring(
                    DVec3::new(at.x, cap_y - 4.3, at.z),
                    cap_r,
                    24,
                    4.0,
                    [1.0, 0.40, 0.05, alpha * 0.75],
                );
                out.extend(ring_rim);
                // Under-cap roll ring
                let ring_under = ground_ring(
                    DVec3::new(at.x, cap_y - 10.3, at.z),
                    cap_r * 0.75,
                    24,
                    2.5,
                    [0.75, 0.20, 0.85, alpha * 0.60],
                );
                out.extend(ring_under);

                // Radial billowing lobes curling from stem out to cap rim
                const CAP_LOBES: usize = 8;
                for i in 0..CAP_LOBES {
                    let ang = (i as f64) * TAU / (CAP_LOBES as f64);
                    let dir = DVec3::new(ang.cos(), 0.0, ang.sin());
                    let p0 = stem_top_pt - DVec3::new(0.0, 8.0, 0.0) + dir * (cap_r * 0.2);
                    let p1 = stem_top_pt + DVec3::new(0.0, 1.0, 0.0) + dir * (cap_r * 0.5);
                    let p2 = stem_top_pt - DVec3::new(0.0, 4.0, 0.0) + dir * cap_r;
                    let p3 = stem_top_pt - DVec3::new(0.0, 12.0, 0.0) + dir * (cap_r * 0.75);
                    out.push(Bolt { from: p0, to: p1, width: 2.2, flash: 0.0, color: [1.0, 0.65, 0.20, alpha * 0.8] });
                    out.push(Bolt { from: p1, to: p2, width: 2.5, flash: 0.0, color: [1.0, 0.45, 0.10, alpha * 0.8] });
                    out.push(Bolt { from: p2, to: p3, width: 2.0, flash: 0.0, color: [0.80, 0.25, 0.90, alpha * 0.7] });
                }

                // 5. Ground base splash ring
                let base_r = (cap_r * 0.35).max(8.0);
                out.extend(ground_ring(at, base_r, 24, 2.0, [1.0, 0.45, 0.10, alpha * 0.60]));
            }
            ArsenalFx::Pillar { at, top, bottom, radius, age } => {
                let age_clamped = age.clamp(0.0, 1.0);
                let grow = (age_clamped / 0.25).min(1.0) as f64;
                let alpha = if age_clamped <= 0.8 {
                    1.0
                } else {
                    ((1.0 - age_clamped) / 0.2).clamp(0.0, 1.0)
                };
                if alpha <= 0.0 {
                    continue;
                }

                let cur_r = (4.0 + (radius - 4.0) * grow).max(1.0);
                let top_pt = DVec3::new(at.x, top, at.z);
                let bot_pt = DVec3::new(at.x, bottom, at.z);

                // 1. Bright white-gold core cylinder (dense, smooth vertical quads around axis)
                // Center-most core beams
                out.push(Bolt {
                    from: top_pt,
                    to: bot_pt,
                    width: (cur_r * 0.4) as f32,
                    flash: 0.0,
                    color: [1.0, 1.0, 0.95, alpha],
                });
                out.push(Bolt {
                    from: top_pt,
                    to: bot_pt,
                    width: (cur_r * 0.85) as f32,
                    flash: 0.0,
                    color: [1.0, 0.96, 0.82, alpha * 0.9],
                });

                // Inner core ring of 24 thin vertical quads around the axis
                const CORE_RAYS: usize = 24;
                let r_core = cur_r * 0.35;
                for i in 0..CORE_RAYS {
                    let ang = (i as f64) * TAU / (CORE_RAYS as f64);
                    let offset = DVec3::new(ang.cos() * r_core, 0.0, ang.sin() * r_core);
                    out.push(Bolt {
                        from: top_pt + offset,
                        to: bot_pt + offset,
                        width: (cur_r * 0.25) as f32,
                        flash: 0.0,
                        color: [1.0, 0.97, 0.88, alpha * 0.85],
                    });
                }

                // Mid core ring of 16 vertical quads
                const MID_RAYS: usize = 16;
                let r_mid = cur_r * 0.7;
                for i in 0..MID_RAYS {
                    let ang = (i as f64) * TAU / (MID_RAYS as f64) + 0.1;
                    let offset = DVec3::new(ang.cos() * r_mid, 0.0, ang.sin() * r_mid);
                    out.push(Bolt {
                        from: top_pt + offset,
                        to: bot_pt + offset,
                        width: (cur_r * 0.4) as f32,
                        flash: 0.0,
                        color: [1.0, 0.90, 0.60, alpha * 0.75],
                    });
                }

                // 2. Soft wide outer glow (wide translucent vertical quads around beam)
                const OUTER_GLOW_RAYS: usize = 12;
                let r_outer = cur_r * 1.3;
                for i in 0..OUTER_GLOW_RAYS {
                    let ang = (i as f64) * TAU / (OUTER_GLOW_RAYS as f64);
                    let offset = DVec3::new(ang.cos() * r_outer, 0.0, ang.sin() * r_outer);
                    out.push(Bolt {
                        from: top_pt + offset,
                        to: bot_pt + offset,
                        width: (cur_r * 1.4) as f32,
                        flash: 0.0,
                        color: [1.0, 0.78, 0.30, alpha * 0.22],
                    });
                }
                out.push(Bolt {
                    from: top_pt,
                    to: bot_pt,
                    width: (cur_r * 2.8) as f32,
                    flash: 0.0,
                    color: [1.0, 0.65, 0.15, alpha * 0.16],
                });

                // 3. Slow spiralling motes (soft light particles drifting around the column)
                const MOTE_COUNT: usize = 64;
                for m in 0..MOTE_COUNT {
                    let t_m = (m as f64 + 0.5) / (MOTE_COUNT as f64);
                    let drift = (f64::from(age_clamped) * 0.15 + m as f64 * 0.073).fract();
                    let norm_y = (t_m + drift).fract();
                    let y = bottom + (top - bottom) * norm_y;
                    let ang = m as f64 * 2.39996 + f64::from(age_clamped) * 0.8;
                    let r_mote = cur_r * (0.5 + 0.8 * ((m as f64 * 1.9).sin().abs()));
                    let mote_pos = DVec3::new(at.x + ang.cos() * r_mote, y, at.z + ang.sin() * r_mote);
                    out.push(Bolt {
                        from: mote_pos,
                        to: mote_pos + DVec3::new(0.0, 1.5, 0.0),
                        width: 0.7,
                        flash: 0.0,
                        color: [1.0, 0.96, 0.75, alpha * 0.65],
                    });
                }

                // 4. Heat-shimmer ring at the ground
                let shimmer_osc = (f64::from(age_clamped) * 16.0).sin() * 0.5;
                let ground_y = bottom + 0.1;
                let shimmer_r1 = cur_r * 1.4 + shimmer_osc;
                out.extend(ground_ring(
                    DVec3::new(at.x, ground_y, at.z),
                    shimmer_r1,
                    32,
                    2.0,
                    [1.0, 0.88, 0.40, alpha * 0.85],
                ));
                let shimmer_r2 = cur_r * 2.0 - shimmer_osc;
                out.extend(ground_ring(
                    DVec3::new(at.x, ground_y, at.z),
                    shimmer_r2,
                    32,
                    1.6,
                    [1.0, 0.70, 0.20, alpha * 0.50],
                ));

                // Ground & ceiling glow flash
                out.push(Bolt {
                    from: bot_pt,
                    to: bot_pt,
                    width: 0.0,
                    flash: (cur_r * 3.5) as f32,
                    color: [1.0, 0.95, 0.75, alpha * 0.8],
                });
                out.push(Bolt {
                    from: top_pt,
                    to: top_pt,
                    width: 0.0,
                    flash: (cur_r * 2.0) as f32,
                    color: [1.0, 0.98, 0.85, alpha * 0.8],
                });
            }
            ArsenalFx::Quake { at, length, seed, age } => {
                let age_clamped = age.clamp(0.0, 1.0);
                let progress = (age_clamped / 0.2).min(1.0) as f64;
                let alpha = ((1.0 - age_clamped) * 1.25).clamp(0.0, 1.0);
                if alpha <= 0.0 {
                    continue;
                }

                let mut rng_state = seed.wrapping_add(1);
                let crack_count = 8;
                for k in 0..crack_count {
                    let base_ang = (k as f64) * TAU / (crack_count as f64) + (splitmix64(&mut rng_state) - 0.5) * 0.3;
                    let crack_len = length * (0.75 + 0.35 * splitmix64(&mut rng_state)) * progress;
                    const SEGMENTS: usize = 6;
                    let mut prev_pt = DVec3::new(at.x, at.y + 0.3, at.z);
                    for s in 1..=SEGMENTS {
                        let t = s as f64 / (SEGMENTS as f64);
                        let r = crack_len * t;
                        let ang_jitter = (splitmix64(&mut rng_state) - 0.5) * 0.25;
                        let next_pt = DVec3::new(
                            at.x + (base_ang + ang_jitter).cos() * r,
                            at.y + 0.3,
                            at.z + (base_ang + ang_jitter).sin() * r,
                        );
                        // Hot inner amber crack
                        out.push(Bolt {
                            from: prev_pt,
                            to: next_pt,
                            width: 1.2,
                            flash: 0.0,
                            color: [1.0, 0.75, 0.25, alpha],
                        });
                        // Outer brown/amber glow
                        out.push(Bolt {
                            from: prev_pt,
                            to: next_pt,
                            width: 2.8,
                            flash: 0.0,
                            color: [0.72, 0.38, 0.08, alpha * 0.60],
                        });

                        // Dust and rock chunks thrown 30..60 blocks up along them
                        if s % 2 == 0 {
                            let h_max = 30.0 + 30.0 * splitmix64(&mut rng_state);
                            let y_lift = h_max * 4.0 * (age_clamped * (1.0 - age_clamped)) as f64;
                            let rock_pos = next_pt + DVec3::new(
                                (splitmix64(&mut rng_state) - 0.5) * 2.0,
                                y_lift,
                                (splitmix64(&mut rng_state) - 0.5) * 2.0,
                            );
                            out.push(Bolt {
                                from: rock_pos,
                                to: rock_pos + DVec3::new(0.0, 1.2, 0.0),
                                width: 0.8,
                                flash: 0.0,
                                color: [0.90, 0.55, 0.15, alpha * 0.90],
                            });
                            out.push(Bolt {
                                from: rock_pos,
                                to: rock_pos,
                                width: 0.0,
                                flash: 2.2,
                                color: [1.0, 0.65, 0.20, alpha * 0.80],
                            });
                        }
                        prev_pt = next_pt;
                    }
                }

                // Center ground flash and expanding ground shock rings
                out.push(Bolt {
                    from: at + DVec3::new(0.0, 0.3, 0.0),
                    to: at + DVec3::new(0.0, 0.3, 0.0),
                    width: 0.0,
                    flash: 16.0 * (1.0 - age_clamped),
                    color: [1.0, 0.75, 0.30, alpha],
                });
                out.extend(ground_ring(at, 12.0 * progress, 24, 2.0, [1.0, 0.65, 0.15, alpha * 0.70]));
                out.extend(ground_ring(at, 28.0 * progress, 32, 1.4, [0.72, 0.38, 0.08, alpha * 0.50]));
            }
            ArsenalFx::Meteor { from, to, progress } => {
                let p_clamp = progress.clamp(0.0, 1.0);
                let head = from.lerp(to, f64::from(p_clamp));
                let dist = from.distance(head);
                let trail_len = dist.min(50.0);
                let dir = if dist > 1e-6 { (head - from) / dist } else { DVec3::NEG_Y };

                // 4 trail segments: front hot core -> middle fire -> rear flame -> tail smoke
                let p0 = head;
                let p1 = head - dir * (trail_len * 0.20);
                let p2 = head - dir * (trail_len * 0.45);
                let p3 = head - dir * (trail_len * 0.75);
                let p4 = head - dir * trail_len;

                // Front core
                out.push(Bolt { from: p1, to: p0, width: 2.6, flash: 0.0, color: [1.0, 0.85, 0.40, 1.0] });
                // Middle fire
                out.push(Bolt { from: p2, to: p1, width: 2.2, flash: 0.0, color: [1.0, 0.45, 0.10, 0.85] });
                // Rear flame
                out.push(Bolt { from: p3, to: p2, width: 1.6, flash: 0.0, color: [1.0, 0.25, 0.05, 0.65] });
                // Tail smoke
                out.push(Bolt { from: p4, to: p3, width: 1.0, flash: 0.0, color: [0.75, 0.12, 0.02, 0.35] });

                // Wide fiery envelope
                out.push(Bolt { from: p4, to: p0, width: 6.5, flash: 0.0, color: [1.0, 0.30, 0.05, 0.25] });

                // Head fireball flash
                out.push(Bolt { from: head, to: head, width: 0.0, flash: 8.5, color: [1.0, 0.80, 0.35, 1.0] });

                // Shedding sparks
                let seed = from.x.to_bits().wrapping_add((progress * 100.0) as u64);
                let mut r = seed;
                for _ in 0..3 {
                    let spark_off = DVec3::new(
                        (splitmix64(&mut r) - 0.5) * 3.0,
                        (splitmix64(&mut r) - 0.5) * 3.0,
                        (splitmix64(&mut r) - 0.5) * 3.0,
                    );
                    let sp_from = head - dir * (trail_len * 0.3);
                    out.push(Bolt {
                        from: sp_from,
                        to: sp_from + spark_off,
                        width: 0.35,
                        flash: 0.0,
                        color: [1.0, 0.65, 0.20, 0.80],
                    });
                }
            }
            ArsenalFx::Missile { from, to, progress } => {
                let p_clamp = progress.clamp(0.0, 1.0);
                let dir = to - from;
                let dist = dir.length();
                let cross = if dir.x.abs() < 1e-4 && dir.z.abs() < 1e-4 {
                    DVec3::X
                } else {
                    dir.cross(DVec3::Y).normalize_or_zero()
                };
                let ctrl = from.lerp(to, 0.5) + DVec3::Y * (dist * 0.22).min(35.0) + cross * (dist * 0.12).min(20.0);
                let bezier = |t: f32| {
                    let t64 = f64::from(t);
                    let inv = 1.0 - t64;
                    from * (inv * inv) + ctrl * (2.0 * inv * t64) + to * (t64 * t64)
                };

                let head = bezier(p_clamp);
                // Head tip glow (pink-red)
                out.push(Bolt { from: head, to: head, width: 0.0, flash: 4.8, color: [1.0, 0.85, 0.90, 1.0] });

                // Curving flame & smoke trail
                const TRAIL_STEPS: usize = 5;
                let step_len = 0.04_f32;
                for i in 0..TRAIL_STEPS {
                    let t1 = (p_clamp - (i as f32) * step_len).max(0.0);
                    let t0 = (p_clamp - ((i + 1) as f32) * step_len).max(0.0);
                    if t1 > t0 {
                        let pt0 = bezier(t0);
                        let pt1 = bezier(t1);
                        let frac = 1.0 - (i as f32 / TRAIL_STEPS as f32);
                        let (width, col) = if i == 0 {
                            (1.6, [1.0, 0.85, 0.90, 0.95])
                        } else if i < 3 {
                            (1.4, [1.0, 0.25, 0.45, 0.85 * frac])
                        } else {
                            (1.8, [0.70, 0.35, 0.45, 0.45 * frac])
                        };
                        out.push(Bolt { from: pt0, to: pt1, width, flash: 0.0, color: col });
                    }
                }
            }
            ArsenalFx::Chain { points, count, age } => {
                let age_clamped = age.clamp(0.0, 1.0);
                let flicker = (1.0 - age_clamped) * (0.65 + 0.35 * (age_clamped * 35.0).sin().abs());
                let alpha = (flicker * 1.2).clamp(0.0, 1.0);
                if alpha <= 0.0 || count < 2 {
                    continue;
                }

                let active_count = (count as usize).min(13);
                for i in 0..(active_count - 1) {
                    let p_start = points[i];
                    let p_end = points[i + 1];

                    // Direct glow connecting the pair
                    out.push(Bolt {
                        from: p_start,
                        to: p_end,
                        width: 1.8,
                        flash: 0.0,
                        color: [0.50, 0.75, 1.0, alpha * 0.45],
                    });

                    // Jagged lightning segments
                    const JAGGED_SEGS: usize = 4;
                    let diff = p_end - p_start;
                    let dist = diff.length();
                    let axis = if dist > 1e-6 { diff / dist } else { DVec3::Y };
                    let perp = if axis.x.abs() < 1e-4 && axis.z.abs() < 1e-4 {
                        DVec3::X
                    } else {
                        axis.cross(DVec3::Y).normalize_or(DVec3::X)
                    };
                    let perp2 = axis.cross(perp).normalize_or(DVec3::Z);

                    let mut curr = p_start;
                    for s in 1..=JAGGED_SEGS {
                        let next = if s == JAGGED_SEGS {
                            p_end
                        } else {
                            let t = s as f64 / JAGGED_SEGS as f64;
                            let j_seed = (i * 10 + s) as u64;
                            let j1 = (f64::from(age_clamped) * 30.0 + (j_seed as f64 * 3.7)).sin() * (dist * 0.08).min(2.5);
                            let j2 = (f64::from(age_clamped) * 27.0 + (j_seed as f64 * 5.1)).cos() * (dist * 0.08).min(2.5);
                            p_start.lerp(p_end, t) + perp * j1 + perp2 * j2
                        };
                        out.push(Bolt {
                            from: curr,
                            to: next,
                            width: 0.8,
                            flash: 0.0,
                            color: [0.88, 0.94, 1.0, alpha],
                        });
                        curr = next;
                    }

                    // Node flash at point i
                    out.push(Bolt {
                        from: p_start,
                        to: p_start,
                        width: 0.0,
                        flash: 3.5 * flicker,
                        color: [0.88, 0.94, 1.0, alpha],
                    });
                }
                // Last node flash
                let last_pt = points[active_count - 1];
                out.push(Bolt {
                    from: last_pt,
                    to: last_pt,
                    width: 0.0,
                    flash: 3.5 * flicker,
                    color: [0.88, 0.94, 1.0, alpha],
                });
            }
            ArsenalFx::BlackHole { at, age, collapse } => {
                let age_clamped = age.clamp(0.0, 1.0);
                let col_clamped = collapse.clamp(0.0, 1.0);
                let alpha = ((1.0 - age_clamped) * 1.2).clamp(0.0, 1.0);
                if alpha <= 0.0 && col_clamped <= 0.0 {
                    continue;
                }

                let disc_r = 16.0 * (1.0 - f64::from(col_clamped) * 0.6);

                // 1. Dark core
                out.push(Bolt {
                    from: at,
                    to: at,
                    width: 0.0,
                    flash: 5.5 * (1.0 - col_clamped * 0.5),
                    color: [0.05, 0.02, 0.08, 0.95],
                });

                // 2. Bright accretion disc (violet 0xA040FF: [0.627, 0.251, 1.0])
                out.extend(ground_ring(
                    DVec3::new(at.x, at.y - 0.3, at.z),
                    disc_r * 0.45,
                    24,
                    2.0,
                    [0.92, 0.70, 1.0, alpha],
                ));
                out.extend(ground_ring(
                    DVec3::new(at.x, at.y - 0.3, at.z),
                    disc_r * 0.75,
                    32,
                    2.8,
                    [0.627, 0.251, 1.0, alpha * 0.90],
                ));
                out.extend(ground_ring(
                    DVec3::new(at.x, at.y - 0.3, at.z),
                    disc_r,
                    32,
                    3.5,
                    [0.45, 0.15, 0.85, alpha * 0.50],
                ));

                // 3. Debris spiralling inward
                const DEBRIS: usize = 14;
                for i in 0..DEBRIS {
                    let seed = i as f64 / (DEBRIS as f64);
                    let way = (seed + f64::from(age_clamped) * 2.0).fract();
                    let inward = 1.0 - way;
                    let r_deb = (disc_r * 0.5) + (35.0 * inward);
                    let ang = seed * TAU * 3.0 + f64::from(age_clamped) * 10.0;
                    let y_off = (seed - 0.5) * 10.0 * inward;
                    let p0 = at + DVec3::new(ang.cos() * r_deb, y_off, ang.sin() * r_deb);
                    let p1 = at + DVec3::new((ang + 0.15).cos() * (r_deb - 2.5), y_off * 0.85, (ang + 0.15).sin() * (r_deb - 2.5));
                    out.push(Bolt {
                        from: p0,
                        to: p1,
                        width: 0.6,
                        flash: 0.0,
                        color: [0.75, 0.40, 1.0, alpha * (0.3 + 0.7 * way as f32)],
                    });
                }

                // 4. Final implosion flash if collapsing
                if col_clamped > 0.0 {
                    let flash_sz = col_clamped * 48.0;
                    out.push(Bolt {
                        from: at,
                        to: at,
                        width: 0.0,
                        flash: flash_sz,
                        color: [0.98, 0.95, 1.0, col_clamped],
                    });
                    out.push(Bolt {
                        from: at,
                        to: at,
                        width: 0.0,
                        flash: flash_sz * 1.5,
                        color: [0.627, 0.251, 1.0, col_clamped * 0.70],
                    });
                    let impl_r = 30.0 * f64::from(col_clamped);
                    out.extend(ground_ring(
                        DVec3::new(at.x, at.y - 0.3, at.z),
                        impl_r,
                        32,
                        2.5,
                        [0.80, 0.50, 1.0, (1.0 - col_clamped) * 0.80],
                    ));
                }
            }
            ArsenalFx::Frost { at, radius, age } => {
                let age_clamped = age.clamp(0.0, 1.0);
                let alpha = ((1.0 - age_clamped) * (1.0 - age_clamped * 0.5)).clamp(0.0, 1.0);
                if alpha <= 0.0 {
                    continue;
                }

                let grow = f64::from(age_clamped).sqrt();
                let cur_r = radius * grow;

                // Icy blue 0x90E8FF: [0.565, 0.910, 1.0]
                // 1. Ground rings
                out.extend(ground_ring(at, cur_r, 32, 2.2, [0.565, 0.910, 1.0, alpha * 0.90]));
                out.extend(ground_ring(at, cur_r * 0.65, 24, 1.8, [0.80, 0.95, 1.0, alpha * 0.70]));

                // 2. Hemispherical dome rings & meridian ribs
                let dome_h = cur_r * 0.65;
                let ring_mid_r = cur_r * 0.80;
                let ring_mid_y = at.y + dome_h * 0.5;
                out.extend(ground_ring(
                    DVec3::new(at.x, ring_mid_y - 0.3, at.z),
                    ring_mid_r,
                    24,
                    1.5,
                    [0.565, 0.910, 1.0, alpha * 0.60],
                ));

                const DOME_RIBS: usize = 6;
                for i in 0..DOME_RIBS {
                    let ang = (i as f64) * PI / (DOME_RIBS as f64);
                    let dir = DVec3::new(ang.cos(), 0.0, ang.sin());
                    let p_ground_a = at + dir * cur_r;
                    let p_ground_b = at - dir * cur_r;
                    let p_mid_a = at + dir * ring_mid_r + DVec3::new(0.0, dome_h * 0.5, 0.0);
                    let p_mid_b = at - dir * ring_mid_r + DVec3::new(0.0, dome_h * 0.5, 0.0);
                    let p_top = at + DVec3::new(0.0, dome_h, 0.0);

                    out.push(Bolt { from: p_ground_a, to: p_mid_a, width: 1.2, flash: 0.0, color: [0.565, 0.910, 1.0, alpha * 0.65] });
                    out.push(Bolt { from: p_mid_a, to: p_top, width: 1.2, flash: 0.0, color: [0.80, 0.95, 1.0, alpha * 0.75] });
                    out.push(Bolt { from: p_top, to: p_mid_b, width: 1.2, flash: 0.0, color: [0.80, 0.95, 1.0, alpha * 0.75] });
                    out.push(Bolt { from: p_mid_b, to: p_ground_b, width: 1.2, flash: 0.0, color: [0.565, 0.910, 1.0, alpha * 0.65] });
                }

                // 3. Frost shards bursting out
                const SHARDS: usize = 12;
                for i in 0..SHARDS {
                    let ang = (i as f64) * TAU / (SHARDS as f64) + 0.2;
                    let dir = DVec3::new(ang.cos(), 0.0, ang.sin());
                    let base_pt = at + dir * (cur_r * 0.7) + DVec3::new(0.0, 0.5, 0.0);
                    let tip_pt = at + dir * (cur_r * 1.05) + DVec3::new(0.0, 3.0 + (i as f64 * 1.3).sin().abs() * 3.0, 0.0);
                    out.push(Bolt {
                        from: base_pt,
                        to: tip_pt,
                        width: 0.8,
                        flash: 0.0,
                        color: [0.88, 0.97, 1.0, alpha * 0.85],
                    });
                }

                // Center burst flash
                out.push(Bolt {
                    from: at + DVec3::new(0.0, 0.3, 0.0),
                    to: at + DVec3::new(0.0, 0.3, 0.0),
                    width: 0.0,
                    flash: (cur_r * 0.8) as f32,
                    color: [0.75, 0.92, 1.0, alpha],
                });
            }
            ArsenalFx::Wall { from, to, age } => {
                let age_clamped = age.clamp(0.0, 1.0);
                let alpha = ((1.0 - age_clamped) * 1.2).clamp(0.0, 1.0);
                if alpha <= 0.0 {
                    continue;
                }

                // Magenta 0xFF40A0: [1.0, 0.251, 0.627]
                // 1. Ground baseline plasma line
                out.push(Bolt {
                    from,
                    to,
                    width: 3.5,
                    flash: 0.0,
                    color: [1.0, 0.251, 0.627, alpha * 0.90],
                });
                out.push(Bolt {
                    from,
                    to,
                    width: 1.2,
                    flash: 0.0,
                    color: [1.0, 0.85, 0.95, alpha],
                });

                // 2. Rising flames 10..20 blocks high along the line
                let dist = from.distance(to);
                let num_flames = (dist / 3.0).clamp(6.0, 30.0) as usize;
                for i in 0..num_flames {
                    let t = i as f64 / (num_flames - 1).max(1) as f64;
                    let base = from.lerp(to, t);
                    let h = 10.0 + 10.0 * (t * 17.3 + f64::from(age_clamped) * 2.0).sin().abs();
                    let sway = DVec3::new(
                        (f64::from(age_clamped) * 12.0 + t * 15.0).sin() * 1.5,
                        h,
                        (f64::from(age_clamped) * 14.0 + t * 18.0).cos() * 1.5,
                    );
                    let tip = base + sway;

                    out.push(Bolt {
                        from: base,
                        to: tip,
                        width: 1.8,
                        flash: 0.0,
                        color: [1.0, 0.251, 0.627, alpha * 0.85],
                    });
                    out.push(Bolt {
                        from: base,
                        to: base.lerp(tip, 0.6),
                        width: 0.8,
                        flash: 0.0,
                        color: [1.0, 0.85, 0.95, alpha],
                    });
                    out.push(Bolt {
                        from: tip,
                        to: tip + DVec3::new(0.0, 1.5, 0.0),
                        width: 0.4,
                        flash: 0.0,
                        color: [1.0, 0.50, 0.80, alpha * 0.70],
                    });
                }
            }
            ArsenalFx::Grid { at, half, step, age } => {
                let age_clamped = age.clamp(0.0, 1.0);
                let alpha = ((1.0 - age_clamped) * 1.2).clamp(0.0, 1.0);
                if alpha <= 0.0 || step <= 1e-4 {
                    continue;
                }

                // Green 0x40FF60: [0.251, 1.0, 0.376]
                let y = at.y + 0.3;
                let mut curr_offset = -half;
                while curr_offset <= half + 1e-4 {
                    // Lines along X
                    let from_x = DVec3::new(at.x - half, y, at.z + curr_offset);
                    let to_x = DVec3::new(at.x + half, y, at.z + curr_offset);
                    out.push(Bolt {
                        from: from_x,
                        to: to_x,
                        width: 0.6,
                        flash: 0.0,
                        color: [0.251, 1.0, 0.376, alpha * 0.45],
                    });

                    // Lines along Z
                    let from_z = DVec3::new(at.x + curr_offset, y, at.z - half);
                    let to_z = DVec3::new(at.x + curr_offset, y, at.z + half);
                    out.push(Bolt {
                        from: from_z,
                        to: to_z,
                        width: 0.6,
                        flash: 0.0,
                        color: [0.251, 1.0, 0.376, alpha * 0.45],
                    });

                    curr_offset += step;
                }

                // Sweeping vertical orbital laser beams along grid lines
                const SWEEP_BEAMS: usize = 3;
                for b in 0..SWEEP_BEAMS {
                    let sweep_t = (f64::from(age_clamped) * 2.0 + (b as f64) * 0.33).fract();
                    let sweep_x = at.x - half + (2.0 * half) * sweep_t;
                    let sweep_z = at.z - half + (2.0 * half) * (sweep_t * 1.5).fract();
                    let beam_top = DVec3::new(sweep_x, at.y + 180.0, sweep_z);
                    let beam_bot = DVec3::new(sweep_x, y, sweep_z);

                    // Core beam
                    out.push(Bolt {
                        from: beam_top,
                        to: beam_bot,
                        width: 1.2,
                        flash: 0.0,
                        color: [0.85, 1.0, 0.90, alpha],
                    });
                    // Main green beam
                    out.push(Bolt {
                        from: beam_top,
                        to: beam_bot,
                        width: 3.2,
                        flash: 0.0,
                        color: [0.251, 1.0, 0.376, alpha * 0.85],
                    });
                    // Ground impact flash
                    out.push(Bolt {
                        from: beam_bot,
                        to: beam_bot,
                        width: 0.0,
                        flash: 6.0,
                        color: [0.45, 1.0, 0.65, alpha],
                    });
                }
            }
            ArsenalFx::Drill { from, to, age } => {
                let age_clamped = age.clamp(0.0, 1.0);
                let alpha = if age_clamped <= 0.85 {
                    1.0
                } else {
                    ((1.0 - age_clamped) / 0.15).clamp(0.0, 1.0)
                };
                if alpha <= 0.0 {
                    continue;
                }

                // Teal 0x40FFB0: [0.251, 1.0, 0.690]
                // 1. Intense white-hot core
                out.push(Bolt {
                    from,
                    to,
                    width: 0.8,
                    flash: 0.0,
                    color: [0.92, 1.0, 0.97, alpha],
                });
                // 2. Teal main beam
                out.push(Bolt {
                    from,
                    to,
                    width: 2.2,
                    flash: 0.0,
                    color: [0.251, 1.0, 0.690, alpha * 0.85],
                });
                // 3. Faint outer glow
                out.push(Bolt {
                    from,
                    to,
                    width: 5.5,
                    flash: 0.0,
                    color: [0.10, 0.80, 0.55, alpha * 0.30],
                });

                // 4. Helical spiral around drill beam
                const SPIRAL_STEPS: usize = 12;
                let axis = (to - from).normalize_or_zero();
                let perp = if axis.x.abs() < 1e-4 && axis.z.abs() < 1e-4 {
                    DVec3::X
                } else {
                    axis.cross(DVec3::Y).normalize_or(DVec3::X)
                };
                let perp2 = axis.cross(perp).normalize_or(DVec3::Z);
                for s in 0..SPIRAL_STEPS {
                    let t0 = s as f64 / (SPIRAL_STEPS as f64);
                    let t1 = (s + 1) as f64 / (SPIRAL_STEPS as f64);
                    let ang0 = t0 * TAU * 4.0 + f64::from(age_clamped) * 25.0;
                    let ang1 = t1 * TAU * 4.0 + f64::from(age_clamped) * 25.0;
                    let p0 = from.lerp(to, t0) + (perp * ang0.cos() + perp2 * ang0.sin()) * 1.2;
                    let p1 = from.lerp(to, t1) + (perp * ang1.cos() + perp2 * ang1.sin()) * 1.2;
                    out.push(Bolt {
                        from: p0,
                        to: p1,
                        width: 0.6,
                        flash: 0.0,
                        color: [0.45, 1.0, 0.80, alpha * 0.75],
                    });
                }

                // 5. Grinding sparks at bottom
                const SPARKS: usize = 10;
                for i in 0..SPARKS {
                    let ang = (i as f64) * TAU / (SPARKS as f64) + f64::from(age_clamped) * 30.0;
                    let sp_len = 2.0 + ((i as f64 * 3.1).sin().abs()) * 2.5;
                    let sp_up = 1.0 + ((i as f64 * 4.7).cos().abs()) * 3.0;
                    let sp_to = to + DVec3::new(ang.cos() * sp_len, sp_up, ang.sin() * sp_len);
                    out.push(Bolt {
                        from: to,
                        to: sp_to,
                        width: 0.35,
                        flash: 0.0,
                        color: [0.85, 1.0, 0.95, alpha * 0.90],
                    });
                }

                // 6. Impact glow at to
                out.push(Bolt {
                    from: to,
                    to,
                    width: 0.0,
                    flash: 8.0,
                    color: [0.45, 1.0, 0.80, alpha * 0.90],
                });
            }
            ArsenalFx::WaveRing { at, front, strength, super_ } => {
                let str_clamped = strength.clamp(0.0, 2.0);
                if str_clamped <= 0.0 || front <= 0.0 {
                    continue;
                }

                let wall_h = 6.0 + 34.0 * f64::from(strength);
                let segments = ((36.0 + front * 0.13).round() as u32).clamp(32, 100);
                let alpha = (str_clamped * 0.85).clamp(0.0, 1.0);

                // 1. Stacked wall bands at radius front
                // Dense dusty bright at the bottom fading to transparent at the top
                const BAND_FRACTIONS: [f64; 5] = [0.0, 0.22, 0.48, 0.74, 1.0];
                for &frac in &BAND_FRACTIONS {
                    let y = at.y + 0.3 * (1.0 - frac) + wall_h * frac;
                    let r = 0.94 - 0.06 * frac as f32;
                    let g = 0.84 + 0.04 * frac as f32;
                    let b = 0.68 + 0.26 * frac as f32;
                    let band_alpha = (alpha * (1.0 - frac as f32 * 0.85)).clamp(0.0, 1.0);
                    let band_width = (3.2 - 1.6 * frac as f32) * str_clamped;
                    out.extend(ring_at_y(at, y, front, segments, band_width, [r, g, b, band_alpha]));
                }

                // 2. Extra dense dusty base ring near ground
                out.extend(ring_at_y(
                    at,
                    at.y + 0.3,
                    front + 0.5,
                    segments,
                    2.0 * str_clamped,
                    [0.90, 0.80, 0.70, alpha * 0.50],
                ));

                // 3. Dust trail ring slightly behind front (front-6..front-2)
                let trail_r = if front >= 6.0 { front - 4.0 } else { front * 0.6 };
                out.extend(ring_at_y(
                    at,
                    at.y + 0.3,
                    trail_r,
                    segments,
                    2.5 * str_clamped,
                    [0.82, 0.72, 0.60, alpha * 0.55],
                ));

                // 4. For super_, a bright white-hot crest at the top edge
                if super_ {
                    out.extend(ring_at_y(
                        at,
                        at.y + wall_h,
                        front,
                        segments,
                        3.6 * str_clamped,
                        [1.0, 0.98, 0.94, alpha],
                    ));
                }
            }
            ArsenalFx::Supernova { at, age } => {
                let age_clamped = age.clamp(0.0, 1.0);
                const TRANS: f32 = 10.0 / 24.0;
                if age_clamped < TRANS {
                    // Phase 1: Small sun grows for 10 s (layered glowing sphere, flares, rays)
                    let grow = (f64::from(age_clamped) / f64::from(TRANS)).clamp(0.0, 1.0);
                    let r_sun = 3.0 + 25.0 * grow;
                    let intensity = (0.5 + 0.5 * grow) as f32;

                    // Core flashes
                    out.push(Bolt {
                        from: at,
                        to: at,
                        width: 0.0,
                        flash: (r_sun * 1.5) as f32,
                        color: [1.0, 1.0, 0.95, intensity],
                    });
                    out.push(Bolt {
                        from: at,
                        to: at,
                        width: 0.0,
                        flash: (r_sun * 2.6) as f32,
                        color: [1.0, 0.75, 0.25, intensity * 0.7],
                    });

                    // Layered glowing sphere: rings in multiple planes
                    let r_inner = r_sun * 0.6;
                    out.extend(ring_at_y(at, at.y, r_inner, 24, 2.0, [1.0, 0.98, 0.90, intensity * 0.9]));
                    out.extend(ring_at_y(at, at.y, r_sun, 32, 2.5, [1.0, 0.80, 0.25, intensity * 0.85]));
                    out.extend(ring_at_y(at, at.y + r_sun * 0.4, r_sun * 0.85, 24, 1.8, [1.0, 0.70, 0.20, intensity * 0.7]));
                    out.extend(ring_at_y(at, at.y - r_sun * 0.4, r_sun * 0.85, 24, 1.8, [1.0, 0.70, 0.20, intensity * 0.7]));

                    // Vertical meridian circles around the growing sun
                    const MERIDIANS: usize = 4;
                    for m in 0..MERIDIANS {
                        let m_ang = (m as f64) * PI / (MERIDIANS as f64) + f64::from(age_clamped) * 1.5;
                        const STEPS: usize = 16;
                        for s in 0..STEPS {
                            let a0 = (s as f64) * TAU / (STEPS as f64);
                            let a1 = ((s + 1) as f64) * TAU / (STEPS as f64);
                            let p0 = at + DVec3::new(m_ang.cos() * a0.cos() * r_sun, a0.sin() * r_sun, m_ang.sin() * a0.cos() * r_sun);
                            let p1 = at + DVec3::new(m_ang.cos() * a1.cos() * r_sun, a1.sin() * r_sun, m_ang.sin() * a1.cos() * r_sun);
                            out.push(Bolt {
                                from: p0,
                                to: p1,
                                width: 2.0,
                                flash: 0.0,
                                color: [1.0, 0.85, 0.30, intensity * 0.8],
                            });
                        }
                    }

                    // Corona flares & rays
                    const RAYS: usize = 32;
                    for i in 0..RAYS {
                        let ang_u = (i as f64 * 2.39996) + f64::from(age_clamped) * 3.5;
                        let ang_v = (i as f64 * 1.7) + f64::from(age_clamped) * 2.1;
                        let dir = DVec3::new(ang_u.cos() * ang_v.cos(), ang_v.sin(), ang_u.sin() * ang_v.cos());
                        let ray_len = r_sun * (0.8 + 0.9 * ((i as f64 * 3.7 + f64::from(age_clamped) * 10.0).sin().abs()));
                        out.push(Bolt {
                            from: at + dir * (r_sun * 0.8),
                            to: at + dir * (r_sun + ray_len),
                            width: 1.8,
                            flash: 0.0,
                            color: [1.0, 0.88, 0.35, intensity * 0.8],
                        });
                    }
                } else {
                    // Phase 2: at 10/24 collapse then expand into huge plasma sphere shell (radius up to 150) and fade
                    let post = ((f64::from(age_clamped) - f64::from(TRANS)) / (1.0 - f64::from(TRANS))).clamp(0.0, 1.0);
                    if post < 0.04 {
                        // Rapid collapse: core contracts with blinding flash
                        let col_t = post / 0.04;
                        let r_col = (1.0 - col_t) * 25.0 + 2.0;
                        out.push(Bolt {
                            from: at,
                            to: at,
                            width: 0.0,
                            flash: (r_col * 4.0) as f32,
                            color: [1.0, 1.0, 1.0, 1.0],
                        });
                        out.extend(ring_at_y(at, at.y, r_col, 24, 3.0, [1.0, 1.0, 1.0, 1.0]));
                    } else {
                        // Expansion into plasma sphere shell of radius up to 150
                        let exp_t = (post - 0.04) / 0.96;
                        let r_shell = 150.0 * (1.0 - (1.0 - exp_t).powi(2));
                        let fade = ((1.0 - exp_t) * 1.15).clamp(0.0, 1.0) as f32;
                        if fade > 0.0 {
                            // Central blast flash
                            out.push(Bolt {
                                from: at,
                                to: at,
                                width: 0.0,
                                flash: (r_shell * 0.35).min(60.0) as f32,
                                color: [1.0, 0.90, 0.65, fade],
                            });

                            // Latitude rings of the plasma sphere shell
                            const LATS: [f64; 7] = [-1.0, -0.65, -0.3, 0.0, 0.3, 0.65, 1.0];
                            for &lat_sin in &LATS {
                                let lat_cos = (1.0 - lat_sin * lat_sin).max(0.0).sqrt();
                                let ring_r = r_shell * lat_cos;
                                let ring_y = at.y + r_shell * lat_sin;
                                if ring_r > 1.0 {
                                    let segs = if lat_sin.abs() < 0.1 { 36 } else { 24 };
                                    out.extend(ring_at_y(
                                        DVec3::new(at.x, ring_y, at.z),
                                        ring_y,
                                        ring_r,
                                        segs,
                                        2.8,
                                        [1.0, 0.70, 0.28, fade * 0.85],
                                    ));
                                }
                            }

                            // Meridians on the sphere
                            const MERIDIANS: usize = 6;
                            for m in 0..MERIDIANS {
                                let m_ang = (m as f64) * PI / (MERIDIANS as f64);
                                const STEPS: usize = 16;
                                for s in 0..STEPS {
                                    let a0 = (s as f64) * PI / (STEPS as f64) - PI * 0.5;
                                    let a1 = ((s + 1) as f64) * PI / (STEPS as f64) - PI * 0.5;
                                    let p0 = at + DVec3::new(m_ang.cos() * a0.cos() * r_shell, a0.sin() * r_shell, m_ang.sin() * a0.cos() * r_shell);
                                    let p1 = at + DVec3::new(m_ang.cos() * a1.cos() * r_shell, a1.sin() * r_shell, m_ang.sin() * a1.cos() * r_shell);
                                    out.push(Bolt {
                                        from: p0,
                                        to: p1,
                                        width: 2.2,
                                        flash: 0.0,
                                        color: [1.0, 0.85, 0.40, fade * 0.75],
                                    });
                                }
                            }

                            // Radial plasma discharge filaments
                            const DISCHARGES: usize = 36;
                            for i in 0..DISCHARGES {
                                let ang_u = i as f64 * 2.39996 + f64::from(age_clamped) * 2.0;
                                let ang_v = (i as f64 * 1.5).sin() * 1.2;
                                let dir = DVec3::new(ang_u.cos() * ang_v.cos(), ang_v.sin(), ang_u.sin() * ang_v.cos());
                                out.push(Bolt {
                                    from: at + dir * (r_shell * 0.88),
                                    to: at + dir * (r_shell * 1.10),
                                    width: 2.0,
                                    flash: 0.0,
                                    color: [1.0, 0.60, 0.15, fade * 0.8],
                                });
                            }
                        }
                    }
                }
            }
            ArsenalFx::Erasure { at, radius, age } => {
                let age_clamped = age.clamp(0.0, 1.0);
                const TRANS: f32 = 10.0 / 22.0;
                if age_clamped < TRANS {
                    // Pre-charge ambient: subtle converging energy motes and faint cyan ring
                    let prog = (f64::from(age_clamped) / f64::from(TRANS)).clamp(0.0, 1.0);
                    let alpha = (prog * 0.5) as f32;
                    let r_ring = radius * (1.0 - prog * 0.2);
                    out.extend(ground_ring(at, r_ring, 32, 1.5, [0.49, 0.97, 1.0, alpha]));
                } else {
                    // After 10/22: glowing dust columns rising from the area and dissolving into light, white-cyan
                    let post = ((f64::from(age_clamped) - f64::from(TRANS)) / (1.0 - f64::from(TRANS))).clamp(0.0, 1.0);
                    let alpha = ((1.0 - post) * 1.2).clamp(0.0, 1.0) as f32;
                    if alpha > 0.0 {
                        const COLS: usize = 42;
                        for c in 0..COLS {
                            let theta = c as f64 * 2.399963;
                            let frac = (c as f64 + 0.5) / (COLS as f64);
                            let r = radius * frac.sqrt();
                            let col_x = at.x + r * theta.cos();
                            let col_z = at.z + r * theta.sin();
                            let max_h = 35.0 + 35.0 * ((c as f64 * 1.7).sin().abs());
                            let rise_h = max_h * post.min(0.65) / 0.65;

                            const SEGS: usize = 5;
                            for s in 0..SEGS {
                                let y0 = at.y + rise_h * (s as f64 / SEGS as f64);
                                let y1 = at.y + rise_h * ((s + 1) as f64 / SEGS as f64);
                                let seg_fade = 1.0 - (s as f32 / SEGS as f32) * 0.65;
                                let p0 = DVec3::new(col_x, y0, col_z);
                                let p1 = DVec3::new(col_x, y1, col_z);
                                out.push(Bolt {
                                    from: p0,
                                    to: p1,
                                    width: 1.4,
                                    flash: 0.0,
                                    color: [0.49, 0.97, 1.0, alpha * seg_fade * 0.9],
                                });
                            }
                            let top_pos = DVec3::new(col_x, at.y + rise_h, col_z);
                            out.push(Bolt {
                                from: top_pos,
                                to: top_pos,
                                width: 0.0,
                                flash: 2.2,
                                color: [0.85, 0.98, 1.0, alpha * 0.8],
                            });
                            let mote_pos = top_pos + DVec3::new(0.0, post * 8.0, 0.0);
                            out.push(Bolt {
                                from: mote_pos,
                                to: mote_pos + DVec3::new(0.0, 1.5, 0.0),
                                width: 0.8,
                                flash: 0.0,
                                color: [0.90, 1.0, 1.0, alpha * 0.65],
                            });
                        }

                        out.extend(ground_ring(at, radius, 48, 2.2, [0.49, 0.97, 1.0, alpha * 0.8]));
                        out.push(Bolt {
                            from: at,
                            to: at,
                            width: 0.0,
                            flash: (radius * 0.4).min(40.0) as f32,
                            color: [0.75, 0.97, 1.0, alpha],
                        });
                    }
                }
            }
        }
    }
    out.truncate(ARSENAL_BOLTS);
    out
}

/// Territory highlight effect during the 10 s charge:
/// contour-like grid lines draped on the terrain (sampled every ~3 blocks),
/// a bright scanning ring sweeping outward then back,
/// shimmering color/brightness travelling along lines with time,
/// and intensity rising toward the end of the charge.
///
/// Budget capped at 1200 bolts for radius 120.
pub fn scan_bolts(
    at: DVec3,
    radius: f64,
    age: f32,
    color: [f32; 3],
    height: impl Fn(f64, f64) -> Option<f64>,
) -> Vec<Bolt> {
    if radius <= 0.0 {
        return Vec::new();
    }
    const MAX_SCAN_BOLTS: usize = 1200;
    let mut out = Vec::with_capacity(MAX_SCAN_BOLTS);

    let age_clamped = age.clamp(0.0, 1.0);
    // Intensity rising toward the end of the 10 s charge
    let intensity = (0.65 + 0.55 * f64::from(age_clamped * age_clamped)) as f32;

    // Bright scanning ring sweeping outward then back
    let sweep_frac = (f64::from(age_clamped) * PI).sin();
    let sweep_r = radius * sweep_frac;

    // 1. Bright scanning ring sweeping outward then back (built first so it is never starved)
    const RING_SEGS: usize = 48;
    if sweep_r > 1.5 {
        let mut ring_pts: Vec<(f64, Option<f64>, f64)> = Vec::with_capacity(RING_SEGS);
        for i in 0..RING_SEGS {
            let ang = (i as f64) * TAU / (RING_SEGS as f64);
            let rx = at.x + sweep_r * ang.cos();
            let rz = at.z + sweep_r * ang.sin();
            let ry = height(rx, rz).map(|h| h + 0.2);
            ring_pts.push((rx, ry, rz));
        }

        for i in 0..RING_SEGS {
            let j = (i + 1) % RING_SEGS;
            if let (Some(y0), Some(y1)) = (ring_pts[i].1, ring_pts[j].1) {
                let from = DVec3::new(ring_pts[i].0, y0, ring_pts[i].2);
                let to = DVec3::new(ring_pts[j].0, y1, ring_pts[j].2);
                let ring_alpha = (0.9 + 0.1 * intensity).clamp(0.0, 1.0);
                out.push(Bolt {
                    from,
                    to,
                    width: 2.6,
                    flash: 0.8,
                    color: [
                        (color[0] * 1.8 + 0.2).min(2.0),
                        (color[1] * 1.8 + 0.2).min(2.0),
                        (color[2] * 1.8 + 0.2).min(2.0),
                        ring_alpha,
                    ],
                });
            }
        }
    }

    // 2. Draped grid lines along X and Z
    let line_spacing = (radius / 5.0).max(22.0);
    let sample_step = 3.2; // step along each line every ~3 blocks
    let r_sq = radius * radius;

    let num_lines = (radius / line_spacing).floor() as i32;

    // Grid lines parallel to Z (fixed X)
    for i in -num_lines..=num_lines {
        if out.len() >= MAX_SCAN_BOLTS {
            break;
        }
        let offset = i as f64 * line_spacing;
        if offset.abs() >= radius {
            continue;
        }
        let half_chord = (r_sq - offset * offset).max(0.0).sqrt();
        let world_x = at.x + offset;

        let mut prev_pt: Option<DVec3> = None;
        let mut z = -half_chord;
        while z <= half_chord + 1e-6 {
            if out.len() >= MAX_SCAN_BOLTS {
                break;
            }
            let world_z = at.z + z;
            if let Some(h) = height(world_x, world_z) {
                let curr_pt = DVec3::new(world_x, h + 0.1, world_z);
                if let Some(prev) = prev_pt {
                    let dist = (world_x - at.x).hypot(world_z - at.z);
                    let shimmer = ((dist * 0.15 - f64::from(age_clamped) * 12.0).sin() * 0.5 + 0.5) as f32;
                    let mut b = intensity * (0.85 + 0.45 * shimmer);
                    let ring_diff = (dist - sweep_r).abs();
                    if ring_diff < 10.0 {
                        b = (b + 0.8 * (1.0 - (ring_diff / 10.0) as f32)).min(2.5);
                    }
                    let alpha = (0.75 + 0.25 * intensity).clamp(0.0, 1.0);
                    out.push(Bolt {
                        from: prev,
                        to: curr_pt,
                        width: 1.6,
                        flash: 0.0,
                        color: [
                            (color[0] * b).min(2.0),
                            (color[1] * b).min(2.0),
                            (color[2] * b).min(2.0),
                            alpha,
                        ],
                    });
                }
                prev_pt = Some(curr_pt);
            } else {
                prev_pt = None;
            }
            z += sample_step;
        }
    }

    // Grid lines parallel to X (fixed Z)
    for i in -num_lines..=num_lines {
        if out.len() >= MAX_SCAN_BOLTS {
            break;
        }
        let offset = i as f64 * line_spacing;
        if offset.abs() >= radius {
            continue;
        }
        let half_chord = (r_sq - offset * offset).max(0.0).sqrt();
        let world_z = at.z + offset;

        let mut prev_pt: Option<DVec3> = None;
        let mut x = -half_chord;
        while x <= half_chord + 1e-6 {
            if out.len() >= MAX_SCAN_BOLTS {
                break;
            }
            let world_x = at.x + x;
            if let Some(h) = height(world_x, world_z) {
                let curr_pt = DVec3::new(world_x, h + 0.1, world_z);
                if let Some(prev) = prev_pt {
                    let dist = (world_x - at.x).hypot(world_z - at.z);
                    let shimmer = ((dist * 0.15 - f64::from(age_clamped) * 12.0).sin() * 0.5 + 0.5) as f32;
                    let mut b = intensity * (0.85 + 0.45 * shimmer);
                    let ring_diff = (dist - sweep_r).abs();
                    if ring_diff < 10.0 {
                        b = (b + 0.8 * (1.0 - (ring_diff / 10.0) as f32)).min(2.5);
                    }
                    let alpha = (0.75 + 0.25 * intensity).clamp(0.0, 1.0);
                    out.push(Bolt {
                        from: prev,
                        to: curr_pt,
                        width: 1.6,
                        flash: 0.0,
                        color: [
                            (color[0] * b).min(2.0),
                            (color[1] * b).min(2.0),
                            (color[2] * b).min(2.0),
                            alpha,
                        ],
                    });
                }
                prev_pt = Some(curr_pt);
            } else {
                prev_pt = None;
            }
            x += sample_step;
        }
    }

    out.truncate(MAX_SCAN_BOLTS);
    out
}
