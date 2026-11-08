#[path = "../mixins/astral_cruiser/lang.rs"]
#[allow(unused)]
mod astral_lang;
#[path = "../mixins/astral_cruiser/arsenal.rs"]
#[allow(unused)]
mod astral_arsenal;
#[path = "../mixins/astral_cruiser/orbit.rs"]
#[allow(unused)]
mod astral_orbit;
#[path = "../mixins/astral_cruiser/strike_plans.rs"]
mod astral_strike_plans;

use astral_arsenal::{spec, WaveSound, Weapon};
use astral_orbit::{Carve, Event, Shown};
use astral_strike_plans::plan;
use glam::DVec3;

const T: DVec3 = DVec3::new(100.0, 64.0, 0.0);
const SHIP: DVec3 = DVec3::new(0.0, 346.0, 0.0);

#[test]
fn planned_blasts_clear_at_the_carve_time_and_position() {
    for (weapon, count, height) in [
        (Weapon::Antimatter, 1, 64.0), (Weapon::Supernova, 1, 64.0),
        (Weapon::BlackHole, 1, 32.0), (Weapon::Meteors, 40, 12.0),
        (Weapon::Cluster, 60, 12.0),
    ] {
        let p = plan(weapon, T, SHIP, 1);
        let mut found = 0;
        for t in &p {
            if let Event::Carve(Carve::Clear { center, radius, height: h }) = t.event {
                found += 1;
                assert_eq!(h, height);
                assert!(p.iter().any(|other| other.at == t.at && match other.event {
                    Event::Carve(Carve::Sphere { center: c, radius: r }) |
                    Event::Carve(Carve::FlatBowl { center: c, radius: r, .. }) => c == center && r == radius,
                    _ => false,
                }));
                if weapon == Weapon::BlackHole { assert_eq!(t.at, 4.0); }
            }
        }
        assert_eq!(found, count, "{weapon:?}");
    }
    for w in [Weapon::Drill, Weapon::SolarPillar, Weapon::Lance, Weapon::Grid, Weapon::PlasmaWall, Weapon::Erasure] {
        assert!(!plan(w, T, SHIP, 1).iter().any(|t| matches!(t.event, Event::Carve(Carve::Clear { .. }))));
    }
}

#[test]
fn classic_weapons_return_empty_plan() {
    for w in [Weapon::Ion, Weapon::Barrage, Weapon::Rod, Weapon::Lance] {
        let p = plan(w, T, SHIP, 1);
        assert!(p.is_empty(), "classic weapon {:?} should have empty plan", w);
    }
}

#[test]
fn antimatter_plan_has_three_waves_and_flat_bowl() {
    let p = plan(Weapon::Antimatter, T, SHIP, 42);
    let waves: Vec<_> = p
        .iter()
        .filter_map(|t| match t.event {
            Event::Wave {
                center,
                radius,
                strength,
                damaging,
                sound,
            } => Some((t.at, center, radius, strength, damaging, sound)),
            _ => None,
        })
        .collect();
    assert_eq!(waves.len(), 3, "expected 3 waves for Antimatter");
    assert!((waves[0].0 - 1.5).abs() < 1e-6);
    assert!((waves[1].0 - 2.7).abs() < 1e-6);
    assert!((waves[2].0 - 4.1).abs() < 1e-6);
    for w in &waves {
        assert_eq!(w.1, T);
        assert_eq!(w.2, 500.0);
        assert!(w.4); // damaging
        assert_eq!(w.5, WaveSound::Super);
    }

    let flat_bowls: Vec<_> = p
        .iter()
        .filter_map(|t| match t.event {
            Event::Carve(Carve::FlatBowl { center, radius, depth }) => {
                Some((t.at, center, radius, depth))
            }
            _ => None,
        })
        .collect();
    assert_eq!(flat_bowls.len(), 1, "expected 1 flat bowl for Antimatter");
    assert!((flat_bowls[0].0 - 1.5).abs() < 1e-6);
    assert_eq!(flat_bowls[0].1, T);
    assert_eq!(flat_bowls[0].2, 100.0);
    assert_eq!(flat_bowls[0].3, 40.0);

    let has_mushroom = p.iter().any(|t| {
        t.at == 0.0
            && matches!(
                t.event,
                Event::Show {
                    fx: Shown::Mushroom,
                    at,
                    seconds,
                } if at == T && seconds == 16.0
            )
    });
    assert!(has_mushroom, "expected Mushroom show event at 0");
}

#[test]
fn meteors_plan_has_forty_shells_within_60() {
    let p = plan(Weapon::Meteors, T, SHIP, 12345);
    let shells: Vec<_> = p
        .iter()
        .filter_map(|t| match t.event {
            Event::Shell { from, to, seconds, weapon } => {
                Some((t.at, from, to, seconds, weapon))
            }
            _ => None,
        })
        .collect();
    assert_eq!(shells.len(), 40, "expected 40 meteor shells");

    for (at, from, to, seconds, weapon) in shells {
        assert_eq!(weapon, Weapon::Meteors);
        assert_eq!(seconds, 1.2);
        // Altitude 300 above target
        assert!((from.y - (T.y + 300.0)).abs() < 1e-6);
        let h_offset = ((from.x - T.x).powi(2) + (from.z - T.z).powi(2)).sqrt();
        assert!(
            (h_offset - 80.0).abs() < 1e-6,
            "expected horizontal offset 80 from target, got {h_offset}"
        );
        let dist = ((to.x - T.x).powi(2) + (to.z - T.z).powi(2)).sqrt();
        assert!(dist <= 60.0 + 1e-6, "shell landed at {dist} > 60");

        // Verify impact events follow at at + 1.2
        let impact_time = at + 1.2;
        let has_sphere = p.iter().any(|t| {
            (t.at - impact_time).abs() < 1e-6
                && matches!(t.event, Event::Carve(Carve::Sphere { center, radius }) if center == to && radius == 5.0)
        });
        assert!(has_sphere, "missing sphere carve for meteor at {impact_time}");
        let has_impact = p.iter().any(|t| {
            (t.at - impact_time).abs() < 1e-6
                && matches!(t.event, Event::Impact { at: impact_at, weapon: Weapon::Meteors, radius } if impact_at == to && radius == 5.0)
        });
        assert!(has_impact, "missing impact event for meteor at {impact_time}");
    }

    let final_wave = p.iter().find(|t| {
        (t.at - 6.0).abs() < 1e-6
            && matches!(t.event, Event::Wave { sound: WaveSound::Medium, .. })
    });
    assert!(final_wave.is_some(), "missing final medium wave at 6.0");
}

#[test]
fn plans_sorted_by_time() {
    for &w in &Weapon::ALL[4..] {
        let p = plan(w, T, SHIP, 999);
        for window in p.windows(2) {
            assert!(
                window[0].at <= window[1].at + 1e-9,
                "{:?} not sorted by time: {} > {}",
                w,
                window[0].at,
                window[1].at
            );
        }
    }
}

#[test]
fn every_new_weapon_plan_is_non_empty_and_finishes_within_spec_duration() {
    for &w in &Weapon::ALL[4..] {
        let p = plan(w, T, SHIP, 42);
        assert!(!p.is_empty(), "{:?} plan is empty", w);
        let last_at = p.last().unwrap().at;
        let margin = match w {
            Weapon::Meteors => 1.2 + 0.1,
            Weapon::Cluster => 0.8 + 0.1,
            _ => 0.1,
        };
        let limit = spec(w).duration + margin;
        assert!(
            last_at <= limit,
            "{:?} plan finished at {last_at}, exceeding limit {limit}",
            w
        );
    }
}

#[test]
fn cluster_plan_has_sixty_shells_and_wave() {
    let p = plan(Weapon::Cluster, T, SHIP, 77);
    let shells: Vec<_> = p
        .iter()
        .filter_map(|t| match t.event {
            Event::Shell { from, to, seconds, weapon } => {
                Some((t.at, from, to, seconds, weapon))
            }
            _ => None,
        })
        .collect();
    assert_eq!(shells.len(), 60);
    for (_, from, to, seconds, weapon) in shells {
        assert_eq!(from, SHIP);
        assert_eq!(seconds, 0.8);
        assert_eq!(weapon, Weapon::Cluster);
        let dist = ((to.x - T.x).powi(2) + (to.z - T.z).powi(2)).sqrt();
        assert!(dist <= 40.0 + 1e-6, "cluster shell landed at {dist} > 40");
    }
    let wave = p.iter().find(|t| (t.at - 1.5).abs() < 1e-6 && matches!(t.event, Event::Wave { .. }));
    assert!(wave.is_some(), "expected cluster wave at 1.5");
}

#[test]
fn solar_pillar_and_tectonic_plans() {
    let sp = plan(Weapon::SolarPillar, T, SHIP, 10);
    assert!(sp.iter().any(|t| t.at == 0.0 && matches!(t.event, Event::Show { fx: Shown::Pillar, .. })));
    assert!(sp.iter().any(|t| (t.at - 1.0).abs() < 1e-6 && matches!(t.event, Event::Carve(Carve::Shaft { radius, .. }) if radius == 25.0)));
    assert!(sp.iter().any(|t| (t.at - 1.0).abs() < 1e-6 && matches!(t.event, Event::Carve(Carve::Rim { radius, thickness, .. }) if radius == 25.0 && thickness == 3.0)));

    let tec = plan(Weapon::Tectonic, T, SHIP, 20);
    assert!(tec.iter().any(|t| t.at == 0.0 && matches!(t.event, Event::Show { fx: Shown::Quake, .. })));
    assert!(tec.iter().any(|t| (t.at - 0.5).abs() < 1e-6 && matches!(t.event, Event::Carve(Carve::Fissures { count: 10, .. }))));
    assert!(tec.iter().any(|t| (t.at - 0.5).abs() < 1e-6 && matches!(t.event, Event::Carve(Carve::Uplift { .. }))));
}

#[test]
fn plasma_wall_is_perpendicular_to_ship_direction() {
    // Ship at (0, 346, 0), Target at (100, 64, 0): dx = 100, dz = 0 -> direction along +x.
    // Perpendicular should be along z!
    let p = plan(Weapon::PlasmaWall, T, SHIP, 33);
    let trench = p.iter().find_map(|t| match t.event {
        Event::Carve(Carve::Trench { from, to, width, depth }) => Some((from, to, width, depth)),
        _ => None,
    }).expect("expected trench in plasma wall");
    let (from, to, width, depth) = trench;
    assert_eq!(width, 4.0);
    assert_eq!(depth, 6.0);
    assert!((from.x - T.x).abs() < 1e-6);
    assert!((to.x - T.x).abs() < 1e-6);
    let length = (from.z - to.z).abs();
    assert!((length - 120.0).abs() < 1e-6, "expected trench length 120, got {length}");
}

#[test]
fn grid_plan_has_twenty_five_intersections() {
    let p = plan(Weapon::Grid, T, SHIP, 55);
    let damages: Vec<_> = p.iter().filter_map(|t| match t.event {
        Event::Damage { at, radius, amount, .. } => Some((at, radius, amount)),
        _ => None,
    }).collect();
    assert_eq!(damages.len(), 25);
    for (at, radius, amount) in damages {
        assert_eq!(radius, 3.0);
        assert_eq!(amount, 40.0);
        let dx = (at.x - T.x).abs();
        let dz = (at.z - T.z).abs();
        assert!(dx <= 24.0 + 1e-6 && dz <= 24.0 + 1e-6);
    }
}

#[test]
fn erasure_and_supernova_plans_start_with_scan_and_finale_at_ten() {
    let ep = plan(Weapon::Erasure, T, SHIP, 42);
    // Both plans start with Scan at 0 for 10 s
    assert!(ep.iter().any(|t| t.at == 0.0 && matches!(t.event, Event::Show { fx: Shown::Scan, at, seconds } if at == T && seconds == 10.0)));
    assert!(ep.iter().any(|t| t.at == 0.0 && matches!(t.event, Event::Show { fx: Shown::Erasure, at, seconds } if at == T && seconds == 22.0)));
    // Finale exactly at 10.0
    assert!(ep.iter().any(|t| (t.at - 10.0).abs() < 1e-6 && matches!(t.event, Event::Carve(Carve::Erase { center, radius, plane }) if center == T && radius == 120.0 && (plane - (T.y - 20.0)).abs() < 1e-6)));
    assert!(ep.iter().any(|t| (t.at - 10.0).abs() < 1e-6 && matches!(t.event, Event::Damage { at, radius, amount, falloff } if at == T && radius == 120.0 && amount == 200.0 && !falloff)));
    assert!(ep.iter().any(|t| (t.at - 10.0).abs() < 1e-6 && matches!(t.event, Event::Wave { center, radius, sound: WaveSound::Medium, damaging: false, .. } if center == T && radius == 200.0)));

    let sp = plan(Weapon::Supernova, T, SHIP, 42);
    // Starts with Scan at 0 for 10 s
    assert!(sp.iter().any(|t| t.at == 0.0 && matches!(t.event, Event::Show { fx: Shown::Scan, at, seconds } if at == T && seconds == 10.0)));
    let sn_pos = T + DVec3::new(0.0, 60.0, 0.0);
    assert!(sp.iter().any(|t| t.at == 0.0 && matches!(t.event, Event::Show { fx: Shown::Supernova, at, seconds } if at == sn_pos && seconds == 24.0)));
    // Finale at 10.0
    assert!(sp.iter().any(|t| (t.at - 10.0).abs() < 1e-6 && matches!(t.event, Event::Impact { at, weapon: Weapon::Supernova, radius } if at == T && radius == 90.0)));
    assert!(sp.iter().any(|t| (t.at - 10.0).abs() < 1e-6 && matches!(t.event, Event::Carve(Carve::FlatBowl { center, radius, depth }) if center == T && radius == 90.0 && depth == 50.0)));
    assert!(sp.iter().any(|t| (t.at - 10.0).abs() < 1e-6 && matches!(t.event, Event::Damage { at, radius, amount, falloff } if at == T && radius == 120.0 && amount == 500.0 && falloff)));
    assert!(sp.iter().any(|t| (t.at - 10.0).abs() < 1e-6 && matches!(t.event, Event::Ignite { at, radius } if at == T && radius == 140.0)));

    // Supernova has 3 damaging waves at 10.0, 11.0, 12.4
    let sn_waves: Vec<_> = sp.iter().filter_map(|t| match t.event {
        Event::Wave { center, radius, strength, damaging, sound } => Some((t.at, center, radius, strength, damaging, sound)),
        _ => None,
    }).collect();
    assert_eq!(sn_waves.len(), 3);
    assert!((sn_waves[0].0 - 10.0).abs() < 1e-6 && sn_waves[0].4 && (sn_waves[0].3 - 1.0).abs() < 1e-6);
    assert!((sn_waves[1].0 - 11.0).abs() < 1e-6 && sn_waves[1].4 && (sn_waves[1].3 - 0.7).abs() < 1e-6);
    assert!((sn_waves[2].0 - 12.4).abs() < 1e-6 && sn_waves[2].4 && (sn_waves[2].3 - 0.4).abs() < 1e-6);
    for w in &sn_waves {
        assert_eq!(w.1, T);
        assert_eq!(w.2, 500.0);
        assert_eq!(w.5, WaveSound::Super);
    }
}
