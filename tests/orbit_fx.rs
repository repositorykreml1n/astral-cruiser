#[path = "../mixins/astral_cruiser/layout.rs"]
#[allow(unused)]
mod astral_layout;
#[path = "../mixins/astral_cruiser/furnish.rs"]
#[allow(unused)]
mod astral_furnish;
#[path = "../mixins/astral_cruiser/hull_mesh.rs"]
#[allow(unused)]
mod astral_hull;
#[path = "../mixins/astral_cruiser/orbit_fx.rs"]
mod astral_orbit_fx;

use astral_orbit_fx::{aim_radius, ground_ring, strike_bolts, StrikeFx, FX_BOLTS};
use glam::DVec3;

#[test]
fn ground_ring_points_lie_on_the_radius() {
    let at = DVec3::new(10.0, 64.0, -20.0);
    let radius = 15.0;
    let segments = 32;
    let ring = ground_ring(at, radius, segments, 0.5, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(ring.len(), segments as usize);
    for bolt in &ring {
        for pt in [bolt.from, bolt.to] {
            let horiz_dist = (pt.x - at.x).hypot(pt.z - at.z);
            assert!(
                (horiz_dist - radius).abs() < 1e-9,
                "endpoint horizontal distance {horiz_dist} != {radius}"
            );
            assert!(
                (pt.y - (at.y + 0.3)).abs() < 1e-9,
                "endpoint y {} != {}",
                pt.y,
                at.y + 0.3
            );
        }
    }
    let first = &ring[0];
    let last = &ring[ring.len() - 1];
    assert!(
        (last.to - first.from).length() < 1e-9,
        "last endpoint does not match first start"
    );
}

#[test]
fn aim_ring_shrinks_while_charging() {
    assert_eq!(aim_radius(16.0, 0.0), 16.0);
    assert!((aim_radius(16.0, 1.0) - 6.4).abs() < 1e-9);

    let at = DVec3::new(0.0, 64.0, 0.0);
    let bolts = strike_bolts(&[StrikeFx::Aim {
        at,
        radius: 16.0,
        charge: 0.5,
    }]);
    // Inner ring (64) + dashed outer ring (32) + 4 ticks (4) + centre glow (1) = 101
    assert_eq!(bolts.len(), 101);

    // Inner ring points lie on the converged aim radius
    let expected_r = aim_radius(16.0, 0.5);
    for bolt in &bolts[0..64] {
        let r_from = (bolt.from.x - at.x).hypot(bolt.from.z - at.z);
        assert!(
            (r_from - expected_r).abs() < 1e-6,
            "inner ring radius {r_from} != {expected_r}"
        );
    }

    // Small centre glow at the center
    let center_glow = bolts.last().unwrap();
    assert_eq!(center_glow.width, 0.0);
    assert!(center_glow.flash > 0.0);
    assert_eq!(center_glow.from, DVec3::new(at.x, at.y + 0.3, at.z));
}

#[test]
fn ion_beam_fades_only_at_the_end() {
    let from = DVec3::new(0.0, 200.0, 0.0);
    let to = DVec3::new(0.0, 64.0, 0.0);

    let b_half = strike_bolts(&[StrikeFx::IonBeam { from, to, age: 0.5 }]);
    assert_eq!(b_half[0].color[3], 1.0);

    let b_late = strike_bolts(&[StrikeFx::IonBeam { from, to, age: 0.9 }]);
    assert!(
        (b_late[0].color[3] - 0.5).abs() < 1e-6,
        "core alpha at 0.9 was {}",
        b_late[0].color[3]
    );

    let b_end = strike_bolts(&[StrikeFx::IonBeam { from, to, age: 1.0 }]);
    assert_eq!(b_end[0].color[3], 0.0);
}

#[test]
fn ion_beam_filaments_stay_within_six_blocks_of_axis() {
    let from = DVec3::new(0.0, 200.0, 0.0);
    let to = DVec3::new(0.0, 64.0, 0.0);
    let bolts = strike_bolts(&[StrikeFx::IonBeam { from, to, age: 0.3 }]);

    // Filaments are bolts index 3 .. 3 + 60
    let filament_bolts = &bolts[3..63];
    assert_eq!(filament_bolts.len(), 60);
    for bolt in filament_bolts {
        for pt in [bolt.from, bolt.to] {
            let dist_to_axis = pt.x.hypot(pt.z);
            assert!(
                dist_to_axis <= 6.0,
                "filament point {pt:?} distance to axis {dist_to_axis} > 6.0"
            );
            assert!(
                pt.y >= to.y - 1e-6 && pt.y <= from.y + 1e-6,
                "filament point {pt:?} y out of bounds"
            );
        }
    }
}

#[test]
fn shell_head_moves_from_launch_to_impact() {
    let from = DVec3::new(0.0, 200.0, 0.0);
    let to = DVec3::new(0.0, 60.0, 0.0);

    let bolts = strike_bolts(&[StrikeFx::Shell {
        from,
        to,
        progress: 0.5,
    }]);
    let midpoint = (from + to) * 0.5;
    // Front segment ends at head
    assert_eq!(bolts[0].to, midpoint);
    // Head flash is at head
    assert_eq!(bolts[3].to, midpoint);
    assert_eq!(bolts[3].from, midpoint);
    assert_eq!(bolts[3].width, 0.0);
    assert!(bolts[3].flash > 0.0);

    // Total trail length
    let trail_len = (bolts[0].to - bolts[2].from).length();
    assert!(
        trail_len <= 24.0 + 1e-9,
        "trail length {trail_len} exceeded 24"
    );
    // Falling alpha and width across the 3 segments
    assert!(bolts[0].color[3] > bolts[1].color[3]);
    assert!(bolts[1].color[3] > bolts[2].color[3]);
    assert!(bolts[0].width > bolts[1].width);
    assert!(bolts[1].width > bolts[2].width);

    let bolts_near = strike_bolts(&[StrikeFx::Shell {
        from,
        to,
        progress: 0.05,
    }]);
    let near_head = from.lerp(to, f64::from(0.05_f32));
    // Tail segment starts at from when distance < trail length
    assert_eq!(bolts_near[2].from, from);
    assert!((bolts_near[0].to - near_head).length() < 1e-9);
    let near_len = (bolts_near[0].to - bolts_near[2].from).length();
    assert!(
        near_len <= 24.0 + 1e-9,
        "near trail length {near_len} exceeded 24"
    );
}

#[test]
fn rod_trail_is_forty_long() {
    let from = DVec3::new(0.0, 300.0, 0.0);
    let to = DVec3::new(0.0, 60.0, 0.0);

    let bolts = strike_bolts(&[StrikeFx::Rod {
        from,
        to,
        progress: 0.5,
    }]);
    // 4 trail segments: front is bolts[0], tail is bolts[3]
    let trail_len = (bolts[0].to - bolts[3].from).length();
    assert!(
        (trail_len - 40.0).abs() < 1e-9,
        "trail length was {trail_len}, expected 40"
    );
    // Heat haze glow along the trail
    assert_eq!(bolts[4].from, bolts[3].from);
    assert_eq!(bolts[4].to, bolts[0].to);
    assert!(bolts[4].width > 5.0);
    // Head flash
    assert_eq!(bolts[5].flash, 11.0);
}

#[test]
fn impact_ring_grows_and_fades() {
    let at = DVec3::new(5.0, 64.0, 10.0);
    let r = 20.0;

    let b_0 = strike_bolts(&[StrikeFx::Impact {
        at,
        radius: r,
        age: 0.0,
    }]);
    // b_0[0] is white flash, b_0[1] is fireball, b_0[2] is first shockwave ring bolt
    let r0 = (b_0[2].from.x - at.x).hypot(b_0[2].from.z - at.z);
    assert!(
        (r0 - 0.3 * r).abs() < 1e-9,
        "radius at age 0 was {r0}, expected {}",
        0.3 * r
    );

    let b_1 = strike_bolts(&[StrikeFx::Impact {
        at,
        radius: r,
        age: 1.0,
    }]);
    let r1 = (b_1[2].from.x - at.x).hypot(b_1[2].from.z - at.z);
    assert!(
        (r1 - 1.5 * r).abs() < 1e-9,
        "radius at age 1 was {r1}, expected {}",
        1.5 * r
    );
    assert_eq!(b_1[2].color[3], 0.0, "ring alpha at age 1 was not 0");
    assert_eq!(b_1[0].color[3], 0.0, "white flash alpha at age 1 was not 0");
    assert_eq!(b_1[0].flash, 0.0, "white flash size at age 1 was not 0");
    assert_eq!(b_1[1].color[3], 0.0, "fireball alpha at age 1 was not 0");
    assert_eq!(b_1[1].flash, 0.0, "fireball size at age 1 was not 0");
}

#[test]
fn impact_debris_streaks_start_at_impact_point() {
    let at = DVec3::new(10.0, 64.0, 15.0);
    let r = 20.0;

    let b_early = strike_bolts(&[StrikeFx::Impact {
        at,
        radius: r,
        age: 0.2,
    }]);
    // 1 white flash + 1 fireball + 32 shock ring + 32 dust ring = 66
    // Debris streaks are index 66..78 (12 bolts)
    let debris_early = &b_early[66..78];
    assert_eq!(debris_early.len(), 12);
    for streak in debris_early {
        assert_eq!(streak.from, at, "debris streak did not start at impact point");
        assert!(streak.to.y > at.y, "debris streak did not shoot upward");
    }

    let b_late = strike_bolts(&[StrikeFx::Impact {
        at,
        radius: r,
        age: 0.8,
    }]);
    let debris_late = &b_late[66..78];
    for (early, late) in debris_early.iter().zip(debris_late.iter()) {
        let len_early = (early.to - early.from).length();
        let len_late = (late.to - late.from).length();
        assert!(
            len_late < len_early,
            "debris streak did not shrink with age: early {len_early}, late {len_late}"
        );
    }
}

#[test]
fn lance_draws_a_beam_to_its_point() {
    let from = DVec3::new(10.0, 300.0, 20.0);
    let to = DVec3::new(15.0, 64.0, 25.0);
    let bolts = strike_bolts(&[StrikeFx::Lance { from, to }]);
    assert_eq!(bolts.len(), 9);

    // Core beam
    assert_eq!(bolts[0].from, from);
    assert_eq!(bolts[0].to, to);
    assert_eq!(bolts[0].width, 1.5);
    assert_eq!(bolts[0].flash, 0.0);

    // Red glow beam
    assert_eq!(bolts[1].from, from);
    assert_eq!(bolts[1].to, to);
    assert_eq!(bolts[1].width, 3.8);
    assert_eq!(bolts[1].flash, 0.0);

    // 6 sparks starting at `to`
    for spark in &bolts[2..8] {
        assert_eq!(spark.from, to);
        assert!((spark.to - to).length() > 0.5);
    }

    // Glow at target
    let glow = &bolts[8];
    assert_eq!(glow.from, to);
    assert_eq!(glow.to, to);
    assert_eq!(glow.width, 0.0);
    assert_eq!(glow.flash, 5.5);
}

#[test]
fn strike_bolts_never_exceeds_the_budget() {
    let impacts: Vec<StrikeFx> = (0..200)
        .map(|i| StrikeFx::Impact {
            at: DVec3::new(i as f64, 64.0, 0.0),
            radius: 10.0,
            age: 0.2,
        })
        .collect();
    let bolts = strike_bolts(&impacts);
    assert!(bolts.len() <= FX_BOLTS);
    assert_eq!(bolts.len(), FX_BOLTS);
}

#[test]
fn all_bolts_have_finite_coordinates_and_valid_alpha() {
    let from = DVec3::new(12.0, 240.0, -18.0);
    let to = DVec3::new(14.0, 64.0, -22.0);
    let samples = [
        StrikeFx::Aim { at: to, radius: 20.0, charge: 0.0 },
        StrikeFx::Aim { at: to, radius: 20.0, charge: 0.7 },
        StrikeFx::Aim { at: to, radius: 20.0, charge: 1.0 },
        StrikeFx::IonBeam { from, to, age: 0.0 },
        StrikeFx::IonBeam { from, to, age: 0.5 },
        StrikeFx::IonBeam { from, to, age: 0.95 },
        StrikeFx::Shell { from, to, progress: 0.0 },
        StrikeFx::Shell { from, to, progress: 0.4 },
        StrikeFx::Shell { from, to, progress: 1.0 },
        StrikeFx::Rod { from, to, progress: 0.0 },
        StrikeFx::Rod { from, to, progress: 0.6 },
        StrikeFx::Rod { from, to, progress: 1.0 },
        StrikeFx::Impact { at: to, radius: 15.0, age: 0.0 },
        StrikeFx::Impact { at: to, radius: 15.0, age: 0.3 },
        StrikeFx::Impact { at: to, radius: 15.0, age: 1.0 },
        StrikeFx::Lance { from, to },
    ];

    let bolts = strike_bolts(&samples);
    for (i, bolt) in bolts.iter().enumerate() {
        assert!(bolt.from.is_finite(), "bolt {i} from not finite: {:?}", bolt.from);
        assert!(bolt.to.is_finite(), "bolt {i} to not finite: {:?}", bolt.to);
        assert!(bolt.width.is_finite() && bolt.width >= 0.0, "bolt {i} invalid width: {}", bolt.width);
        assert!(bolt.flash.is_finite() && bolt.flash >= 0.0, "bolt {i} invalid flash: {}", bolt.flash);
        for (c_idx, &c) in bolt.color.iter().enumerate() {
            assert!(c.is_finite(), "bolt {i} color[{c_idx}] not finite: {c}");
        }
        let alpha = bolt.color[3];
        assert!(
            (0.0..=1.0).contains(&alpha),
            "bolt {i} alpha not in 0..=1: {alpha}"
        );
    }
}
