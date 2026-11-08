#[allow(dead_code, unused_imports)]
#[path = "../mixins/astral_cruiser/lang.rs"]
mod astral_lang;
#[path = "../mixins/astral_cruiser/arsenal.rs"]
mod astral_arsenal;

use astral_arsenal::{
    display_blurb, display_name, spec, Group, WaveSound, Weapon, COUNT, MIN_COOLDOWN, SUPER_COOLDOWN,
};
use astral_lang::Lang;
use std::collections::HashSet;

#[test]
fn table_has_eighteen_unique_names() {
    assert_eq!(COUNT, 18);
    assert_eq!(Weapon::ALL.len(), 18);
    let mut names = HashSet::new();
    let mut names_en = HashSet::new();
    for &w in &Weapon::ALL {
        let s = spec(w);
        assert!(!s.name.is_empty(), "weapon {:?} has empty name", w);
        assert!(
            names.insert(s.name),
            "duplicate weapon name: {}",
            s.name
        );
        assert!(!s.name_en.is_empty(), "weapon {:?} has empty name_en", w);
        assert!(
            names_en.insert(s.name_en),
            "duplicate weapon name_en: {}",
            s.name_en
        );
        assert!(!s.blurb.is_empty(), "weapon {:?} has empty blurb", w);
        let char_count = s.blurb.chars().count();
        assert!(
            char_count <= 60,
            "blurb for {:?} too long ({} chars): {}",
            w,
            char_count,
            s.blurb
        );
        assert!(!s.blurb_en.is_empty(), "weapon {:?} has empty blurb_en", w);
        let char_count_en = s.blurb_en.chars().count();
        assert!(
            char_count_en <= 60,
            "blurb_en for {:?} too long ({} chars): {}",
            w,
            char_count_en,
            s.blurb_en
        );
        assert_eq!(display_name(w, Lang::Ru), s.name);
        assert_eq!(display_name(w, Lang::En), s.name_en);
        assert_eq!(display_blurb(w, Lang::Ru), s.blurb);
        assert_eq!(display_blurb(w, Lang::En), s.blurb_en);
    }
    assert_eq!(names.len(), 18);
    assert_eq!(names_en.len(), 18);
}

#[test]
fn supers_have_500_reach_and_10s_cooldown() {
    let supers = [Weapon::Antimatter, Weapon::SolarPillar, Weapon::Tectonic];
    for &w in &supers {
        let s = spec(w);
        assert!(w.is_super(), "{:?} should be super", w);
        assert_eq!(s.group, Group::Super);
        assert_eq!(s.reach, 500.0, "{:?} reach should be 500.0", w);
        assert_eq!(s.cooldown, 10.0, "{:?} cooldown should be 10.0", w);
        assert_eq!(s.cooldown, SUPER_COOLDOWN);
        assert!(
            s.waves.iter().any(|wave| wave.damaging),
            "{:?} must have at least one damaging wave",
            w
        );
        for wave in s.waves {
            assert_eq!(wave.radius, 500.0);
            assert!(wave.damaging);
            assert_eq!(wave.sound, WaveSound::Super);
        }
    }
}

#[test]
fn antimatter_has_three_waves() {
    let s = spec(Weapon::Antimatter);
    assert_eq!(s.waves.len(), 3);
    let expected_rel_delays = [0.0, 1.2, 2.6];
    let expected_strengths = [1.0, 0.6, 0.35];
    for (i, w) in s.waves.iter().enumerate() {
        let rel_delay = w.delay - s.charge;
        assert!(
            (rel_delay - expected_rel_delays[i]).abs() < 1e-6,
            "wave {} relative delay expected {}, got {}",
            i,
            expected_rel_delays[i],
            rel_delay
        );
        assert!(
            (w.strength - expected_strengths[i]).abs() < 1e-6,
            "wave {} strength expected {}, got {}",
            i,
            expected_strengths[i],
            w.strength
        );
        assert_eq!(w.radius, 500.0);
        assert!(w.damaging);
        assert_eq!(w.sound, WaveSound::Super);
    }
}

#[test]
fn tectonic_has_five_waves() {
    let s = spec(Weapon::Tectonic);
    assert_eq!(s.waves.len(), 5);
    let expected_strengths = [0.8, 0.65, 0.5, 0.35, 0.2];
    for (i, w) in s.waves.iter().enumerate() {
        let rel_delay = w.delay - s.charge;
        let expected_rel = i as f64 * 0.8;
        assert!(
            (rel_delay - expected_rel).abs() < 1e-6,
            "wave {} relative delay expected {}, got {}",
            i,
            expected_rel,
            rel_delay
        );
        assert!(
            (w.strength - expected_strengths[i]).abs() < 1e-6,
            "wave {} strength expected {}, got {}",
            i,
            expected_strengths[i],
            w.strength
        );
        assert_eq!(w.radius, 500.0);
        assert!(w.damaging);
        assert_eq!(w.sound, WaveSound::Super);
    }
}

#[test]
fn only_supers_damage_with_waves() {
    for &w in &Weapon::ALL {
        let s = spec(w);
        if s.waves.iter().any(|wave| wave.damaging) {
            assert!(
                w.is_super(),
                "{:?} has damaging waves so must be super",
                w
            );
        } else if !w.is_super() {
            assert!(
                s.waves.iter().all(|wave| !wave.damaging),
                "{:?} non-super weapon must not have damaging waves",
                w
            );
        }
    }
}

#[test]
fn cooldowns_never_below_tick() {
    for &w in &Weapon::ALL {
        let s = spec(w);
        assert!(
            s.cooldown >= MIN_COOLDOWN,
            "{:?} cooldown {} is below MIN_COOLDOWN {}",
            w,
            s.cooldown,
            MIN_COOLDOWN
        );
        assert!(
            s.cooldown >= 0.05,
            "{:?} cooldown {} is below 0.05 s tick",
            w,
            s.cooldown
        );
    }
}

#[test]
fn stepped_wraps_all_eighteen() {
    assert_eq!(Weapon::Supernova.stepped(1), Weapon::Ion);
    assert_eq!(Weapon::Ion.stepped(-1), Weapon::Supernova);

    for (i, &w) in Weapon::ALL.iter().enumerate() {
        assert_eq!(w.stepped(0), w);
        assert_eq!(w.stepped(18), w);
        assert_eq!(w.stepped(-18), w);
        assert_eq!(w.stepped(36), w);
        assert_eq!(w.stepped(1), Weapon::ALL[(i + 1) % COUNT]);
        assert_eq!(w.stepped(-1), Weapon::ALL[(i + COUNT - 1) % COUNT]);
    }
}

#[test]
fn erasure_and_supernova_specs() {
    let erasure = spec(Weapon::Erasure);
    assert_eq!(erasure.name, "Стирание");
    assert_eq!(erasure.group, Group::Super);
    assert_eq!(erasure.color, 0x7CF7FF);
    assert_eq!(erasure.energy, 4000.0);
    assert_eq!(erasure.cooldown, SUPER_COOLDOWN);
    assert_eq!(erasure.charge, 10.0);
    assert_eq!(erasure.duration, 22.0);
    assert_eq!(erasure.reach, 120.0);
    assert_eq!(erasure.waves.len(), 1);
    assert_eq!(erasure.waves[0].delay, 10.0);
    assert_eq!(erasure.waves[0].radius, 200.0);
    assert!(!erasure.waves[0].damaging);
    assert_eq!(erasure.waves[0].sound, WaveSound::Medium);

    let sn = spec(Weapon::Supernova);
    assert_eq!(sn.name, "Сверхновая");
    assert_eq!(sn.group, Group::Super);
    assert_eq!(sn.color, 0xFFB347);
    assert_eq!(sn.energy, 5000.0);
    assert_eq!(sn.cooldown, SUPER_COOLDOWN);
    assert_eq!(sn.charge, 10.0);
    assert_eq!(sn.duration, 24.0);
    assert_eq!(sn.reach, 500.0);
    assert_eq!(sn.waves.len(), 3);
    let expected_delays = [10.0, 11.0, 12.4];
    let expected_strengths = [1.0, 0.7, 0.4];
    for (i, wave) in sn.waves.iter().enumerate() {
        assert!((wave.delay - expected_delays[i]).abs() < 1e-6);
        assert!((wave.strength - expected_strengths[i]).abs() < 1e-6);
        assert_eq!(wave.radius, 500.0);
        assert!(wave.damaging);
        assert_eq!(wave.sound, WaveSound::Super);
    }
}

#[test]
fn classic_values_kept() {
    let ion = spec(Weapon::Ion);
    assert_eq!(ion.energy, 60.0);
    assert_eq!(ion.charge, 3.0);
    assert_eq!(ion.duration, 4.0);
    assert_eq!(ion.reach, 16.0);
    assert_eq!(ion.cooldown, 0.25);
    assert_eq!(ion.energy_per_second, 0.0);
    assert_eq!(ion.group, Group::Classic);

    let barrage = spec(Weapon::Barrage);
    assert_eq!(barrage.energy, 50.0);
    assert_eq!(barrage.charge, 0.0);
    assert_eq!(barrage.duration, 4.0);
    assert_eq!(barrage.reach, 32.0);
    assert_eq!(barrage.cooldown, 0.2);

    let rod = spec(Weapon::Rod);
    assert_eq!(rod.energy, 80.0);
    assert_eq!(rod.charge, 0.0);
    assert_eq!(rod.duration, 2.5);
    assert_eq!(rod.reach, 32.0);
    assert_eq!(rod.cooldown, 0.45);

    let lance = spec(Weapon::Lance);
    assert_eq!(lance.energy, 0.0);
    assert_eq!(lance.energy_per_second, 12.0);
    assert_eq!(lance.charge, 0.0);
    assert_eq!(lance.duration, 8.0);
    assert_eq!(lance.reach, 3.0);
    assert_eq!(lance.cooldown, 0.15);
}

#[test]
fn from_index_roundtrip() {
    for i in 0..COUNT {
        let w = Weapon::from_index(i).expect("should exist");
        assert_eq!(w as usize, i);
        assert_eq!(Weapon::ALL[i], w);
    }
    assert_eq!(Weapon::from_index(COUNT), None);
    assert_eq!(Weapon::from_index(COUNT + 1), None);
    assert_eq!(Weapon::from_index(100), None);
}
