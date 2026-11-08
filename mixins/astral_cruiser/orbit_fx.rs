// The visual effects of orbital strikes: aim rings, ion columns, shells,
// kinetic rods, impact flashes and the cutting lance. Built from light quads.
use super::astral_hull::Bolt;
use glam::DVec3;
use std::f64::consts::TAU;

/// The most bolts `strike_bolts` ever returns in one frame.
pub const FX_BOLTS: usize = 900;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StrikeFx {
    /// The aim ring on the ground; charge 0..1 while an ion beam charges, 0 otherwise.
    Aim { at: DVec3, radius: f64, charge: f32 },
    /// The ion column from the ship's belly to the ground; age 0..1 over its 4 s.
    IonBeam { from: DVec3, to: DVec3, age: f32 },
    /// A falling barrage shell; progress 0..1 from launch to impact.
    Shell { from: DVec3, to: DVec3, progress: f32 },
    /// The kinetic rod falling; progress 0..1.
    Rod { from: DVec3, to: DVec3, progress: f32 },
    /// What a hit leaves: a flash and a dust ring; radius is the strike's, age 0..1.
    Impact { at: DVec3, radius: f64, age: f32 },
    /// The lance beam from the belly to its point on the ground.
    Lance { from: DVec3, to: DVec3 },
}

/// A horizontal ring on the ground at `at.y + 0.3` made of `segments` bolts.
pub fn ground_ring(at: DVec3, radius: f64, segments: u32, width: f32, color: [f32; 4]) -> Vec<Bolt> {
    if segments == 0 {
        return Vec::new();
    }
    let point = |step: u32| {
        let angle = TAU * f64::from(step) / f64::from(segments);
        DVec3::new(at.x + angle.cos() * radius, at.y + 0.3, at.z + angle.sin() * radius)
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

/// A dashed ring on the ground skipping every other segment, rotated by `turn`.
fn dashed_ground_ring(
    at: DVec3,
    radius: f64,
    segments: u32,
    width: f32,
    color: [f32; 4],
    turn: f64,
) -> Vec<Bolt> {
    if segments == 0 {
        return Vec::new();
    }
    let point = |step: u32| {
        let angle = turn + TAU * f64::from(step) / f64::from(segments);
        DVec3::new(at.x + angle.cos() * radius, at.y + 0.3, at.z + angle.sin() * radius)
    };
    (0..segments)
        .filter(|step| step % 2 == 0)
        .map(|step| Bolt {
            from: point(step),
            to: point(step + 1),
            width,
            flash: 0.0,
            color,
        })
        .collect()
}

/// How far the inner aim ring has shrunk, 100% down to 40% at full charge.
pub fn aim_radius(radius: f64, charge: f32) -> f64 {
    radius * (1.0 - 0.6 * f64::from(charge.clamp(0.0, 1.0)))
}

/// A deterministic hash from a 3D position to a 0.0..1.0 float.
fn position_hash(p: DVec3) -> f64 {
    let mut h = p.x.to_bits() ^ p.z.to_bits().rotate_left(17) ^ p.y.to_bits().rotate_left(31);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    (h as f64) / (u64::MAX as f64)
}

/// The light bolts for every orbital effect this frame, up to `FX_BOLTS`.
pub fn strike_bolts(fx: &[StrikeFx]) -> Vec<Bolt> {
    let mut out = Vec::new();
    for effect in fx {
        if out.len() >= FX_BOLTS {
            break;
        }
        match *effect {
            StrikeFx::Aim { at, radius, charge } => {
                let turn = f64::from(charge) * TAU * 0.75;
                let inner_r = aim_radius(radius, charge);
                let pulse = if charge > 0.0 {
                    (charge * 18.0 * std::f32::consts::PI).sin().abs()
                } else {
                    0.0
                };
                let inner_alpha = (0.8 + 0.2 * charge + 0.15 * pulse).clamp(0.0, 1.0);
                let inner_width = 0.6 + 0.3 * charge;
                let inner_color = [1.0, 0.35 + 0.25 * charge, 0.25 + 0.35 * charge, inner_alpha];
                out.extend(ground_ring(at, inner_r, 64, inner_width, inner_color));

                // Dashed outer ring rotating with charge
                let outer_color = [1.0, 0.38, 0.25, 0.35 + 0.15 * charge];
                out.extend(dashed_ground_ring(at, radius, 64, 0.35, outer_color, turn));

                // Four tick marks pointing inward from outer ring
                for i in 0..4 {
                    let angle = turn + f64::from(i) * TAU / 4.0;
                    let from_p = DVec3::new(
                        at.x + angle.cos() * radius * 1.06,
                        at.y + 0.3,
                        at.z + angle.sin() * radius * 1.06,
                    );
                    let to_p = DVec3::new(
                        at.x + angle.cos() * radius * 0.88,
                        at.y + 0.3,
                        at.z + angle.sin() * radius * 0.88,
                    );
                    out.push(Bolt {
                        from: from_p,
                        to: to_p,
                        width: 0.5,
                        flash: 0.0,
                        color: [1.0, 0.45, 0.3, 0.75 + 0.25 * charge],
                    });
                }

                // Small centre glow
                let center_p = DVec3::new(at.x, at.y + 0.3, at.z);
                out.push(Bolt {
                    from: center_p,
                    to: center_p,
                    width: 0.0,
                    flash: 1.5 + 1.5 * charge,
                    color: [1.0, 0.55, 0.35, (0.8 + 0.2 * charge).clamp(0.0, 1.0)],
                });
            }
            StrikeFx::IonBeam { from, to, age } => {
                let a = if age <= 0.8 {
                    1.0
                } else {
                    ((1.0 - age) / 0.2).clamp(0.0, 1.0)
                };

                // 1. White-hot core beam
                out.push(Bolt {
                    from,
                    to,
                    width: 6.0,
                    flash: 0.0,
                    color: [0.95, 0.98, 1.0, a],
                });

                // 2. Wider cyan glow beam
                out.push(Bolt {
                    from,
                    to,
                    width: 15.0,
                    flash: 0.0,
                    color: [0.35, 0.75, 1.0, 0.45 * a],
                });

                // 3. Faint very wide halo beam
                out.push(Bolt {
                    from,
                    to,
                    width: 32.0,
                    flash: 0.0,
                    color: [0.15, 0.45, 0.95, 0.15 * a],
                });

                // 4. Six thin spiral filaments around column
                let axis_dir = (to - from).normalize_or_zero();
                let (perp_x, perp_z) = if axis_dir.x.abs() < 1e-4 && axis_dir.z.abs() < 1e-4 {
                    (DVec3::X, DVec3::Z)
                } else {
                    let right = axis_dir.cross(DVec3::Y).normalize_or_zero();
                    let up = right.cross(axis_dir).normalize_or_zero();
                    (right, up)
                };

                const FILAMENT_STEPS: usize = 10;
                for i in 0..6 {
                    let r_filament = 3.2 + 0.3 * (i as f64);
                    let base_angle = (i as f64) * TAU / 6.0 + f64::from(age) * 20.0;
                    for step in 0..FILAMENT_STEPS {
                        let t0 = step as f64 / FILAMENT_STEPS as f64;
                        let t1 = (step + 1) as f64 / FILAMENT_STEPS as f64;
                        let ang0 = base_angle + t0 * TAU * 2.5;
                        let ang1 = base_angle + t1 * TAU * 2.5;
                        let p0 = from.lerp(to, t0) + (perp_x * ang0.cos() + perp_z * ang0.sin()) * r_filament;
                        let p1 = from.lerp(to, t1) + (perp_x * ang1.cos() + perp_z * ang1.sin()) * r_filament;
                        out.push(Bolt {
                            from: p0,
                            to: p1,
                            width: 0.45,
                            flash: 0.0,
                            color: [0.65, 0.9, 1.0, 0.75 * a],
                        });
                    }
                }

                // 5. Bright ground flash
                out.push(Bolt {
                    from: to,
                    to,
                    width: 0.0,
                    flash: 22.0,
                    color: [0.9, 0.96, 1.0, a],
                });

                // 6. Two expanding ground shock rings + scorch ring at impact radius (16.0)
                let r_shock1 = 4.0 + 18.0 * f64::from(age);
                let a_shock1 = ((1.0 - age) * 0.8).clamp(0.0, 1.0);
                out.extend(ground_ring(to, r_shock1, 32, 1.5, [0.5, 0.85, 1.0, a_shock1]));

                let r_shock2 = 8.0 + 32.0 * f64::from(age);
                let a_shock2 = ((1.0 - age) * (1.0 - age) * 0.6).clamp(0.0, 1.0);
                out.extend(ground_ring(to, r_shock2, 32, 1.0, [0.3, 0.7, 1.0, a_shock2]));

                let scorch_alpha = (0.5 * a).clamp(0.0, 1.0);
                out.extend(ground_ring(to, 16.0, 32, 1.2, [0.25, 0.65, 1.0, scorch_alpha]));
            }
            StrikeFx::Shell { from, to, progress } => {
                let head = from.lerp(to, f64::from(progress));
                let d = from.distance(head);
                let trail_len = d.min(24.0);
                let dir = if d > 1e-9 { (head - from) / d } else { DVec3::NEG_Y };

                let p0 = head;
                let p1 = head - dir * (trail_len * (1.0 / 3.0));
                let p2 = head - dir * (trail_len * (2.0 / 3.0));
                let p3 = head - dir * trail_len;

                // 3 trail segments: front, middle, tail
                out.push(Bolt {
                    from: p1,
                    to: p0,
                    width: 1.5,
                    flash: 0.0,
                    color: [1.0, 0.85, 0.45, 0.95],
                });
                out.push(Bolt {
                    from: p2,
                    to: p1,
                    width: 1.1,
                    flash: 0.0,
                    color: [1.0, 0.6, 0.2, 0.65],
                });
                out.push(Bolt {
                    from: p3,
                    to: p2,
                    width: 0.6,
                    flash: 0.0,
                    color: [0.95, 0.35, 0.1, 0.3],
                });

                // Brighter head glow
                out.push(Bolt {
                    from: head,
                    to: head,
                    width: 0.0,
                    flash: 5.5,
                    color: [1.0, 0.9, 0.7, 1.0],
                });
            }
            StrikeFx::Rod { from, to, progress } => {
                let head = from.lerp(to, f64::from(progress));
                let d = from.distance(head);
                let trail_len = d.min(40.0);
                let dir = if d > 1e-9 { (head - from) / d } else { DVec3::NEG_Y };

                let p0 = head;
                let p1 = head - dir * (trail_len * 0.25);
                let p2 = head - dir * (trail_len * 0.50);
                let p3 = head - dir * (trail_len * 0.75);
                let p4 = head - dir * trail_len;

                // 4 trail segments: white -> yellow -> orange -> dark red
                out.push(Bolt {
                    from: p1,
                    to: p0,
                    width: 3.2,
                    flash: 0.0,
                    color: [1.0, 0.95, 0.9, 1.0],
                });
                out.push(Bolt {
                    from: p2,
                    to: p1,
                    width: 2.8,
                    flash: 0.0,
                    color: [1.0, 0.85, 0.2, 0.85],
                });
                out.push(Bolt {
                    from: p3,
                    to: p2,
                    width: 2.2,
                    flash: 0.0,
                    color: [1.0, 0.5, 0.1, 0.65],
                });
                out.push(Bolt {
                    from: p4,
                    to: p3,
                    width: 1.5,
                    flash: 0.0,
                    color: [0.75, 0.15, 0.05, 0.35],
                });

                // Heat haze glow along the trail
                out.push(Bolt {
                    from: p4,
                    to: p0,
                    width: 7.5,
                    flash: 0.0,
                    color: [1.0, 0.4, 0.1, 0.2],
                });

                // Glowing white-orange head
                out.push(Bolt {
                    from: head,
                    to: head,
                    width: 0.0,
                    flash: 11.0,
                    color: [1.0, 0.88, 0.6, 1.0],
                });
            }
            StrikeFx::Impact { at, radius, age } => {
                let white_flash_size = radius as f32 * 2.2 * (1.0 - age);
                let white_alpha = ((1.0 - age * 3.0).max(0.0) * (1.0 - age)).clamp(0.0, 1.0);
                out.push(Bolt {
                    from: at,
                    to: at,
                    width: 0.0,
                    flash: white_flash_size,
                    color: [1.0, 1.0, 1.0, white_alpha],
                });

                let fire_flash_size = radius as f32 * 1.6 * (1.0 - age);
                let fire_alpha = ((1.0 - age) * (1.0 - age)).clamp(0.0, 1.0);
                out.push(Bolt {
                    from: at,
                    to: at,
                    width: 0.0,
                    flash: fire_flash_size,
                    color: [1.0, 0.55, 0.15, fire_alpha],
                });

                // Fast expanding bright shockwave ring
                let ring_radius = radius * (0.3 + 1.2 * f64::from(age));
                let ring_alpha = ((1.0 - age) * (1.0 - age)).clamp(0.0, 1.0);
                out.extend(ground_ring(
                    at,
                    ring_radius,
                    32,
                    2.2,
                    [1.0, 0.85, 0.6, ring_alpha],
                ));

                // Slower dust ring
                let dust_radius = radius * (0.2 + 0.7 * f64::from(age));
                let dust_alpha = ((1.0 - age) * 0.6).clamp(0.0, 1.0);
                out.extend(ground_ring(
                    at,
                    dust_radius,
                    32,
                    1.8,
                    [0.65, 0.58, 0.5, dust_alpha],
                ));

                // 12 debris streaks shooting outward and up (deterministic angles), shrinking with age
                let shrink = f64::from(1.0 - age);
                let streak_alpha = (1.0 - age).clamp(0.0, 1.0);
                for i in 0..12 {
                    let angle = (i as f64) * TAU / 12.0 + (i as f64 * 1.7).sin() * 0.15;
                    let dist_h = radius * (0.5 + 0.35 * ((i as f64 * 2.3).sin().abs())) * shrink;
                    let dist_v = radius * (0.25 + 0.45 * ((i as f64 * 4.1).cos().abs())) * shrink;
                    let streak_to = at + DVec3::new(angle.cos() * dist_h, dist_v, angle.sin() * dist_h);
                    out.push(Bolt {
                        from: at,
                        to: streak_to,
                        width: 0.35,
                        flash: 0.0,
                        color: [1.0, 0.65, 0.2, streak_alpha],
                    });
                }
            }
            StrikeFx::Lance { from, to } => {
                // 1. Core beam
                out.push(Bolt {
                    from,
                    to,
                    width: 1.5,
                    flash: 0.0,
                    color: [1.0, 0.9, 0.8, 1.0],
                });

                // 2. Red glow beam
                out.push(Bolt {
                    from,
                    to,
                    width: 3.8,
                    flash: 0.0,
                    color: [1.0, 0.25, 0.15, 0.65],
                });

                // 3. Sparks at the ground point (6 short streaks rotating with a hash of the point)
                let base_angle = position_hash(to) * TAU;
                for i in 0..6 {
                    let angle = base_angle + (i as f64) * TAU / 6.0;
                    let spark_len = 1.2 + 0.5 * ((i as f64 * 2.5).sin().abs());
                    let spark_up = 0.25 + 0.35 * ((i as f64 * 3.7).cos().abs());
                    let spark_to = to + DVec3::new(angle.cos() * spark_len, spark_up, angle.sin() * spark_len);
                    out.push(Bolt {
                        from: to,
                        to: spark_to,
                        width: 0.25,
                        flash: 0.0,
                        color: [1.0, 0.75, 0.35, 0.95],
                    });
                }

                // 4. Glow at ground point
                out.push(Bolt {
                    from: to,
                    to,
                    width: 0.0,
                    flash: 5.5,
                    color: [1.0, 0.35, 0.2, 0.9],
                });
            }
        }
    }
    out.truncate(FX_BOLTS);
    out
}
