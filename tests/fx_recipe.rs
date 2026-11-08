extern crate self as horizon_client;

#[path = "../mixins/astral_cruiser/lang.rs"]
pub mod astral_lang;

#[path = "../mixins/astral_cruiser/arsenal.rs"]
pub mod astral_arsenal;

#[path = "../mixins/astral_cruiser/weapons.rs"]
pub mod astral_weapons;

#[path = "../mixins/astral_cruiser/fx_instance.rs"]
pub mod astral_fx_instance;

pub mod gfx {
    pub use crate::astral_fx_instance;
}

#[path = "../mixins/astral_cruiser/fx_budget.rs"]
pub mod astral_fx_budget;

#[path = "../mixins/astral_cruiser/fx_recipe.rs"]
pub mod astral_fx_recipe;

use astral_arsenal::Weapon;
use astral_fx_instance::{FxSpawn, Prim};
use astral_fx_recipe::{Layer, Phase, impact_light, laser_recipe, recipe, spawn};
use astral_weapons::WeaponKind;
use glam::DVec3;

#[test]
fn every_weapon_has_all_four_phases() {
    for w in Weapon::ALL {
        let rec = recipe(w);
        assert!(!rec.is_empty(), "Weapon {:?} recipe must not be empty", w);

        let has_launch = rec.iter().any(|l| l.phase == Phase::Launch);
        let has_flight = rec.iter().any(|l| l.phase == Phase::Flight);
        let has_impact = rec.iter().any(|l| l.phase == Phase::Impact);
        let has_after = rec.iter().any(|l| l.phase == Phase::After);

        assert!(has_launch, "Weapon {:?} missing Launch phase", w);
        assert!(has_flight, "Weapon {:?} missing Flight phase", w);
        assert!(has_impact, "Weapon {:?} missing Impact phase", w);
        assert!(has_after, "Weapon {:?} missing After phase", w);

        let launch_glow = rec
            .iter()
            .any(|l| l.phase == Phase::Launch && l.prim == Prim::Glow);
        let launch_spark = rec
            .iter()
            .any(|l| l.phase == Phase::Launch && l.prim == Prim::Spark);
        assert!(
            launch_glow,
            "Weapon {:?} Launch phase must have a Glow layer",
            w
        );
        assert!(
            launch_spark,
            "Weapon {:?} Launch phase must have a Spark layer",
            w
        );
    }
}

#[test]
fn supers_have_fireballs_and_shell_on_impact() {
    for w in Weapon::ALL {
        if w.is_super() {
            let rec = recipe(w);
            let fireball_layers: Vec<_> = rec
                .iter()
                .filter(|l| l.phase == Phase::Impact && l.prim == Prim::Fireball)
                .collect();
            assert!(
                fireball_layers.len() >= 2,
                "Super weapon {:?} must have >= 2 Fireball layers in Impact",
                w
            );

            let total_fireballs: u32 = fireball_layers.iter().map(|l| l.count).sum();
            assert!(
                total_fireballs >= 2,
                "Super weapon {:?} Fireballs total count must be >= 2 in Impact",
                w
            );

            // Solar's shaft is only 25 blocks in radius; its core peaks at 15.
            let min_size = if w == Weapon::SolarPillar { 12.0 } else { 30.0 };
            let has_large_fireball = fireball_layers
                .iter()
                .any(|l| l.size >= min_size && l.size <= 120.0);
            assert!(
                has_large_fireball,
                "Super weapon {:?} must have a crater-sized Impact Fireball",
                w
            );

            let shell_layers: Vec<_> = rec
                .iter()
                .filter(|l| l.phase == Phase::Impact && l.prim == Prim::Shell)
                .collect();
            assert!(
                !shell_layers.is_empty(),
                "Super weapon {:?} must have >= 1 Shell layer in Impact",
                w
            );

            let has_fast_shell = shell_layers
                .iter()
                .any(|l| l.grow >= 60.0 && l.grow <= 200.0);
            assert!(
                has_fast_shell,
                "Super weapon {:?} must have Shell grow in 60..=200 blocks/s",
                w
            );
        }
    }
}

#[test]
fn after_phase_smoke_lasts_8_to_20_s() {
    for w in Weapon::ALL {
        let rec = recipe(w);
        let valid_smoke = rec.iter().any(|l| {
            l.phase == Phase::After && l.prim == Prim::Smoke && l.life >= 8.0 && l.life <= 20.0
        });
        assert!(
            valid_smoke,
            "Weapon {:?} must have an After Smoke layer with life in 8..=20 s",
            w
        );
    }
}

#[test]
fn full_recipe_fits_cap() {
    for w in Weapon::ALL {
        let rec = recipe(w);
        let total_count: u32 = rec.iter().map(|l| l.count).sum();
        let budget = astral_fx_budget::cap(w);
        assert!(
            total_count <= budget,
            "Weapon {:?} total count {} exceeds cap {}",
            w,
            total_count,
            budget
        );
    }
}

#[test]
fn share_half_halves_particles_and_zero_keeps_only_far_prims() {
    let layers = [
        Layer {
            phase: Phase::Impact,
            prim: Prim::Spark,
            color: [1.0, 1.0, 1.0, 1.0],
            count: 100,
            size: 1.0,
            spread: 5.0,
            speed: 10.0,
            life: 1.0,
            delay: 0.0,
            grow: 0.0,
        },
        Layer {
            phase: Phase::Impact,
            prim: Prim::Smoke,
            color: [0.5, 0.5, 0.5, 0.5],
            count: 40,
            size: 2.0,
            spread: 5.0,
            speed: 2.0,
            life: 2.0,
            delay: 0.0,
            grow: 1.0,
        },
        Layer {
            phase: Phase::Impact,
            prim: Prim::Fireball,
            color: [1.0, 0.5, 0.0, 1.0],
            count: 2,
            size: 10.0,
            spread: 0.0,
            speed: 0.0,
            life: 3.0,
            delay: 0.0,
            grow: 5.0,
        },
        Layer {
            phase: Phase::Impact,
            prim: Prim::Shell,
            color: [0.8, 0.8, 1.0, 0.8],
            count: 1,
            size: 5.0,
            spread: 0.0,
            speed: 0.0,
            life: 2.0,
            delay: 0.0,
            grow: 20.0,
        },
    ];

    let mut out_full = Vec::new();
    spawn(
        &layers,
        Phase::Impact,
        DVec3::ZERO,
        DVec3::Y,
        1.0,
        0.0,
        1.0,
        1234,
        &mut out_full,
    );

    let mut out_half = Vec::new();
    spawn(
        &layers,
        Phase::Impact,
        DVec3::ZERO,
        DVec3::Y,
        1.0,
        0.0,
        0.5,
        1234,
        &mut out_half,
    );

    let mut out_zero = Vec::new();
    spawn(
        &layers,
        Phase::Impact,
        DVec3::ZERO,
        DVec3::Y,
        1.0,
        0.0,
        0.0,
        1234,
        &mut out_zero,
    );

    let count_prim = |out: &[FxSpawn], prim: Prim| out.iter().filter(|s| s.prim == prim).count();

    assert_eq!(count_prim(&out_full, Prim::Spark), 100);
    assert_eq!(count_prim(&out_full, Prim::Smoke), 40);
    assert_eq!(count_prim(&out_full, Prim::Fireball), 2);
    assert_eq!(count_prim(&out_full, Prim::Shell), 1);

    // Share 0.5 halves small particles:
    assert_eq!(count_prim(&out_half, Prim::Spark), 50);
    assert_eq!(count_prim(&out_half, Prim::Smoke), 20);
    assert_eq!(count_prim(&out_half, Prim::Fireball), 2);
    assert_eq!(count_prim(&out_half, Prim::Shell), 1);

    // Share 0.0 keeps only far prims:
    assert_eq!(count_prim(&out_zero, Prim::Spark), 0);
    assert_eq!(count_prim(&out_zero, Prim::Smoke), 0);
    assert_eq!(count_prim(&out_zero, Prim::Fireball), 2);
    assert_eq!(count_prim(&out_zero, Prim::Shell), 1);
    assert_eq!(out_zero.len(), 3);
}

#[test]
fn same_seed_same_spawns() {
    let rec = recipe(Weapon::Antimatter);
    let mut out1 = Vec::new();
    let mut out2 = Vec::new();
    let mut out3 = Vec::new();

    spawn(
        rec,
        Phase::Impact,
        DVec3::new(10.0, 20.0, 30.0),
        DVec3::Y,
        1.0,
        5.0,
        1.0,
        9999,
        &mut out1,
    );
    spawn(
        rec,
        Phase::Impact,
        DVec3::new(10.0, 20.0, 30.0),
        DVec3::Y,
        1.0,
        5.0,
        1.0,
        9999,
        &mut out2,
    );
    spawn(
        rec,
        Phase::Impact,
        DVec3::new(10.0, 20.0, 30.0),
        DVec3::Y,
        1.0,
        5.0,
        1.0,
        8888,
        &mut out3,
    );

    assert_eq!(out1.len(), out2.len());
    assert_eq!(out1, out2);
    assert_ne!(out1, out3);
}

#[test]
fn antimatter_core_is_violet_and_edge_dark() {
    let rec = recipe(Weapon::Antimatter);

    let mut found_violet_core = false;
    for layer in rec {
        if layer.phase == Phase::Impact && layer.prim == Prim::Fireball {
            let [r, g, b, _] = layer.color;
            if (r - 0.75).abs() <= 0.05 && (g - 0.35).abs() <= 0.05 && (b - 1.0).abs() <= 0.05 {
                found_violet_core = true;
                break;
            }
        }
    }
    assert!(
        found_violet_core,
        "Antimatter Impact must have a violet-ish Fireball [0.75, 0.35, 1.0] (within 0.05)"
    );

    let mut found_dark_smoke = false;
    for layer in rec {
        if layer.prim == Prim::Smoke {
            let [r, g, b, a] = layer.color;
            if r < 0.25 && g < 0.25 && b < 0.25 && a >= 0.6 {
                found_dark_smoke = true;
                break;
            }
        }
    }
    assert!(
        found_dark_smoke,
        "Antimatter must have a dark-violet Smoke layer (rgb each < 0.25 and alpha >= 0.6)"
    );
}

#[test]
fn every_laser_kind_has_launch_and_impact() {
    for k in WeaponKind::ALL {
        let rec = laser_recipe(k);
        assert!(!rec.is_empty(), "Laser {:?} recipe must not be empty", k);

        let has_launch = rec.iter().any(|l| l.phase == Phase::Launch);
        let has_impact = rec.iter().any(|l| l.phase == Phase::Impact);
        assert!(has_launch, "Laser {:?} missing Launch layer", k);
        assert!(has_impact, "Laser {:?} missing Impact layer", k);

        let total_count: u32 = rec.iter().map(|l| l.count).sum();
        assert!(
            total_count <= astral_fx_budget::LASER_CAP,
            "Laser {:?} total count {} exceeds LASER_CAP {}",
            k,
            total_count,
            astral_fx_budget::LASER_CAP
        );
    }
}

#[test]
fn impact_light_bounds_and_radius() {
    for w in Weapon::ALL {
        let (rgb, radius) = impact_light(w);
        assert!(rgb[0] >= 0.0 && rgb[0] <= 1.0);
        assert!(rgb[1] >= 0.0 && rgb[1] <= 1.0);
        assert!(rgb[2] >= 0.0 && rgb[2] <= 1.0);
        if w.is_super() {
            assert!(
                radius >= 400.0 && radius <= 900.0,
                "Super weapon {:?} radius {} must be 400..=900",
                w,
                radius
            );
        } else {
            assert!(
                radius >= 40.0 && radius <= 200.0,
                "Weapon {:?} radius {} must be 40..=200",
                w,
                radius
            );
        }
    }
}

#[test]
fn impact_fireballs_stay_near_sixty_percent_of_the_crater() {
    // Actual excavation footprints, not the much larger shockwave/light radii.
    for (weapon, crater) in [
        (Weapon::Antimatter, 100.0_f32),
        (Weapon::Supernova, 90.0),
        (Weapon::Erasure, 120.0),
        (Weapon::SolarPillar, 25.0),
        (Weapon::Rod, 24.0),
    ] {
        for strength in [1.0, 1.5] {
            let mut instances = Vec::new();
            spawn(recipe(weapon), Phase::Impact, DVec3::ZERO, DVec3::Y,
                strength, 0.0, 1.0, 42, &mut instances);
            // At the end of life ease_out is one and growth is capped at 1.5s.
            let peak = instances.iter().filter(|s| s.prim == Prim::Fireball)
                .map(|s| s.size + s.grow * s.life.min(1.5)).fold(0.0_f32, f32::max);
            assert!((crater * 0.5..=crater * 0.65).contains(&peak),
                "{weapon:?} at strength {strength}: peak radius {peak}, crater {crater}");
        }
    }
}

#[test]
fn impact_glows_are_short_flashes() {
    for weapon in Weapon::ALL {
        for layer in recipe(weapon).iter().filter(|l|
            l.phase == Phase::Impact && l.prim == Prim::Glow)
        {
            assert!(layer.life <= 0.6, "{weapon:?} flash lasts {}s", layer.life);
        }
    }
}

#[test]
fn distant_spawn_points_keep_only_fireballs_shells_and_beams() {
    use astral_fx_budget::within_spawn_range;
    let camera = DVec3::new(10000.0, 70.0, -10000.0);
    for prim in [Prim::Spark, Prim::Smoke, Prim::Glow] {
        assert!(within_spawn_range(prim, camera + DVec3::X * 1500.0, camera));
        assert!(!within_spawn_range(prim, camera + DVec3::X * 1500.01, camera));
        assert!(!within_spawn_range(prim, camera + DVec3::Y * 1500.01, camera));
    }
    for prim in [Prim::Fireball, Prim::Shell, Prim::Beam] {
        assert!(within_spawn_range(prim, camera + DVec3::X * 100_000.0, camera));
    }
}
