#[path = "../mixins/astral_cruiser/lang.rs"] mod astral_lang;
#[path = "../mixins/astral_cruiser/arsenal.rs"] mod astral_arsenal;
mod astral_fx_recipe {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Phase { Launch, Flight, Impact, After }
}
#[path = "../mixins/astral_cruiser/effect_events.rs"] mod effect_events;
use astral_arsenal::Weapon;
use astral_fx_recipe::Phase;
use effect_events::EffectEvents;
use glam::DVec3;

#[test]
fn launch_is_once_and_first_wave_is_per_strike_even_at_same_point() {
    let mut fx = EffectEvents::default();
    for now in [0.0, 0.01, 0.05] {
        fx.launch_once(1, Weapon::Ion, DVec3::ZERO, DVec3::Z, now);
    }
    assert_eq!(fx.advance(0.05).iter().filter(|e| e.phase == Phase::Launch).count(), 1);
    assert!(fx.first_wave(1, 0.1));
    assert!(!fx.first_wave(1, 0.2));
    assert!(fx.first_wave(2, 0.2));
    assert!(fx.advance(0.2).is_empty());
}

#[test]
fn flight_uses_its_own_clock_and_after_keeps_the_world_impact_point() {
    let mut fx = EffectEvents::default();
    fx.projectile(Weapon::Meteors, DVec3::ZERO, DVec3::new(0.0, 0.0, 100.0), 1.0, 0.0);
    fx.advance(0.0);
    assert!(fx.advance(0.05).is_empty());
    let flight = fx.advance(0.1);
    assert_eq!(flight.len(), 1);
    assert_eq!(flight[0].phase, Phase::Flight);
    assert!((flight[0].at.z - 10.0).abs() < 1e-6);
    assert!(fx.advance(0.11).is_empty());
    let hit = DVec3::new(10000.0, 70.0, 10000.0);
    fx.impact(Weapon::Meteors, hit, DVec3::Y, 1.0);
    assert_eq!(fx.advance(1.0)[0].phase, Phase::Impact);
    assert!(fx.advance(2.49).is_empty());
    let after = fx.advance(2.5);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].phase, Phase::After);
    assert_eq!(after[0].at, hit);
    assert!(fx.advance(3.0).is_empty());
}

#[test]
fn advance_reuses_emission_storage_after_a_quiet_frame() {
    let mut fx = EffectEvents::default();
    for _ in 0..8 { fx.phase(Weapon::Ion, Phase::Impact, DVec3::ZERO, DVec3::Y, 0.0); }
    let storage = fx.advance(0.0).as_ptr();
    assert!(fx.advance(0.1).is_empty());
    assert_eq!(fx.advance(0.1).as_ptr(), storage);
    for _ in 0..8 { fx.phase(Weapon::Ion, Phase::Impact, DVec3::ZERO, DVec3::Y, 0.2); }
    assert_eq!(fx.advance(0.2).as_ptr(), storage);
}
