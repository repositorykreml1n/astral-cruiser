#[path = "../mixins/astral_cruiser/layout.rs"]
mod astral_layout;
#[path = "../mixins/astral_cruiser/furnish.rs"]
mod astral_furnish;
#[path = "../mixins/astral_cruiser/passenger.rs"]
mod astral_passenger;

use astral_layout::{Material, ShipLayout};
use astral_passenger::{Passenger, WalkKeys};
use glam::DVec3;
use std::f64::consts::FRAC_PI_2;

const TICK: f64 = 0.05;
const NO_KEYS: WalkKeys =
    WalkKeys { forward: false, back: false, left: false, right: false, jump: false, sprint: false };
const FORWARD: WalkKeys = WalkKeys { forward: true, ..NO_KEYS };

fn walk(p: &mut Passenger, keys: &WalkKeys, heading: f64, layout: &ShipLayout, ticks: u32) {
    for _ in 0..ticks {
        p.tick(keys, heading, layout, TICK);
    }
}

#[test]
fn walking_ahead_covers_the_walking_speed() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -4.0, 0.0));
    walk(&mut p, &FORWARD, 0.0, &layout, 20);
    // One second at 4.3 blocks/s along +Z, the ship's nose.
    assert!((p.local - DVec3::new(0.0, -4.0, 4.3)).length() < 1e-9, "{}", p.local);
}

#[test]
fn sprinting_is_faster_than_walking() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -4.0, 0.0));
    let keys = WalkKeys { sprint: true, ..FORWARD };
    walk(&mut p, &keys, 0.0, &layout, 20);
    assert!((p.local.z - 5.6).abs() < 1e-9, "{}", p.local);
}

#[test]
fn the_heading_turns_the_walk() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(-8.0, -4.0, 10.0));
    walk(&mut p, &FORWARD, FRAC_PI_2, &layout, 20);
    assert!((p.local - DVec3::new(-3.7, -4.0, 10.0)).length() < 1e-9, "{}", p.local);
}

#[test]
fn left_is_the_pilots_left() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(-8.0, -4.0, 10.0));
    let keys = WalkKeys { left: true, ..NO_KEYS };
    walk(&mut p, &keys, 0.0, &layout, 20);
    assert!((p.local - DVec3::new(-3.7, -4.0, 10.0)).length() < 1e-9, "{}", p.local);
}

#[test]
fn a_diagonal_walk_is_no_faster_than_a_straight_one() {
    let layout = ShipLayout::cruiser();
    let start = DVec3::new(-8.0, -4.0, 10.0);
    let mut p = Passenger::at(start);
    let keys = WalkKeys { left: true, ..FORWARD };
    walk(&mut p, &keys, 0.0, &layout, 20);
    assert!(((p.local - start).length() - 4.3).abs() < 1e-9, "{}", p.local);
}

#[test]
fn a_wall_stops_the_walker_at_their_shoulder() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -4.0, 35.0));
    walk(&mut p, &WalkKeys { left: true, ..NO_KEYS }, 0.0, &layout, 40);
    // The corridor's wall is at x = 2.5 and the body 0.3 wide on each side.
    assert!(p.local.x <= 2.2 && p.local.x > 1.9, "{}", p.local);
}

#[test]
fn walking_into_a_wall_at_an_angle_slides_along_it() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -4.0, 20.0));
    let keys = WalkKeys { left: true, ..FORWARD };
    walk(&mut p, &keys, 0.0, &layout, 20);
    assert!(p.local.x <= 2.2);
    assert!(p.local.z > 22.5, "stuck on the wall: {}", p.local);
}

#[test]
fn an_opening_lets_the_walker_through() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -4.0, 11.0));
    walk(&mut p, &WalkKeys { right: true, ..NO_KEYS }, 0.0, &layout, 40);
    // From the corridor through the door into the gun bay.
    assert!(p.local.x < -4.0, "{}", p.local);
}

#[test]
fn the_reactor_core_stops_the_walker() {
    let layout = ShipLayout::cruiser();
    let core = layout.props.iter().find(|p| p.material == Material::Reactor && p.min.x > 0.0).unwrap();
    let mut p = Passenger::at(DVec3::new(4.0, -9.0, -19.0));
    walk(&mut p, &WalkKeys { left: true, ..NO_KEYS }, 0.0, &layout, 60);
    let middle = (core.min.x + core.max.x) / 2.0;
    assert!(p.local.x < middle - 3.0, "walked into the core: {}", p.local);
}

#[test]
fn a_walker_in_the_air_falls_to_the_deck_and_stays() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -2.0, 30.0));
    walk(&mut p, &NO_KEYS, 0.0, &layout, 40);
    assert_eq!(p.local.y, -4.0);
    walk(&mut p, &NO_KEYS, 0.0, &layout, 40);
    assert_eq!(p.local.y, -4.0);
}

#[test]
fn a_jump_leaves_the_deck_and_comes_back() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -4.0, 30.0));
    walk(&mut p, &WalkKeys { jump: true, ..NO_KEYS }, 0.0, &layout, 1);
    walk(&mut p, &NO_KEYS, 0.0, &layout, 5);
    assert!(p.local.y > -3.5, "no jump: {}", p.local);
    walk(&mut p, &NO_KEYS, 0.0, &layout, 40);
    assert_eq!(p.local.y, -4.0);
}

#[test]
fn a_jump_does_not_pass_the_ceiling() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -0.3, 30.0));
    p.rise = 12.0;
    walk(&mut p, &NO_KEYS, 0.0, &layout, 4);
    assert!(p.local.y + 1.8 <= 2.0 + 1e-9, "head through the ceiling: {}", p.local);
}

#[test]
fn a_bad_timestep_moves_nobody() {
    let layout = ShipLayout::cruiser();
    let start = DVec3::new(0.0, -4.0, 30.0);
    let mut p = Passenger::at(start);
    p.tick(&FORWARD, 0.0, &layout, f64::NAN);
    p.tick(&FORWARD, f64::NAN, &layout, 0.05);
    assert_eq!(p.local, start);
}

#[test]
fn someone_outside_every_room_stays_put_rather_than_falling_forever() {
    let layout = ShipLayout::cruiser();
    let start = DVec3::new(100.0, 0.0, 100.0);
    let mut p = Passenger::at(start);
    walk(&mut p, &FORWARD, 0.0, &layout, 40);
    assert_eq!(p.local, start);
}

#[test]
fn furniture_stops_the_walker() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(layout.bridge_stand());
    walk(&mut p, &FORWARD, 0.0, &layout, 80);
    assert!(p.local.z < layout.pilot_seat().z, "walked through the helm chair: {}", p.local);
}

#[test]
fn the_catwalk_carries_the_walker_over_the_pit() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -4.0, -15.0));
    walk(&mut p, &WalkKeys { back: true, ..NO_KEYS }, 0.0, &layout, 40);
    assert_eq!(p.local.y, -4.0, "fell through the catwalk: {}", p.local);
    assert!(p.local.z < -20.0, "{}", p.local);
}

#[test]
fn the_rail_keeps_the_walker_on_the_catwalk() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -4.0, -20.0));
    walk(&mut p, &WalkKeys { left: true, ..NO_KEYS }, 0.0, &layout, 40);
    assert_eq!(p.local.y, -4.0, "went over the rail: {}", p.local);
    assert!(p.local.x < 2.5, "{}", p.local);
}

#[test]
fn stairs_are_walked_down_into_the_pit() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(1.0, -4.0, -11.5));
    walk(&mut p, &WalkKeys { left: true, ..NO_KEYS }, 0.0, &layout, 80);
    assert_eq!(p.local.y, -9.0, "{}", p.local);
    assert!(p.local.x > 11.0, "{}", p.local);
}

#[test]
fn stairs_are_walked_back_up_without_jumping() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(13.0, -9.0, -11.5));
    walk(&mut p, &WalkKeys { right: true, ..NO_KEYS }, 0.0, &layout, 60);
    assert_eq!(p.local.y, -4.0, "{}", p.local);
    assert!(p.local.x < 3.0, "{}", p.local);
}

#[test]
fn a_wall_is_not_a_step() {
    let layout = ShipLayout::cruiser();
    let mut p = Passenger::at(DVec3::new(0.0, -4.0, 6.0));
    walk(&mut p, &WalkKeys { left: true, ..NO_KEYS }, 0.0, &layout, 40);
    assert_eq!(p.local.y, -4.0, "{}", p.local);
    assert!(p.local.x <= 2.2, "{}", p.local);
}
