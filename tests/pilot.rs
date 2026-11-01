#[path = "../mixins/astral_cruiser/core.rs"]
mod astral_core;
#[path = "../mixins/astral_cruiser/pilot.rs"]
mod astral_pilot;

use astral_core::{CollisionQuery, ShipPose};
use astral_pilot::{
    Arrival, Footing, PilotKeys, ShipIs, Step, chase_camera, ease_angle, flight_input, footing,
    landing_y, near_door, refusal, rise_out, roof_at, solid_cells, spawn_pose, streaks,
    turret_angles, yaw_of,
};
use glam::{DQuat, DVec3};
use std::f64::consts::FRAC_PI_2;

/// Ground under the whole hull, up to this height.
struct Ground(f64);
impl CollisionQuery for Ground {
    fn is_clear(&self, pose: &ShipPose, half: DVec3) -> bool {
        pose.position.y - half.y >= self.0
    }
}

const HALF: DVec3 = DVec3::new(48.0, 18.0, 80.0);

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// The ship's rotation for a Minecraft yaw: yaw 0 faces +Z, yaw 90 faces -X.
fn facing(yaw_degrees: f64) -> DQuat {
    DQuat::from_rotation_y(-yaw_degrees.to_radians())
}

#[test]
fn w_thrusts_forward_and_s_backward() {
    let forward = PilotKeys { forward: true, ..Default::default() };
    let back = PilotKeys { back: true, ..Default::default() };
    assert_eq!(flight_input(&forward, DQuat::IDENTITY, 0.0).thrust, DVec3::Z);
    assert_eq!(flight_input(&back, DQuat::IDENTITY, 0.0).thrust, -DVec3::Z);
}

#[test]
fn a_strafes_toward_the_pilots_left() {
    // Facing +Z with Y up the pilot's left is +X, as for a player at yaw 0.
    let left = PilotKeys { left: true, ..Default::default() };
    let right = PilotKeys { right: true, ..Default::default() };
    assert_eq!(flight_input(&left, DQuat::IDENTITY, 0.0).thrust, DVec3::X);
    assert_eq!(flight_input(&right, DQuat::IDENTITY, 0.0).thrust, -DVec3::X);
}

#[test]
fn space_rises_and_shift_descends() {
    let up = PilotKeys { up: true, ..Default::default() };
    let down = PilotKeys { down: true, ..Default::default() };
    assert_eq!(flight_input(&up, DQuat::IDENTITY, 0.0).thrust, DVec3::Y);
    assert_eq!(flight_input(&down, DQuat::IDENTITY, 0.0).thrust, -DVec3::Y);
}

#[test]
fn opposite_keys_cancel() {
    let keys = PilotKeys { forward: true, back: true, left: true, right: true, ..Default::default() };
    assert_eq!(flight_input(&keys, DQuat::IDENTITY, 0.0).thrust, DVec3::ZERO);
}

#[test]
fn ctrl_engages_boost() {
    let keys = PilotKeys { boost: true, ..Default::default() };
    assert!(flight_input(&keys, DQuat::IDENTITY, 0.0).boost);
    assert!(!flight_input(&PilotKeys::default(), DQuat::IDENTITY, 0.0).boost);
}

#[test]
fn a_ship_already_on_course_does_not_turn() {
    let input = flight_input(&PilotKeys::default(), facing(37.0), 37.0);
    assert!(close(input.yaw_rate, 0.0));
}

#[test]
fn a_small_error_turns_slowly_toward_the_look_direction() {
    // Looking 0.1 rad to the ship's left: a yaw of -5.7296 degrees.
    let input = flight_input(&PilotKeys::default(), DQuat::IDENTITY, -5.729_578);
    assert!((input.yaw_rate - 0.2).abs() < 1e-6, "{}", input.yaw_rate);
}

#[test]
fn a_large_error_turns_at_the_limit() {
    let input = flight_input(&PilotKeys::default(), DQuat::IDENTITY, 90.0);
    assert!(close(input.yaw_rate, -0.6), "{}", input.yaw_rate);
}

#[test]
fn the_ship_turns_the_short_way_across_the_seam() {
    // From yaw 170 to yaw -170 is 20 degrees further on, not 340 back.
    let input = flight_input(&PilotKeys::default(), facing(170.0), -170.0);
    assert!(close(input.yaw_rate, -0.6), "{}", input.yaw_rate);
}

#[test]
fn a_non_finite_look_direction_does_not_turn_the_ship() {
    let input = flight_input(&PilotKeys::default(), facing(10.0), f32::NAN);
    assert_eq!(input.yaw_rate, 0.0);
}

#[test]
fn yaw_of_reads_the_turn_back_out_of_a_rotation() {
    assert!(close(yaw_of(DQuat::from_rotation_y(1.25)), 1.25));
    assert!(close(yaw_of(DQuat::from_rotation_y(-2.5)), -2.5));
}

#[test]
fn a_clear_ship_stays_where_it_is() {
    let pose = ShipPose::new(DVec3::new(1.0, 200.0, 2.0), DQuat::IDENTITY);
    assert_eq!(rise_out(&pose, HALF, 320.0, &Ground(100.0)), None);
}

#[test]
fn a_ship_caught_in_the_ground_climbs_straight_up() {
    let rotation = DQuat::from_rotation_y(0.7);
    let pose = ShipPose::new(DVec3::new(1.0, 100.0, 2.0), rotation);
    let raised = rise_out(&pose, HALF, 320.0, &Ground(110.0)).expect("it is stuck");
    assert_eq!(raised.position, DVec3::new(1.0, 101.5, 2.0));
    assert_eq!(raised.rotation, pose.rotation);
}

#[test]
fn a_stuck_ship_stops_climbing_at_the_top_of_the_world() {
    let pose = ShipPose::new(DVec3::new(0.0, 302.0, 0.0), DQuat::IDENTITY);
    assert_eq!(rise_out(&pose, HALF, 320.0, &Ground(1000.0)), None);
}

#[test]
fn climbing_ends_once_the_hull_is_clear() {
    let mut pose = ShipPose::new(DVec3::new(0.0, 100.0, 0.0), DQuat::IDENTITY);
    let mut steps = 0;
    while let Some(raised) = rise_out(&pose, HALF, 320.0, &Ground(90.0)) {
        pose = raised;
        steps += 1;
        assert!(steps < 100, "it never stopped");
    }
    assert!(pose.position.y - HALF.y >= 90.0);
    assert!(pose.position.y - HALF.y < 91.5);
}

/// A view from `camera` along yaw and pitch: where a point falls, as tangents of
/// the angles right of and above the middle of the view.
fn on_screen(camera: DVec3, yaw: f32, pitch: f32, point: DVec3) -> (f64, f64) {
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let forward = DVec3::new(-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos());
    let right = DVec3::new(-y.cos(), 0.0, -y.sin());
    let up = right.cross(forward);
    let to = point - camera;
    let depth = to.dot(forward);
    assert!(depth > 0.0, "{point} is behind the camera");
    (to.dot(right) / depth, to.dot(up) / depth)
}

#[test]
fn the_chase_camera_holds_every_corner_of_the_ship_in_view() {
    // The default 70 degree view at 16:9, whichever way the pilot looks.
    let tall = 35.0_f64.to_radians().tan();
    let wide = tall * 16.0 / 9.0;
    let center = DVec3::new(100.0, 90.0, -40.0);
    for (yaw, pitch) in [(0.0, 0.0), (0.0, 25.0), (90.0, 10.0), (200.0, -20.0), (45.0, 60.0)] {
        let camera = chase_camera(center, yaw, pitch, false);
        for x in [-48.0, 48.0] {
            for y in [-18.0, 18.0] {
                for z in [-80.0, 80.0] {
                    // The ship follows the view, so it lies along the yaw.
                    let turn = DQuat::from_rotation_y(-f64::from(yaw).to_radians());
                    let corner = center + turn * DVec3::new(x, y, z);
                    let (right, above) = on_screen(camera, yaw, pitch, corner);
                    assert!(
                        right.abs() < wide && above.abs() < tall,
                        "yaw {yaw} pitch {pitch}: corner {x} {y} {z} at {right:.2} {above:.2}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_chase_camera_sits_behind_and_above_a_level_ship() {
    let camera = chase_camera(DVec3::ZERO, 0.0, 0.0, false);
    assert!(camera.z < -100.0, "{camera}");
    assert!(camera.y > 0.0, "{camera}");
    assert!(close(camera.x, 0.0));
}

#[test]
fn the_front_view_looks_back_at_the_ship_from_ahead_of_it() {
    let camera = chase_camera(DVec3::ZERO, 0.0, 0.0, true);
    assert!(camera.z > 100.0, "{camera}");
    assert!(camera.y > 0.0, "{camera}");
}

fn deck(x: f64, _z: f64) -> Vec<(f64, f64)> {
    if x.abs() < 5.0 { vec![(-1.0, 2.0)] } else { Vec::new() }
}

#[test]
fn hull_cells_stand_at_the_ships_height_to_the_block() {
    let pose = ShipPose::new(DVec3::new(10.0, 100.25, 20.0), DQuat::IDENTITY);
    let cells = solid_cells(&pose, DVec3::new(10.2, 102.3, 20.2), 2.0, 0.5, deck);
    assert!(!cells.is_empty());
    for [_, low, _, _, high, _] in cells {
        assert_eq!((low, high), (99.25, 102.25));
    }
}

#[test]
fn hull_cells_cover_the_ground_around_the_player() {
    let pose = ShipPose::new(DVec3::new(0.0, 100.0, 0.0), DQuat::IDENTITY);
    let around = DVec3::new(0.2, 102.0, 0.2);
    let cells = solid_cells(&pose, around, 2.0, 0.5, deck);
    let under = |x: f64, z: f64| cells.iter().any(|c| c[0] <= x && x < c[3] && c[2] <= z && z < c[5]);
    assert!(under(0.2, 0.2) && under(-1.5, 1.7) && under(1.9, -1.9));
}

#[test]
fn hull_cells_end_at_the_side_of_the_hull() {
    let pose = ShipPose::new(DVec3::new(0.0, 100.0, 0.0), DQuat::IDENTITY);
    let cells = solid_cells(&pose, DVec3::new(6.0, 102.0, 0.0), 3.0, 0.5, deck);
    assert!(cells.iter().all(|c| c[0] < 5.5), "a cell past the side");
    assert!(!cells.is_empty());
}

#[test]
fn hull_cells_turn_with_the_ship() {
    // Turned a quarter, the deck's 10 wide side lies along the world's Z.
    let pose = ShipPose::new(DVec3::ZERO, DQuat::from_rotation_y(FRAC_PI_2));
    let on_it = solid_cells(&pose, DVec3::new(0.0, 2.0, 4.0), 1.0, 0.5, deck);
    let beside_it = solid_cells(&pose, DVec3::new(0.0, 2.0, 8.0), 1.0, 0.5, deck);
    assert!(!on_it.is_empty());
    assert!(beside_it.is_empty());
}

#[test]
fn hull_cells_skip_what_is_far_above_or_below() {
    let pose = ShipPose::new(DVec3::new(0.0, 100.0, 0.0), DQuat::IDENTITY);
    assert!(solid_cells(&pose, DVec3::new(0.0, 60.0, 0.0), 3.0, 0.5, deck).is_empty());
}

const SPANS: [(f64, f64); 2] = [(-10.0, 6.0), (6.0, 10.0)];

#[test]
fn standing_on_the_hull_is_being_carried() {
    assert_eq!(footing(10.0, &SPANS, 3.0), Footing::Carried);
}

#[test]
fn a_jump_above_the_hull_is_still_carried() {
    assert_eq!(footing(12.5, &SPANS, 3.0), Footing::Carried);
}

#[test]
fn someone_high_above_the_hull_is_on_their_own() {
    assert_eq!(footing(14.0, &SPANS, 3.0), Footing::Clear);
}

#[test]
fn someone_under_the_belly_is_on_their_own() {
    assert_eq!(footing(-12.0, &SPANS, 3.0), Footing::Clear);
}

#[test]
fn someone_the_hull_flew_into_is_lifted_to_its_top() {
    assert_eq!(footing(4.0, &[(-10.0, 10.0)], 3.0), Footing::Buried(10.0));
}

#[test]
fn nothing_is_carried_where_there_is_no_hull() {
    assert_eq!(footing(10.0, &[], 3.0), Footing::Clear);
}

#[test]
fn a_new_ship_appears_ahead_of_and_above_the_player_facing_away() {
    let pose = spawn_pose(DVec3::new(10.0, 100.0, 20.0), 0.0);
    assert!(close(pose.position.x, 10.0));
    assert!(close(pose.position.y, 130.0));
    assert!(close(pose.position.z, 150.0));
    let forward = pose.rotation * DVec3::Z;
    assert!((forward - DVec3::Z).length() < 1e-9);
}

#[test]
fn a_new_ship_follows_the_players_heading() {
    // Yaw 90 faces -X: the hull's nose and the offset both go that way.
    let pose = spawn_pose(DVec3::ZERO, 90.0);
    assert!(close(pose.position.x, -130.0));
    assert!(close(pose.position.z, 0.0));
    let forward = pose.rotation * DVec3::Z;
    assert!((forward + DVec3::X).length() < 1e-9);
}

/// A column of air over `kind` from height `top` down.
fn column(top: i32, kind: Arrival) -> impl Fn(i32) -> Arrival {
    move |y| if y <= top { kind } else { Arrival::Open }
}

#[test]
fn the_beam_sets_down_on_top_of_the_first_ground_under_the_door() {
    assert_eq!(landing_y(120, -64, column(70, Arrival::Ground)), Some(71));
}

#[test]
fn the_beam_stops_at_the_nearest_ground_not_the_lowest() {
    let ledge_over_a_valley = |y: i32| match y {
        90 | 40 => Arrival::Ground,
        _ => Arrival::Open,
    };
    assert_eq!(landing_y(120, -64, ledge_over_a_valley), Some(91));
}

#[test]
fn a_door_resting_on_ground_sets_down_right_above_it() {
    assert_eq!(landing_y(70, -64, column(70, Arrival::Ground)), Some(71));
}

#[test]
fn the_beam_will_not_set_down_on_danger() {
    assert_eq!(landing_y(120, -64, column(70, Arrival::Danger)), None);
}

#[test]
fn the_beam_will_not_set_down_on_ground_not_loaded() {
    assert_eq!(landing_y(120, -64, column(70, Arrival::Unknown)), None);
}

#[test]
fn the_beam_finds_nothing_over_the_void() {
    assert_eq!(landing_y(120, 0, |_| Arrival::Open), None);
}

#[test]
fn the_door_reaches_someone_on_the_ground_far_below_it() {
    let door = DVec3::new(0.0, 200.0, 0.0);
    assert!(near_door(DVec3::new(10.0, 70.0, 10.0), door, 16.0));
}

#[test]
fn the_door_does_not_reach_someone_too_far_to_the_side() {
    let door = DVec3::new(0.0, 200.0, 0.0);
    assert!(!near_door(DVec3::new(20.0, 200.0, 0.0), door, 16.0));
}

#[test]
fn only_someone_aboard_can_leave() {
    assert!(refusal(Step::Leave, false, ShipIs::Here).is_some());
    assert_eq!(refusal(Step::Leave, true, ShipIs::Here), None);
}

#[test]
fn only_someone_outside_can_enter() {
    assert!(refusal(Step::Enter, true, ShipIs::Here).is_some());
    assert_eq!(refusal(Step::Enter, false, ShipIs::Here), None);
}

#[test]
fn the_ship_under_the_players_feet_cannot_be_replaced_or_removed() {
    assert!(refusal(Step::Spawn, true, ShipIs::Here).is_some());
    assert!(refusal(Step::Remove, true, ShipIs::Here).is_some());
    assert_eq!(refusal(Step::Spawn, false, ShipIs::Here), None);
    assert_eq!(refusal(Step::Remove, false, ShipIs::Here), None);
}

#[test]
fn a_ship_can_be_spawned_where_there_is_none() {
    assert_eq!(refusal(Step::Spawn, false, ShipIs::Missing), None);
}

#[test]
fn a_ship_in_another_dimension_cannot_be_reached() {
    assert!(refusal(Step::Board, false, ShipIs::Elsewhere).is_some());
    assert!(refusal(Step::Enter, false, ShipIs::Elsewhere).is_some());
}

#[test]
fn a_ship_in_another_dimension_can_still_be_summoned_or_removed() {
    assert_eq!(refusal(Step::Spawn, false, ShipIs::Elsewhere), None);
    assert_eq!(refusal(Step::Remove, false, ShipIs::Elsewhere), None);
}

#[test]
fn nothing_can_be_done_with_a_ship_that_does_not_exist() {
    for step in [Step::Remove, Step::Board, Step::Enter] {
        assert!(refusal(step, false, ShipIs::Missing).is_some(), "{step:?}");
    }
}

#[test]
fn a_turret_at_rest_looks_ahead() {
    let [yaw, pitch] = turret_angles(DVec3::new(10.0, 5.0, 100.0), DVec3::new(10.0, 5.0, 0.0));
    assert!(close(yaw, 0.0) && close(pitch, 0.0));
}

#[test]
fn a_turret_turns_toward_a_target_on_its_left() {
    let [yaw, pitch] = turret_angles(DVec3::new(50.0, 0.0, 0.0), DVec3::ZERO);
    assert!(close(yaw, FRAC_PI_2) && close(pitch, 0.0));
}

/// A flat deck 20 wide and 40 long, 5 thick, with its top 6 above the ship's middle.
fn plate(x: f64, z: f64) -> Vec<(f64, f64)> {
    if x.abs() <= 10.0 && z.abs() <= 20.0 { vec![(1.0, 6.0)] } else { Vec::new() }
}

#[test]
fn the_roof_over_a_spot_is_the_top_of_the_hull_there() {
    let pose = ShipPose::new(DVec3::new(100.0, 70.0, 100.0), DQuat::IDENTITY);
    assert_eq!(roof_at(&pose, 105.0, 110.0, plate), Some(76.0));
}

#[test]
fn off_the_hull_there_is_no_roof() {
    let pose = ShipPose::new(DVec3::new(100.0, 70.0, 100.0), DQuat::IDENTITY);
    assert_eq!(roof_at(&pose, 125.0, 100.0, plate), None);
}

#[test]
fn the_roof_turns_with_the_ship() {
    let pose = ShipPose::new(DVec3::new(100.0, 70.0, 100.0), DQuat::from_rotation_y(FRAC_PI_2));
    assert_eq!(roof_at(&pose, 115.0, 100.0, plate), Some(76.0));
    assert_eq!(roof_at(&pose, 100.0, 115.0, plate), None);
}

#[test]
fn far_from_the_ship_the_hull_is_not_even_asked() {
    let pose = ShipPose::new(DVec3::ZERO, DQuat::IDENTITY);
    let asked = |_: f64, _: f64| -> Vec<(f64, f64)> { panic!("the hull was asked") };
    assert_eq!(roof_at(&pose, 900.0, 0.0, asked), None);
}

#[test]
fn nothing_streaks_past_a_slow_ship() {
    assert!(streaks(DVec3::ZERO, DVec3::new(100.0, 0.0, 0.0)).is_empty());
}

#[test]
fn streaks_run_along_the_ships_way_clear_of_the_hull() {
    let center = DVec3::new(1000.0, 346.0, -2000.0);
    let lines = streaks(center, DVec3::new(800.0, 0.0, 0.0));
    assert!(lines.len() > 50, "{}", lines.len());
    for (from, to) in &lines {
        assert!(((*to - *from).normalize() + DVec3::X).length() < 1e-9);
        assert!(from.distance(center) > 100.0, "{from}");
    }
}

#[test]
fn streaks_hang_still_in_the_air_as_the_ship_goes_by() {
    let (center, velocity) = (DVec3::new(10.0, 346.0, 20.0), DVec3::new(0.0, 0.0, 800.0));
    let (before, after) = (streaks(center, velocity), streaks(center + DVec3::Z * 48.0, velocity));
    let kept = before.iter().filter(|line| after.contains(line)).count();
    assert!(kept > before.len() / 2, "{kept} of {}", before.len());
}

#[test]
fn a_turret_looks_up_at_a_target_above() {
    let [_, pitch] = turret_angles(DVec3::new(0.0, 30.0, 30.0), DVec3::ZERO);
    assert!(close(pitch, std::f64::consts::FRAC_PI_4));
}

#[test]
fn a_turret_turns_the_short_way_and_no_faster_than_it_can() {
    assert!(close(ease_angle(3.0, -3.0, 0.1), 3.1));
    assert!(close(ease_angle(0.0, 2.0, 0.1), 0.1));
    assert!(close(ease_angle(0.0, 0.05, 0.1), 0.05));
}

#[test]
fn mobs_by_the_ship_are_drawn_for_a_camera_far_behind_it() {
    let scale = 1.5;
    let mob_range = 64.0 * scale;
    assert!(astral_pilot::mob_reach(scale, true) * 64.0 > astral_pilot::CHASE_BEHIND + mob_range);
    assert_eq!(astral_pilot::mob_reach(scale, false), scale);
}

#[test]
fn the_ships_own_eye_is_clear_of_its_nose() {
    assert!(astral_pilot::SHIP_EYE.z > 80.0);
}
