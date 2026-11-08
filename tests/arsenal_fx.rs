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
#[allow(unused)]
mod astral_orbit_fx;
#[path = "../mixins/astral_cruiser/arsenal_fx.rs"]
mod astral_arsenal_fx;

use astral_arsenal_fx::{arsenal_bolts, scan_bolts, ArsenalFx, ARSENAL_BOLTS};
use glam::DVec3;

#[test]
fn mushroom_cap_reaches_top() {
    let at = DVec3::new(10.0, 64.0, -20.0);
    let top = 320.0;
    let bolts = arsenal_bolts(&[ArsenalFx::Mushroom { at, top, age: 0.5 }]);
    assert!(!bolts.is_empty());
    let max_y = bolts
        .iter()
        .flat_map(|b| [b.from.y, b.to.y])
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max_y > top - 5.0,
        "mushroom cap max y {max_y} was not > top - 5 ({})",
        top - 5.0
    );
}

#[test]
fn pillar_spans_top_to_bottom() {
    let at = DVec3::new(0.0, 0.0, 0.0);
    let top = 320.0;
    let bottom = -64.0;
    let bolts = arsenal_bolts(&[ArsenalFx::Pillar {
        at,
        top,
        bottom,
        radius: 25.0,
        age: 0.5,
    }]);
    assert!(!bolts.is_empty());
    let (min_y, max_y) = bolts.iter().flat_map(|b| [b.from.y, b.to.y]).fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(min, max), y| (min.min(y), max.max(y)),
    );
    assert!(
        (max_y - top).abs() <= 5.0,
        "max y {max_y} not within 5 of top {top}"
    );
    assert!(
        (min_y - bottom).abs() <= 5.0,
        "min y {min_y} not within 5 of bottom {bottom}"
    );
}

#[test]
fn bolts_capped() {
    let mushrooms: Vec<ArsenalFx> = (0..100)
        .map(|i| ArsenalFx::Mushroom {
            at: DVec3::new(i as f64 * 10.0, 64.0, 0.0),
            top: 320.0,
            age: 0.5,
        })
        .collect();
    let bolts = arsenal_bolts(&mushrooms);
    assert!(bolts.len() <= ARSENAL_BOLTS);
    assert_eq!(bolts.len(), ARSENAL_BOLTS);
}

#[test]
fn wave_ring_radius_matches_front() {
    let at = DVec3::new(15.0, 64.0, -30.0);
    let front = 50.0;
    let bolts = arsenal_bolts(&[ArsenalFx::WaveRing {
        at,
        front,
        strength: 0.8,
        super_: false,
    }]);
    assert!(!bolts.is_empty());
    for bolt in &bolts {
        for pt in [bolt.from, bolt.to] {
            let dist = (pt.x - at.x).hypot(pt.z - at.z);
            assert!(
                dist >= front - 6.0 && dist <= front + 3.0,
                "ring bolt point {pt:?} horizontal distance {dist} not within {front} - 6 .. + 3"
            );
        }
    }
}

#[test]
fn wave_wall_height_matches_strength() {
    let at = DVec3::new(10.0, 64.0, -20.0);
    for &strength in &[0.2_f32, 0.5, 0.8, 1.0, 1.5] {
        let expected = 6.0 + 34.0 * f64::from(strength);
        let bolts = arsenal_bolts(&[ArsenalFx::WaveRing {
            at,
            front: 50.0,
            strength,
            super_: false,
        }]);
        assert!(!bolts.is_empty());
        let max_y = bolts
            .iter()
            .flat_map(|b| [b.from.y, b.to.y])
            .fold(f64::NEG_INFINITY, f64::max);
        let actual = max_y - at.y;
        assert!(
            (actual - expected).abs() <= 2.0,
            "max bolt y - at.y was {actual}, expected within 2 of {expected} for strength {strength}"
        );
    }
}

#[test]
fn wave_wall_budget() {
    let at = DVec3::new(0.0, 64.0, 0.0);
    let bolts = arsenal_bolts(&[ArsenalFx::WaveRing {
        at,
        front: 500.0,
        strength: 1.0,
        super_: true,
    }]);
    assert!(!bolts.is_empty());
    assert!(
        bolts.len() <= 900,
        "full-strength 500-radius wave must fit in <= 900 bolts, got {}",
        bolts.len()
    );
}

#[test]
fn chain_draws_count_segments() {
    let mut points = [DVec3::ZERO; 13];
    points[0] = DVec3::new(0.0, 64.0, 0.0);
    points[1] = DVec3::new(10.0, 64.0, 0.0);
    points[2] = DVec3::new(20.0, 66.0, 5.0);
    points[3] = DVec3::new(30.0, 65.0, -5.0);
    let count = 4;
    let bolts = arsenal_bolts(&[ArsenalFx::Chain {
        points,
        count,
        age: 0.5,
    }]);
    assert!(!bolts.is_empty());

    // Bolts connect at least 3 consecutive point pairs: (0, 1), (1, 2), (2, 3)
    for i in 0..(count as usize - 1) {
        let p_start = points[i];
        let p_end = points[i + 1];
        let has_pair = bolts.iter().any(|b| {
            (b.from == p_start && b.to == p_end)
                || (b.from == p_end && b.to == p_start)
        }) || (
            bolts.iter().any(|b| (b.from - p_start).length() < 1e-4)
                && bolts.iter().any(|b| (b.to - p_end).length() < 1e-4)
        );
        assert!(
            has_pair,
            "bolts did not connect consecutive pair {i} -> {}",
            i + 1
        );
    }
}

#[test]
fn every_variant_draws_something() {
    let at = DVec3::new(0.0, 64.0, 0.0);
    let from = DVec3::new(0.0, 200.0, 0.0);
    let to = DVec3::new(0.0, 64.0, 0.0);
    let mut points = [DVec3::ZERO; 13];
    points[0] = at;
    points[1] = at + DVec3::new(10.0, 0.0, 0.0);

    let variants = [
        ArsenalFx::Mushroom { at, top: 320.0, age: 0.5 },
        ArsenalFx::Pillar { at, top: 320.0, bottom: -64.0, radius: 25.0, age: 0.5 },
        ArsenalFx::Quake { at, length: 60.0, seed: 12345, age: 0.5 },
        ArsenalFx::Meteor { from, to, progress: 0.5 },
        ArsenalFx::Missile { from, to, progress: 0.5 },
        ArsenalFx::Chain { points, count: 2, age: 0.5 },
        ArsenalFx::BlackHole { at, age: 0.5, collapse: 0.0 },
        ArsenalFx::BlackHole { at, age: 0.5, collapse: 0.5 },
        ArsenalFx::Frost { at, radius: 30.0, age: 0.5 },
        ArsenalFx::Wall { from, to, age: 0.5 },
        ArsenalFx::Grid { at, half: 30.0, step: 10.0, age: 0.5 },
        ArsenalFx::Drill { from, to, age: 0.5 },
        ArsenalFx::WaveRing { at, front: 40.0, strength: 0.7, super_: false },
        ArsenalFx::WaveRing { at, front: 40.0, strength: 0.7, super_: true },
        ArsenalFx::Supernova { at, age: 0.5 },
        ArsenalFx::Erasure { at, radius: 120.0, age: 0.6 },
    ];

    for (idx, var) in variants.iter().enumerate() {
        let bolts = arsenal_bolts(&[*var]);
        assert!(
            !bolts.is_empty(),
            "variant index {idx} ({var:?}) returned 0 bolts at mid-age"
        );
    }
}

#[test]
fn typical_frame_under_budget() {
    let at = DVec3::new(0.0, 64.0, 0.0);
    let top = 320.0;
    let bottom = -64.0;
    let mut fx = Vec::new();
    // 1 mushroom
    fx.push(ArsenalFx::Mushroom { at, top, age: 0.5 });
    // 1 pillar
    fx.push(ArsenalFx::Pillar { at, top, bottom, radius: 25.0, age: 0.5 });
    // 3 waves
    fx.push(ArsenalFx::WaveRing { at, front: 50.0, strength: 0.8, super_: false });
    fx.push(ArsenalFx::WaveRing { at, front: 100.0, strength: 0.6, super_: false });
    fx.push(ArsenalFx::WaveRing { at, front: 200.0, strength: 1.0, super_: true });
    // 20 meteors
    for i in 0..20 {
        let from = DVec3::new(i as f64 * 5.0, 300.0, 0.0);
        let to = DVec3::new(i as f64 * 5.0 + 20.0, 64.0, 10.0);
        fx.push(ArsenalFx::Meteor { from, to, progress: 0.3 + (i as f32) * 0.03 });
    }

    let bolts = arsenal_bolts(&fx);
    assert!(
        bolts.len() < ARSENAL_BOLTS,
        "typical frame bolts {} was not strictly under ARSENAL_BOLTS ({})",
        bolts.len(),
        ARSENAL_BOLTS
    );
    assert!(!bolts.is_empty());
}

#[test]
fn all_bolts_have_finite_coordinates_and_valid_alpha() {
    let at = DVec3::new(12.0, 64.0, -18.0);
    let from = DVec3::new(12.0, 240.0, -18.0);
    let to = DVec3::new(14.0, 64.0, -22.0);
    let mut points = [DVec3::ZERO; 13];
    for i in 0..13 {
        points[i] = at + DVec3::new(i as f64 * 3.0, (i as f64 * 0.5).sin() * 2.0, i as f64);
    }

    let samples = [
        ArsenalFx::Mushroom { at, top: 320.0, age: 0.0 },
        ArsenalFx::Mushroom { at, top: 320.0, age: 0.5 },
        ArsenalFx::Mushroom { at, top: 320.0, age: 1.0 },
        ArsenalFx::Pillar { at, top: 320.0, bottom: -64.0, radius: 25.0, age: 0.0 },
        ArsenalFx::Pillar { at, top: 320.0, bottom: -64.0, radius: 25.0, age: 0.5 },
        ArsenalFx::Pillar { at, top: 320.0, bottom: -64.0, radius: 25.0, age: 1.0 },
        ArsenalFx::Quake { at, length: 80.0, seed: 999, age: 0.0 },
        ArsenalFx::Quake { at, length: 80.0, seed: 999, age: 0.5 },
        ArsenalFx::Quake { at, length: 80.0, seed: 999, age: 1.0 },
        ArsenalFx::Meteor { from, to, progress: 0.0 },
        ArsenalFx::Meteor { from, to, progress: 0.5 },
        ArsenalFx::Meteor { from, to, progress: 1.0 },
        ArsenalFx::Missile { from, to, progress: 0.0 },
        ArsenalFx::Missile { from, to, progress: 0.5 },
        ArsenalFx::Missile { from, to, progress: 1.0 },
        ArsenalFx::Chain { points, count: 6, age: 0.0 },
        ArsenalFx::Chain { points, count: 6, age: 0.5 },
        ArsenalFx::Chain { points, count: 6, age: 1.0 },
        ArsenalFx::BlackHole { at, age: 0.0, collapse: 0.0 },
        ArsenalFx::BlackHole { at, age: 0.5, collapse: 0.0 },
        ArsenalFx::BlackHole { at, age: 0.9, collapse: 0.8 },
        ArsenalFx::Frost { at, radius: 40.0, age: 0.0 },
        ArsenalFx::Frost { at, radius: 40.0, age: 0.5 },
        ArsenalFx::Frost { at, radius: 40.0, age: 1.0 },
        ArsenalFx::Wall { from, to, age: 0.0 },
        ArsenalFx::Wall { from, to, age: 0.5 },
        ArsenalFx::Wall { from, to, age: 1.0 },
        ArsenalFx::Grid { at, half: 30.0, step: 10.0, age: 0.0 },
        ArsenalFx::Grid { at, half: 30.0, step: 10.0, age: 0.5 },
        ArsenalFx::Grid { at, half: 30.0, step: 10.0, age: 1.0 },
        ArsenalFx::Drill { from, to, age: 0.0 },
        ArsenalFx::Drill { from, to, age: 0.5 },
        ArsenalFx::Drill { from, to, age: 1.0 },
        ArsenalFx::WaveRing { at, front: 50.0, strength: 0.8, super_: false },
        ArsenalFx::WaveRing { at, front: 50.0, strength: 0.8, super_: true },
        ArsenalFx::Supernova { at, age: 0.0 },
        ArsenalFx::Supernova { at, age: 0.5 },
        ArsenalFx::Supernova { at, age: 1.0 },
        ArsenalFx::Erasure { at, radius: 120.0, age: 0.0 },
        ArsenalFx::Erasure { at, radius: 120.0, age: 0.5 },
        ArsenalFx::Erasure { at, radius: 120.0, age: 1.0 },
    ];

    let bolts = arsenal_bolts(&samples);
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

#[test]
fn super_wave_has_more_bolts_than_normal_wave() {
    let at = DVec3::new(0.0, 64.0, 0.0);
    let normal = arsenal_bolts(&[ArsenalFx::WaveRing {
        at,
        front: 40.0,
        strength: 0.8,
        super_: false,
    }]);
    let super_ = arsenal_bolts(&[ArsenalFx::WaveRing {
        at,
        front: 40.0,
        strength: 0.8,
        super_: true,
    }]);
    assert!(super_.len() > normal.len());
}

#[test]
fn fx_budgets() {
    let at = DVec3::new(0.0, 64.0, 0.0);
    // Pillar budget <= 700 bolts
    let pillar_bolts = arsenal_bolts(&[ArsenalFx::Pillar {
        at,
        top: 320.0,
        bottom: -64.0,
        radius: 25.0,
        age: 0.5,
    }]);
    assert!(
        pillar_bolts.len() <= 700,
        "pillar bolts {} exceeded 700",
        pillar_bolts.len()
    );

    // Supernova budget <= 900 bolts in both phases
    let sn_grow = arsenal_bolts(&[ArsenalFx::Supernova { at, age: 0.3 }]);
    assert!(
        sn_grow.len() <= 900,
        "supernova grow bolts {} exceeded 900",
        sn_grow.len()
    );
    let sn_shell = arsenal_bolts(&[ArsenalFx::Supernova { at, age: 0.8 }]);
    assert!(
        sn_shell.len() <= 900,
        "supernova shell bolts {} exceeded 900",
        sn_shell.len()
    );

    // Erasure budget <= 900 bolts
    let erasure_bolts = arsenal_bolts(&[ArsenalFx::Erasure {
        at,
        radius: 120.0,
        age: 0.7,
    }]);
    assert!(
        erasure_bolts.len() <= 900,
        "erasure bolts {} exceeded 900",
        erasure_bolts.len()
    );

    // Scan bolts budget <= 1200 bolts for radius 120
    let scan_early = scan_bolts(at, 120.0, 0.2, [0.49, 0.97, 1.0], |_, _| Some(64.0));
    assert!(
        scan_early.len() <= 1200,
        "scan bolts early {} exceeded 1200",
        scan_early.len()
    );
    let scan_late = scan_bolts(at, 120.0, 0.9, [0.49, 0.97, 1.0], |_, _| Some(64.0));
    assert!(
        scan_late.len() <= 1200,
        "scan bolts late {} exceeded 1200",
        scan_late.len()
    );
}

#[test]
fn scan_follows_height_function() {
    let at = DVec3::new(10.0, 64.0, -15.0);
    let h_fn = |x: f64, z: f64| Some(64.0 + (x * 0.05).sin() * 5.0 + (z * 0.03).cos() * 3.0);
    let bolts = scan_bolts(at, 120.0, 0.5, [0.49, 0.97, 1.0], h_fn);
    assert!(!bolts.is_empty(), "scan bolts should not be empty");

    for (i, bolt) in bolts.iter().enumerate() {
        let expected_from_y = h_fn(bolt.from.x, bolt.from.z).unwrap();
        assert!(
            (bolt.from.y - expected_from_y).abs() <= 2.0,
            "bolt {i} from.y {} was not within 2 of height {}",
            bolt.from.y,
            expected_from_y
        );
        let expected_to_y = h_fn(bolt.to.x, bolt.to.z).unwrap();
        assert!(
            (bolt.to.y - expected_to_y).abs() <= 2.0,
            "bolt {i} to.y {} was not within 2 of height {}",
            bolt.to.y,
            expected_to_y
        );
    }
}

#[test]
fn supernova_shell_reaches_one_forty_late_in_age() {
    let at = DVec3::new(0.0, 64.0, 0.0);
    // At age 0.95 (late in age), shell should reach >= 140 radius
    let bolts = arsenal_bolts(&[ArsenalFx::Supernova { at, age: 0.95 }]);
    assert!(!bolts.is_empty());
    let max_r = bolts
        .iter()
        .flat_map(|b| [(b.from - at).length(), (b.to - at).length()])
        .fold(0.0_f64, f64::max);
    assert!(
        max_r >= 140.0,
        "supernova shell max radius {max_r} was not >= 140.0"
    );
}
