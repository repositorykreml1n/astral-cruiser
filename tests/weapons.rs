#[path = "../mixins/astral_cruiser/weapons.rs"]
mod astral_weapons;

use astral_weapons::{
    Battery, Body, Cell, Shot, Struck, WeaponKind, first_hit, sight_reach, within_cone,
};
use glam::DVec3;
use std::f64::consts::PI;

const TICK: f64 = 0.05;
const ALL_TURRETS: u32 = 8;

fn hold(battery: &mut Battery, kind: WeaponKind, ticks: u32) -> Vec<Shot> {
    (0..ticks).flat_map(|_| battery.tick(kind, true, TICK)).collect()
}

fn idle(battery: &mut Battery, ticks: u32) -> Vec<Shot> {
    (0..ticks).flat_map(|_| battery.tick(WeaponKind::Turrets, false, TICK)).collect()
}

#[test]
fn turrets_fire_ten_pulses_a_second() {
    let shots = hold(&mut Battery::new(), WeaponKind::Turrets, 20);
    assert_eq!(shots.len(), 10);
}

#[test]
fn turrets_take_turns_round_all_eight_mounts() {
    let shots = hold(&mut Battery::new(), WeaponKind::Turrets, 2 * ALL_TURRETS + 2);
    let mounts: Vec<usize> = shots.iter().take(9).map(|s| s.mount).collect();
    assert_eq!(mounts, [0, 1, 2, 3, 4, 5, 6, 7, 0]);
}

#[test]
fn the_nose_guns_alternate_at_their_own_slower_pace() {
    let shots = hold(&mut Battery::new(), WeaponKind::NosePulse, 26);
    let mounts: Vec<usize> = shots.iter().map(|s| s.mount).collect();
    assert_eq!(mounts.len(), 5);
    assert_eq!(mounts[..3], [8, 9, 8]);
}

#[test]
fn the_belly_beam_hits_once_each_tick_for_its_damage_per_second() {
    let shots = hold(&mut Battery::new(), WeaponKind::BellyBeam, 20);
    assert_eq!(shots.len(), 20);
    assert!(shots.iter().all(|s| s.mount == 10));
    let total: f64 = shots.iter().map(|s| s.damage).sum();
    assert!((total - 30.0).abs() < 1e-9, "{total}");
}

#[test]
fn the_main_beam_fires_only_at_full_charge() {
    let mut battery = Battery::new();
    assert!(hold(&mut battery, WeaponKind::MainBeam, 39).is_empty());
    assert!(battery.charge > 0.9 && battery.charge < 1.0, "{}", battery.charge);
    assert_eq!(hold(&mut battery, WeaponKind::MainBeam, 1).len(), 1);
}

#[test]
fn letting_go_early_loses_the_charge() {
    let mut battery = Battery::new();
    hold(&mut battery, WeaponKind::MainBeam, 20);
    assert!(idle(&mut battery, 40).is_empty());
    assert_eq!(battery.charge, 0.0);
}

#[test]
fn a_fired_main_beam_burns_its_full_length_even_after_letting_go() {
    let mut battery = Battery::new();
    let mut shots = hold(&mut battery, WeaponKind::MainBeam, 40);
    shots.extend(idle(&mut battery, 60));
    assert_eq!(shots.len(), 30, "a second and a half of ticks");
    assert!(shots.iter().all(|s| s.mount == 11 && s.kind == WeaponKind::MainBeam));
}

#[test]
fn the_main_beam_needs_time_before_it_can_charge_again() {
    let mut battery = Battery::new();
    hold(&mut battery, WeaponKind::MainBeam, 40);
    battery.energy = 100.0;
    idle(&mut battery, 40);
    let again = hold(&mut battery, WeaponKind::MainBeam, 41);
    assert!(again.is_empty(), "fired {} shots during the cooldown", again.len());
}

#[test]
fn changing_weapon_drops_the_charge() {
    let mut battery = Battery::new();
    hold(&mut battery, WeaponKind::MainBeam, 30);
    battery.tick(WeaponKind::Turrets, true, TICK);
    assert_eq!(battery.charge, 0.0);
}

#[test]
fn firing_spends_energy_and_an_empty_battery_holds_fire() {
    let mut battery = Battery::new();
    let full = battery.energy;
    hold(&mut battery, WeaponKind::NosePulse, 1);
    assert!(battery.energy < full);
    battery.energy = 0.0;
    assert!(hold(&mut battery, WeaponKind::NosePulse, 1).is_empty());
    assert!(hold(&mut battery, WeaponKind::BellyBeam, 1).is_empty());
}

#[test]
fn the_battery_refills_when_nothing_fires() {
    let mut battery = Battery::new();
    battery.energy = 10.0;
    idle(&mut battery, 20);
    assert!(battery.energy > 10.0);
    idle(&mut battery, 400);
    assert_eq!(battery.energy, Battery::new().energy);
}

#[test]
fn overheating_locks_every_weapon_until_the_guns_have_cooled() {
    let mut battery = Battery::new();
    battery.heat = 0.999;
    hold(&mut battery, WeaponKind::BellyBeam, 2);
    assert!(battery.locked);
    assert!(hold(&mut battery, WeaponKind::Turrets, 10).is_empty());
    idle(&mut battery, 200);
    assert!(!battery.locked);
    assert!(!hold(&mut battery, WeaponKind::Turrets, 4).is_empty());
}

#[test]
fn heat_falls_when_nothing_fires() {
    let mut battery = Battery::new();
    battery.heat = 0.5;
    idle(&mut battery, 20);
    assert!(battery.heat < 0.5);
}

#[test]
fn a_saved_battery_comes_back_with_what_it_had() {
    let mut battery = Battery::new();
    hold(&mut battery, WeaponKind::BellyBeam, 40);
    let restored = Battery::restored(battery.energy, battery.heat);
    assert_eq!((restored.energy, restored.heat), (battery.energy, battery.heat));
    assert!(!restored.locked);
}

#[test]
fn a_bad_timestep_fires_nothing_and_changes_nothing() {
    let mut battery = Battery::new();
    assert!(battery.tick(WeaponKind::Turrets, true, f64::NAN).is_empty());
    assert!(battery.tick(WeaponKind::Turrets, true, -1.0).is_empty());
    assert_eq!(battery, Battery::new());
}

#[test]
fn weapons_step_round_in_order_both_ways() {
    assert_eq!(WeaponKind::Turrets.stepped(1), WeaponKind::NosePulse);
    assert_eq!(WeaponKind::MainBeam.stepped(1), WeaponKind::Turrets);
    assert_eq!(WeaponKind::Turrets.stepped(-1), WeaponKind::MainBeam);
}

fn wall_at_x(x: i32) -> impl Fn(i32, i32, i32) -> Cell {
    move |at, _, _| if at == x { Cell::Solid } else { Cell::Clear }
}

fn body(id: u32, min: [f64; 3], max: [f64; 3]) -> Body {
    Body { id, min: DVec3::from_array(min), max: DVec3::from_array(max) }
}

#[test]
fn a_shot_stops_at_the_first_block() {
    let hit = first_hit(DVec3::new(0.5, 0.5, 0.5), DVec3::X, 100.0, wall_at_x(10), &[]);
    assert_eq!(hit.struck, Struck::Block);
    assert!((hit.point.x - 10.0).abs() < 1e-6, "{}", hit.point);
}

#[test]
fn a_shot_finds_a_body_before_the_wall_behind_it() {
    let target = body(7, [4.0, 0.0, 0.0], [5.0, 2.0, 1.0]);
    let hit = first_hit(DVec3::new(0.5, 0.5, 0.5), DVec3::X, 100.0, wall_at_x(10), &[target]);
    assert_eq!(hit.struck, Struck::Body(7));
    assert!((hit.point.x - 4.0).abs() < 1e-6, "{}", hit.point);
}

#[test]
fn a_sight_far_behind_the_ship_still_finds_what_the_guns_can_reach() {
    let (ship, eye) = (DVec3::new(0.5, 0.5, 200.5), DVec3::new(0.5, 0.5, 24.5));
    let target = body(7, [0.0, 0.0, 480.0], [1.0, 2.0, 481.0]);
    let reach = sight_reach(eye, ship, 300.0);
    let hit = first_hit(eye, DVec3::Z, reach, |_, _, _| Cell::Clear, &[target]);
    assert_eq!(hit.struck, Struck::Body(7));
}

#[test]
fn a_sight_on_the_ship_reaches_as_far_as_the_guns() {
    let ship = DVec3::new(3.0, 4.0, 5.0);
    assert_eq!(sight_reach(ship, ship, 300.0), 300.0);
}

#[test]
fn a_wall_hides_the_body_behind_it() {
    let target = body(7, [14.0, 0.0, 0.0], [15.0, 2.0, 1.0]);
    let hit = first_hit(DVec3::new(0.5, 0.5, 0.5), DVec3::X, 100.0, wall_at_x(10), &[target]);
    assert_eq!(hit.struck, Struck::Block);
}

#[test]
fn of_two_bodies_the_nearer_is_struck() {
    let far = body(1, [9.0, 0.0, 0.0], [10.0, 2.0, 1.0]);
    let near = body(2, [3.0, 0.0, 0.0], [4.0, 2.0, 1.0]);
    let hit = first_hit(DVec3::new(0.5, 0.5, 0.5), DVec3::X, 100.0, |_, _, _| Cell::Clear, &[far, near]);
    assert_eq!(hit.struck, Struck::Body(2));
}

#[test]
fn a_shot_that_meets_nothing_ends_at_its_reach() {
    let hit = first_hit(DVec3::ZERO, DVec3::Z, 50.0, |_, _, _| Cell::Clear, &[]);
    assert_eq!(hit.struck, Struck::Nothing);
    assert!((hit.point - DVec3::Z * 50.0).length() < 1e-9);
}

#[test]
fn a_shot_ends_where_the_world_is_not_loaded() {
    let unloaded = |x: i32, _: i32, _: i32| if x >= 20 { Cell::Unloaded } else { Cell::Clear };
    let hit = first_hit(DVec3::new(0.5, 0.5, 0.5), DVec3::X, 100.0, unloaded, &[]);
    assert_eq!(hit.struck, Struck::Nothing);
    assert!(hit.point.x <= 20.0, "{}", hit.point);
}

#[test]
fn a_slanted_shot_visits_every_block_on_its_way() {
    let dir = DVec3::new(1.0, 1.0, 0.0).normalize();
    let wall = |x: i32, y: i32, _: i32| if x == 5 && y == 4 { Cell::Solid } else { Cell::Clear };
    let hit = first_hit(DVec3::new(0.2, 0.0, 0.5), dir, 100.0, wall, &[]);
    assert_eq!(hit.struck, Struck::Block);
}

#[test]
fn a_shot_from_inside_a_body_strikes_it_at_once() {
    let target = body(3, [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
    let hit = first_hit(DVec3::ZERO, DVec3::X, 100.0, |_, _, _| Cell::Clear, &[target]);
    assert_eq!(hit.struck, Struck::Body(3));
}

#[test]
fn a_bad_direction_goes_nowhere() {
    let hit = first_hit(DVec3::ZERO, DVec3::new(f64::NAN, 0.0, 0.0), 100.0, |_, _, _| Cell::Clear, &[]);
    assert_eq!(hit.struck, Struck::Nothing);
    assert_eq!(hit.point, DVec3::ZERO);
}

#[test]
fn an_aim_inside_the_cone_is_kept() {
    let wanted = DVec3::new(0.1, 0.0, 1.0).normalize();
    assert!((within_cone(DVec3::Z, wanted, 0.5) - wanted).length() < 1e-9);
}

#[test]
fn an_aim_outside_the_cone_comes_back_to_its_edge() {
    let limited = within_cone(DVec3::Z, DVec3::X, 0.5);
    assert!((limited.dot(DVec3::Z).acos() - 0.5).abs() < 1e-9);
    assert!(limited.x > 0.0 && limited.y.abs() < 1e-9);
    assert!((limited.length() - 1.0).abs() < 1e-9);
}

#[test]
fn an_aim_straight_behind_the_gun_goes_along_the_gun() {
    assert_eq!(within_cone(DVec3::Z, -DVec3::Z, PI / 8.0), DVec3::Z);
}
