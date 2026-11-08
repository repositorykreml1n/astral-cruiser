#[path = "../mixins/astral_cruiser/lang.rs"]
#[allow(unused)]
mod astral_lang;
#[path = "../mixins/astral_cruiser/arsenal.rs"]
#[allow(unused)]
mod astral_arsenal;
#[path = "../mixins/astral_cruiser/strike_plans.rs"]
#[allow(unused)]
mod astral_strike_plans;
#[path = "../mixins/astral_cruiser/orbit.rs"]
mod astral_orbit;

use astral_arsenal::{WaveSound, Weapon};
use astral_orbit::{spec, Carve, Event, Orbit, Refusal, MAX_ACTIVE, RANGE};
use glam::DVec3;

const T: DVec3 = DVec3::new(100.0, 64.0, 0.0);
const SHIP: DVec3 = DVec3::new(0.0, 346.0, 0.0);

#[test]
fn nonfinite_targets_are_refused_without_spending_energy() {
    for target in [
        DVec3::new(f64::NAN, 64.0, f64::NAN),
        DVec3::new(100.0, f64::NAN, 0.0),
        DVec3::new(f64::INFINITY, 64.0, 0.0),
    ] {
        let mut orbit = Orbit::new(1);
        orbit.select(Weapon::Rod);
        let mut energy = 1000.0;
        assert_eq!(orbit.fire(target, SHIP, &mut energy), Err(Refusal::OutOfRange));
        assert_eq!(energy, 1000.0);
        assert_eq!(orbit.cooldown(Weapon::Rod), 0.0);
        assert!(orbit.active().is_empty());
    }
}

#[test]
fn blast_weapons_clear_above_each_impact() {
    for (weapon, count, radius, height) in [
        (Weapon::Ion, 1, 14.0, 32.0),
        (Weapon::Rod, 1, 24.0, 32.0),
        (Weapon::Barrage, 24, 6.0, 12.0),
    ] {
        let mut o = Orbit::new(1);
        let mut energy = 10000.0;
        o.select(weapon);
        o.fire(T, SHIP, &mut energy).unwrap();
        let events = run(&mut o, 10.0, &mut energy);
        let clears: Vec<_> = events.iter().filter_map(|e| match e {
            Event::Carve(Carve::Clear { center, radius: r, height: h }) => {
                assert_eq!((*r, *h), (radius, height)); Some(*center)
            }
            _ => None,
        }).collect();
        assert_eq!(clears.len(), count, "{weapon:?}");
        if weapon == Weapon::Barrage {
            for e in &events {
                if let Event::Impact { at, .. } = e { assert!(clears.contains(at)); }
            }
        }
    }
}

fn run(o: &mut Orbit, seconds: f64, e: &mut f64) -> Vec<Event> {
    let mut ev = Vec::new();
    let dt = 0.05;
    let count = (seconds / dt).round() as usize;
    for _ in 0..count {
        ev.extend(o.tick(dt, true, T, SHIP, e));
    }
    ev
}

#[test]
fn ion_charges_three_seconds_before_damage() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Ion);
    o.fire(T, SHIP, &mut e).unwrap();
    assert_eq!(e, 40.0);
    let ev = run(&mut o, 2.9, &mut e);
    assert!(!ev.iter().any(|x| matches!(x, Event::Damage { .. })));
    let ev = run(&mut o, 0.2, &mut e);
    assert!(ev.iter().any(|x| matches!(x, Event::Damage { radius, .. } if *radius == 16.0)));
}

#[test]
fn ion_deals_forty_a_second_for_four_seconds() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Ion);
    o.fire(T, SHIP, &mut e).unwrap();
    let ev = run(&mut o, 7.1, &mut e);
    let total_damage: f64 = ev
        .iter()
        .filter_map(|x| match x {
            Event::Damage { amount, .. } => Some(*amount),
            _ => None,
        })
        .sum();
    assert!(
        (total_damage - 160.0).abs() <= 2.0,
        "expected 160 ± 2 damage, got {total_damage}"
    );
}

#[test]
fn ion_crater_deepens_to_sixteen() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Ion);
    o.fire(T, SHIP, &mut e).unwrap();
    let ev = run(&mut o, 7.1, &mut e);
    let depths: Vec<f64> = ev
        .iter()
        .filter_map(|x| match x {
            Event::Carve(Carve::Bowl { depth, .. }) => Some(*depth),
            _ => None,
        })
        .collect();
    assert_eq!(depths, vec![4.0, 8.0, 12.0, 16.0]);
}

#[test]
fn barrage_drops_twenty_four_shells_inside_thirty_two() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Barrage);
    o.fire(T, SHIP, &mut e).unwrap();
    let ev = run(&mut o, 5.6, &mut e);
    let shells: Vec<DVec3> = ev
        .iter()
        .filter_map(|x| match x {
            Event::Shell { to, .. } => Some(*to),
            _ => None,
        })
        .collect();
    assert_eq!(shells.len(), 24);
    for to in shells {
        let dx = to.x - T.x;
        let dz = to.z - T.z;
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(dist <= 32.0 + 1e-6, "shell fell at distance {dist} > 32");
    }
    let impacts = ev
        .iter()
        .filter(|x| matches!(x, Event::Impact { weapon: Weapon::Barrage, .. }))
        .count();
    assert_eq!(impacts, 24);
}

#[test]
fn barrage_is_the_same_for_the_same_seed() {
    let (mut o1, mut e1) = (Orbit::new(9), 100.0);
    o1.select(Weapon::Barrage);
    o1.fire(T, SHIP, &mut e1).unwrap();
    let ev1 = run(&mut o1, 5.6, &mut e1);
    let shells1: Vec<DVec3> = ev1
        .iter()
        .filter_map(|x| match x {
            Event::Shell { to, .. } => Some(*to),
            _ => None,
        })
        .collect();

    let (mut o2, mut e2) = (Orbit::new(9), 100.0);
    o2.select(Weapon::Barrage);
    o2.fire(T, SHIP, &mut e2).unwrap();
    let ev2 = run(&mut o2, 5.6, &mut e2);
    let shells2: Vec<DVec3> = ev2
        .iter()
        .filter_map(|x| match x {
            Event::Shell { to, .. } => Some(*to),
            _ => None,
        })
        .collect();

    assert_eq!(shells1.len(), 24);
    assert_eq!(shells1, shells2);
}

#[test]
fn rod_lands_after_two_and_a_half_seconds() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Rod);
    o.fire(T, SHIP, &mut e).unwrap();
    let ev_before = run(&mut o, 2.45, &mut e);
    assert!(!ev_before.iter().any(|x| matches!(x, Event::Impact { .. })));

    let ev_after = run(&mut o, 0.1, &mut e);
    let impacts: Vec<_> = ev_after
        .iter()
        .filter(|x| matches!(x, Event::Impact { weapon: Weapon::Rod, .. }))
        .collect();
    assert_eq!(impacts.len(), 1);

    assert!(ev_after.iter().any(|x| matches!(
        x,
        Event::Carve(Carve::Bowl { radius, depth, .. })
            if (*radius - 24.0).abs() < 1e-6 && (*depth - 32.0).abs() < 1e-6
    )));
    assert!(ev_after.iter().any(|x| matches!(
        x,
        Event::Damage { radius, amount, falloff, .. }
            if (*radius - 32.0).abs() < 1e-6 && (*amount - 300.0).abs() < 1e-6 && *falloff
    )));
}

#[test]
fn lance_runs_while_held_and_stops_at_eight_seconds() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Lance);
    o.fire(T, SHIP, &mut e).unwrap();
    let ev = run(&mut o, 10.0, &mut e);
    let total_dt: f64 = ev
        .iter()
        .filter_map(|x| match x {
            Event::LanceTick { dt, .. } => Some(*dt),
            _ => None,
        })
        .sum();
    assert!((total_dt - 8.0).abs() <= 0.05, "total dt: {total_dt}");
    let energy_spent = 100.0 - e;
    assert!((energy_spent - 96.0).abs() <= 1.0, "energy spent: {energy_spent}");
}

#[test]
fn lance_stops_when_released() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Lance);
    o.fire(T, SHIP, &mut e).unwrap();
    let _ = run(&mut o, 1.0, &mut e);
    let ev = o.tick(0.05, false, T, SHIP, &mut e);
    assert!(!ev.iter().any(|x| matches!(x, Event::LanceTick { .. })));
    assert!(!o.active().iter().any(|s| s.weapon == Weapon::Lance));
}

#[test]
fn fire_refuses_out_of_range() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    let target = SHIP + DVec3::new(RANGE + 1.0, 0.0, 0.0);
    assert_eq!(o.fire(target, SHIP, &mut e), Err(Refusal::OutOfRange));
}

#[test]
fn fire_refuses_without_energy() {
    let (mut o, mut e) = (Orbit::new(1), 10.0);
    o.select(Weapon::Rod);
    assert_eq!(o.fire(T, SHIP, &mut e), Err(Refusal::NoEnergy));
    assert_eq!(e, 10.0);
}

#[test]
fn fires_while_other_strikes_run() {
    let (mut o, mut e) = (Orbit::new(1), 5000.0);
    assert!(!o.busy());
    o.select(Weapon::Ion);
    assert_eq!(o.selected(), Weapon::Ion);
    o.fire(T, SHIP, &mut e).unwrap();
    assert!(o.busy());
    assert_eq!(o.active().len(), 1);

    o.select(Weapon::Barrage);
    assert_eq!(o.selected(), Weapon::Barrage);
    assert!(o.fire(T, SHIP, &mut e).is_ok());
    assert_eq!(o.active().len(), 2);

    o.select(Weapon::Rod);
    assert_eq!(o.selected(), Weapon::Rod);
    assert!(o.fire(T, SHIP, &mut e).is_ok());
    assert_eq!(o.active().len(), 3);
}

#[test]
fn refuses_past_max_active() {
    let (mut o, mut e) = (Orbit::new(1), 50000.0);
    assert_eq!(MAX_ACTIVE, 12);
    for i in 0..MAX_ACTIVE {
        let w = Weapon::from_index(i).unwrap();
        o.select(w);
        o.fire(T, SHIP, &mut e).unwrap();
    }
    assert_eq!(o.active().len(), MAX_ACTIVE);

    let extra = Weapon::from_index(MAX_ACTIVE).unwrap();
    o.select(extra);
    assert_eq!(o.fire(T, SHIP, &mut e), Err(Refusal::Busy));
    assert_eq!(o.active().len(), MAX_ACTIVE);
}

#[test]
fn second_lance_is_busy() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Lance);
    o.fire(T, SHIP, &mut e).unwrap();
    assert_eq!(o.active().len(), 1);

    assert_eq!(o.fire(T, SHIP, &mut e), Err(Refusal::Busy));
}

#[test]
fn cooldown_per_weapon_independent() {
    let (mut o, mut e) = (Orbit::new(1), 1000.0);
    o.select(Weapon::Ion);
    o.fire(T, SHIP, &mut e).unwrap();
    let ion_cd = spec(Weapon::Ion).cooldown;
    assert!((o.cooldown(Weapon::Ion) - ion_cd).abs() < 1e-6);
    assert_eq!(o.cooldown(Weapon::Barrage), 0.0);
    assert_eq!(o.cooldown(Weapon::Rod), 0.0);

    o.select(Weapon::Barrage);
    o.fire(T, SHIP, &mut e).unwrap();
    let barrage_cd = spec(Weapon::Barrage).cooldown;
    assert!((o.cooldown(Weapon::Barrage) - barrage_cd).abs() < 1e-6);

    let _ = o.tick(0.1, true, T, SHIP, &mut e);
    assert!((o.cooldown(Weapon::Ion) - (ion_cd - 0.1)).abs() < 1e-6);
    assert!((o.cooldown(Weapon::Barrage) - (barrage_cd - 0.1)).abs() < 1e-6);
    assert_eq!(o.cooldown(Weapon::Rod), 0.0);
}

#[test]
fn barrage_shell_from_follows_ship() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Barrage);
    let ship1 = DVec3::new(0.0, 346.0, 0.0);
    o.fire(T, ship1, &mut e).unwrap();

    let ev1 = o.tick(0.05, true, T, ship1, &mut e);
    let s1 = ev1.iter().find_map(|x| match x {
        Event::Shell { from, .. } => Some(*from),
        _ => None,
    }).expect("expected first shell to spawn");
    assert_eq!(s1, ship1);

    let ship2 = DVec3::new(100.0, 346.0, 50.0);
    let mut s2 = None;
    for _ in 0..10 {
        let ev = o.tick(0.05, true, T, ship2, &mut e);
        if let Some(from) = ev.iter().find_map(|x| match x {
            Event::Shell { from, .. } => Some(*from),
            _ => None,
        }) {
            s2 = Some(from);
            break;
        }
    }
    let s2 = s2.expect("expected second shell to spawn");
    assert_eq!(s2, ship2);
    assert_ne!(s1, s2);
}

#[test]
fn cancel_keeps_cooldowns() {
    let (mut o, mut e) = (Orbit::new(1), 5000.0);
    o.select(Weapon::Antimatter);
    o.fire(T, SHIP, &mut e).unwrap();
    let _ = run(&mut o, 1.0, &mut e);
    assert!(!o.active().is_empty());
    let cd_before = o.cooldown(Weapon::Antimatter);
    assert!((cd_before - 9.0).abs() <= 0.1);

    o.cancel();
    assert!(o.active().is_empty());
    let cd_after = o.cooldown(Weapon::Antimatter);
    assert_eq!(cd_before, cd_after);
}

#[test]
fn classic_weapons_emit_their_waves() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Ion);
    o.fire(T, SHIP, &mut e).unwrap();
    let ev = run(&mut o, 3.1, &mut e);
    let ion_wave = ev.iter().find(|x| matches!(x, Event::Wave { sound: WaveSound::Small, radius, .. } if *radius == 100.0));
    assert!(ion_wave.is_some(), "ion should emit its small wave at 3.0s");

    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Barrage);
    o.fire(T, SHIP, &mut e).unwrap();
    let ev = run(&mut o, 5.6, &mut e);
    let barrage_wave = ev.iter().find(|x| matches!(x, Event::Wave { sound: WaveSound::Small, radius, .. } if *radius == 80.0));
    assert!(barrage_wave.is_some(), "barrage should emit its small wave at 5.5s");

    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Rod);
    o.fire(T, SHIP, &mut e).unwrap();
    let ev = run(&mut o, 2.6, &mut e);
    let rod_wave = ev.iter().find(|x| matches!(x, Event::Wave { sound: WaveSound::Medium, radius, .. } if *radius == 150.0));
    assert!(rod_wave.is_some(), "rod should emit its medium wave at 2.5s");
}

#[test]
fn cooling_refusal_names_the_time_left() {
    let (mut o, mut e) = (Orbit::new(1), 5000.0);
    o.select(Weapon::Antimatter);
    o.fire(T, SHIP, &mut e).unwrap();
    let _ = run(&mut o, 2.0, &mut e);
    o.cancel();
    let err = o.fire(T, SHIP, &mut e);
    match err {
        Err(Refusal::Cooling { left }) => {
            assert!((left - 8.0).abs() <= 0.1, "left: {left}");
        }
        other => panic!("expected Err(Refusal::Cooling), got {other:?}"),
    }
}

#[test]
fn active_strike_is_seen_while_it_runs() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    assert!(o.active().is_empty());
    o.select(Weapon::Ion);
    o.fire(T, SHIP, &mut e).unwrap();
    run(&mut o, 1.0, &mut e);
    let active = o.active();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].weapon, Weapon::Ion);
    assert_eq!(active[0].target, Some(T));
    assert!((active[0].elapsed - 1.0).abs() < 1e-6);
    run(&mut o, 7.0, &mut e);
    assert!(o.active().is_empty());
}

#[test]
fn a_lance_has_no_fixed_target() {
    let (mut o, mut e) = (Orbit::new(1), 100.0);
    o.select(Weapon::Lance);
    o.fire(T, SHIP, &mut e).unwrap();
    run(&mut o, 0.5, &mut e);
    assert_eq!(
        o.active().first().map(|s| (s.weapon, s.target)),
        Some((Weapon::Lance, None))
    );
}

#[test]
fn specs_match_definitions() {
    let ion = spec(Weapon::Ion);
    assert_eq!(ion.charge, 3.0);
    assert_eq!(ion.duration, 4.0);
    assert_eq!(ion.cooldown, 0.25);
    assert_eq!(ion.energy, 60.0);
    assert_eq!(ion.reach, 16.0);

    let barrage = spec(Weapon::Barrage);
    assert_eq!(barrage.charge, 0.0);
    assert_eq!(barrage.duration, 4.0);
    assert_eq!(barrage.cooldown, 0.2);
    assert_eq!(barrage.energy, 50.0);
    assert_eq!(barrage.reach, 32.0);

    let rod = spec(Weapon::Rod);
    assert_eq!(rod.charge, 0.0);
    assert_eq!(rod.duration, 2.5);
    assert_eq!(rod.cooldown, 0.45);
    assert_eq!(rod.energy, 80.0);
    assert_eq!(rod.reach, 32.0);

    let lance = spec(Weapon::Lance);
    assert_eq!(lance.charge, 0.0);
    assert_eq!(lance.duration, 8.0);
    assert_eq!(lance.cooldown, 0.15);
    assert_eq!(lance.energy_per_second, 12.0);
    assert_eq!(lance.reach, 3.0);
}

#[test]
fn concurrent_strikes_keep_stable_source_identity() {
    let (mut o, mut e) = (Orbit::new(9), 10000.0);
    o.select(Weapon::Antimatter);
    o.fire(T, SHIP, &mut e).unwrap();
    o.select(Weapon::Rod);
    o.fire(T + DVec3::X * 20.0, SHIP, &mut e).unwrap();
    let events = run(&mut o, 4.0, &mut e);
    let mut source = None;
    let mut sources = std::collections::HashMap::new();
    let mut carves = 0;
    for event in events {
        match event {
            Event::Source { id, weapon, center } => {
                source = Some((id,weapon,center));
                if let Some(previous) = sources.insert(id,weapon) { assert_eq!(previous,weapon); }
            }
            Event::Carve(_) => { assert!(source.is_some()); carves += 1; }
            _ => {}
        }
    }
    assert!(carves >= 2);
    assert_eq!(sources.len(), 2);
}
