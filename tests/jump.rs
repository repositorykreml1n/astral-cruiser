#[path = "../mixins/astral_cruiser/core.rs"]
mod astral_core;
#[path = "../mixins/astral_cruiser/dash.rs"]
mod astral_dash;
#[path = "../mixins/astral_cruiser/jump.rs"]
mod astral_jump;

use astral_core::{CollisionQuery, Dimension, NavigationLimits, NavigationTarget, ShipPose};
use astral_dash::DashError;
use astral_jump::{Jump, JumpStage, Leap, arrival_area, plan_jump};
use glam::{DQuat, DVec3};

const TICK: f64 = 0.05;
const HALF: DVec3 = DVec3::new(10.0, 10.0, 10.0);
const LIMITS: NavigationLimits = NavigationLimits {
    min_y: -64.0,
    max_y: 320.0,
    border: 100_000.0,
    half_extents: HALF,
};

struct Blocks(Vec<[f64; 6]>);

impl CollisionQuery for Blocks {
    fn is_clear(&self, pose: &ShipPose, half: DVec3) -> bool {
        let (low, high) = (pose.position - half, pose.position + half);
        !self.0.iter().any(|b| {
            low.x < b[3] && high.x > b[0] && low.y < b[4] && high.y > b[1] && low.z < b[5] && high.z > b[2]
        })
    }
}

fn start() -> ShipPose {
    ShipPose::new(DVec3::new(0.0, 100.0, 0.0), DQuat::IDENTITY)
}

fn target(x: f64, y: f64, z: f64) -> NavigationTarget {
    NavigationTarget { dimension: Dimension::Overworld, position: DVec3::new(x, y, z) }
}

fn ready(at: NavigationTarget) -> Jump {
    let mut jump = Jump::new(start(), at);
    let exit = plan_jump(&start(), &at, &LIMITS, &Blocks(Vec::new())).unwrap();
    jump.prepared(exit);
    jump
}

#[test]
fn the_ship_comes_out_short_of_the_target_and_on_the_way_to_it() {
    let at = target(5000.0, 100.0, 0.0);
    let exit = plan_jump(&start(), &at, &LIMITS, &Blocks(Vec::new())).unwrap();
    let short = at.position - exit;
    assert!(short.length() > 100.0 && short.length() < 400.0, "{short}");
    assert!(short.x > 0.0 && short.y.abs() < 1e-9 && short.z.abs() < 1e-9, "{short}");
}

#[test]
fn a_hill_on_the_way_in_makes_it_come_down_from_above() {
    let terrain = Blocks(vec![[4850.0, 0.0, -300.0, 4900.0, 130.0, 300.0]]);
    let at = target(5000.0, 100.0, 0.0);
    let exit = plan_jump(&start(), &at, &LIMITS, &terrain).unwrap();
    assert!(exit.y > 130.0, "{exit}");
}

#[test]
fn nowhere_to_come_in_from_is_refused() {
    // A deep shaft with a lid over it: the point is free, no way leads to it.
    let terrain = Blocks(vec![
        [4900.0, 0.0, -300.0, 4980.0, 400.0, 300.0],
        [5020.0, 0.0, -300.0, 5100.0, 400.0, 300.0],
        [4900.0, 125.0, -300.0, 5100.0, 400.0, 300.0],
    ]);
    let route = plan_jump(&start(), &target(5000.0, 100.0, 0.0), &LIMITS, &terrain);
    assert_eq!(route, Err(DashError::Blocked));
}

#[test]
fn an_occupied_point_is_refused_with_the_lowest_that_fits() {
    let terrain = Blocks(vec![[4900.0, 0.0, -300.0, 5100.0, 95.0, 300.0]]);
    let route = plan_jump(&start(), &target(5000.0, 90.0, 0.0), &LIMITS, &terrain);
    assert_eq!(route, Err(DashError::Occupied { lowest: Some(106.0) }));
}

#[test]
fn the_chunks_loaded_cover_the_way_in_and_the_hull() {
    let rotation = DQuat::from_rotation_y(0.4);
    let to = DVec3::new(1000.0, 90.0, -700.0);
    let ((x, z), radius) = arrival_area(to, rotation);
    let reach = f64::from(radius * 16);
    let middle = DVec3::new(f64::from(x * 16 + 8), 90.0, f64::from(z * 16 + 8));
    let forward = rotation * DVec3::Z;
    for along in [0.0, 100.0, 200.0] {
        let at = to - forward * along;
        assert!((at.x - middle.x).abs() < reach && (at.z - middle.z).abs() < reach, "{along}");
    }
}

#[test]
fn nothing_happens_until_the_charge_is_full_and_the_far_side_ready() {
    let at = target(5000.0, 100.0, 0.0);
    let mut jump = Jump::new(start(), at);
    for _ in 0..400 {
        let moment = jump.tick(TICK);
        assert!(!matches!(moment, Leap::Jumped { .. }), "it jumped into nothing");
    }
    assert_eq!(jump.stage(), JumpStage::Gathering);
    let exit = at.position - DVec3::X * 200.0;
    jump.prepared(exit);
    assert!(matches!(jump.tick(TICK), Leap::Jumped { .. }));
}

#[test]
fn a_far_side_ready_at_once_still_waits_for_the_charge() {
    let mut jump = ready(target(5000.0, 100.0, 0.0));
    assert!(matches!(jump.tick(TICK), Leap::Turned { .. }));
    assert!(jump.charged() < 0.1);
    let ticks = (0..1000).position(|_| matches!(jump.tick(TICK), Leap::Jumped { .. }));
    assert!(ticks.is_some_and(|t| t > 40), "{ticks:?}");
}

#[test]
fn a_far_side_that_never_loads_ends_it() {
    let mut jump = Jump::new(start(), target(5000.0, 100.0, 0.0));
    assert!((0..3000).any(|_| jump.tick(TICK) == Leap::TimedOut));
}

#[test]
fn the_ship_swings_to_face_the_target_while_it_charges() {
    let mut jump = ready(target(5000.0, 100.0, 0.0));
    let mut last = start().rotation;
    while let Leap::Turned { pose } = jump.tick(TICK) {
        assert_eq!(pose.position, start().position);
        last = pose.rotation;
    }
    assert!(((last * DVec3::Z) - DVec3::X).length() < 1e-6, "{}", last * DVec3::Z);
}

/// To the moment the ship reappears.
fn jumped(jump: &mut Jump) -> (ShipPose, DVec3) {
    for _ in 0..2000 {
        if let Leap::Jumped { pose, velocity } = jump.tick(TICK) {
            return (pose, velocity);
        }
    }
    panic!("it never jumped");
}

#[test]
fn the_ship_reappears_at_speed() {
    let at = target(5000.0, 100.0, 0.0);
    let mut jump = ready(at);
    let (pose, velocity) = jumped(&mut jump);
    assert!(pose.position.distance(at.position) > 100.0);
    assert!(velocity.length() > 300.0, "{velocity}");
    assert!(velocity.dot(at.position - pose.position) > 0.0, "it comes in backwards");
    assert_eq!(jump.stage(), JumpStage::Arriving);
}

#[test]
fn it_brakes_hard_and_stops_exactly_on_the_target() {
    let at = target(-2600.5, 150.25, 1800.75);
    let mut jump = ready(at);
    jumped(&mut jump);
    let mut before = f64::MAX;
    for tick in 0..200 {
        match jump.tick(TICK) {
            Leap::Moved { pose, velocity } => {
                assert!(velocity.length() <= before + 1e-9, "it sped up");
                before = velocity.length();
                assert!(pose.position.distance(at.position) < 300.0);
            }
            Leap::Arrived(pose) => {
                assert_eq!(pose.position, at.position);
                assert!(tick < 40, "that was no hard stop: {tick} ticks");
                return;
            }
            other => panic!("{other:?}"),
        }
    }
    panic!("it never stopped");
}

#[test]
fn it_can_be_called_off_until_it_jumps() {
    let mut gathering = ready(target(5000.0, 100.0, 0.0));
    assert!(gathering.cancel());
    let mut gone = ready(target(5000.0, 100.0, 0.0));
    jumped(&mut gone);
    assert!(!gone.cancel());
}

#[test]
fn a_save_while_it_gathers_keeps_the_ship_where_it_is() {
    let mut jump = ready(target(5000.0, 100.0, 0.0));
    jump.tick(TICK);
    assert_eq!(jump.safe_pose().position, start().position);
}
