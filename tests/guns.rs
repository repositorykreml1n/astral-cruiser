#[path = "../mixins/astral_cruiser/lang.rs"]
#[allow(unused)]
mod astral_lang;
#[path = "../mixins/astral_cruiser/arsenal.rs"]
mod astral_arsenal;
#[path = "../mixins/astral_cruiser/guns.rs"]
mod guns;
use astral_arsenal::{spec, Weapon};
use guns::{Battery, Gun, Stage};
use glam::DVec3;

#[test]
fn deployment_charge_recoil_and_idle() {
    let mut gun = Gun::new(Battery::Silo);
    gun.deploy();
    gun.tick(0.49, None, false);
    assert_eq!(gun.stage, Stage::HatchOpening);
    gun.tick(0.01, None, false);
    assert_eq!(gun.stage, Stage::Extending);
    gun.tick(1.0, None, false);
    assert_eq!(gun.stage, Stage::Ready);
    gun.charge(Weapon::Antimatter);
    gun.tick(spec(Weapon::Antimatter).charge / 2.0, None, false);
    assert!((gun.pose().glow - 0.5).abs() < 1e-6);
    gun.tick(spec(Weapon::Antimatter).charge / 2.0, None, false);
    assert_eq!(gun.stage, Stage::Firing);
    gun.tick(0.25, None, false);
    assert_eq!(gun.stage, Stage::Ready);
    gun.tick(4.99, None, false);
    assert_eq!(gun.stage, Stage::Ready);
    gun.tick(0.02, None, false);
    assert_eq!(gun.stage, Stage::Retracting);
    gun.tick(1.5, None, false);
    assert_eq!(gun.stage, Stage::Hidden);
}

#[test]
fn held_deployed_and_aim_moves_muzzle() {
    let mut gun = Gun::new(Battery::Railgun);
    gun.tick(2.0, None, true);
    let before = gun.pose().muzzle;
    gun.tick(20.0, Some(DVec3::new(100.0, -100.0, 150.0)), true);
    assert_eq!(gun.stage, Stage::Ready);
    assert!(gun.pose().muzzle.distance(before) > 1.0);
    assert!((gun.pose().direction.length() - 1.0).abs() < 1e-9);
}

#[test]
fn every_weapon_has_the_specified_battery() {
    use Battery::*;
    let expected = [Emitter, MissilePods, Railgun, Emitter, Silo, Silo, Silo,
        MissilePods, MissilePods, Discharger, Discharger, Discharger,
        Discharger, MissilePods, Emitter, Emitter, Silo, Silo];
    for (weapon, battery) in Weapon::ALL.into_iter().zip(expected) {
        assert_eq!(guns::battery(weapon), battery);
    }
}

#[test]
fn elapsed_time_is_preserved_across_transitions() {
    let mut large = Gun::new(Battery::Emitter);
    let mut small = large.clone();
    large.charge(Weapon::Ion);
    small.charge(Weapon::Ion);
    large.tick(3.0, None, false);
    for _ in 0..60 { small.tick(0.05, None, false); }
    assert_eq!(large.stage, small.stage);
    assert!((large.pose().glow - small.pose().glow).abs() < 1e-8);
}

#[test]
fn aim_turn_rate_is_bounded() {
    let mut gun = Gun::new(Battery::Emitter);
    gun.tick(1.5, None, true);
    let before = gun.pose().direction;
    gun.tick(0.01, Some(DVec3::new(500.0, -20.0, 0.0)), true);
    assert!(before.angle_between(gun.pose().direction) < 0.04);
}
