#[path = "../mixins/astral_cruiser/core.rs"]
mod astral_core;
#[path = "../mixins/astral_cruiser/dash.rs"]
mod astral_dash;

use astral_core::{CollisionQuery, Dimension, NavigationLimits, NavigationTarget, ShipPose};
use astral_dash::{
    areas_in_flight, dash_shows_far_terrain, far_reach, fog_reach,
    Dash, DashError, Moment, Route, SHORT_DASH, Stage, Travel, chunks_of, coordinate, dash_areas, facing,
    lowest_fit, plan, within_world,
};
use glam::{DQuat, DVec3};

const TICK: f64 = 0.05;
const HALF: DVec3 = DVec3::new(10.0, 10.0, 10.0);
const LIMITS: NavigationLimits = NavigationLimits {
    min_y: -64.0,
    max_y: 320.0,
    border: 1000.0,
    half_extents: HALF,
};

/// Open air, with solid boxes standing in it: `[x, y, z, x, y, z]`.
struct Blocks(Vec<[f64; 6]>);

impl CollisionQuery for Blocks {
    fn is_clear(&self, pose: &ShipPose, half: DVec3) -> bool {
        let (low, high) = (pose.position - half, pose.position + half);
        !self.0.iter().any(|b| {
            low.x < b[3] && high.x > b[0] && low.y < b[4] && high.y > b[1] && low.z < b[5] && high.z > b[2]
        })
    }
}

fn open() -> Blocks {
    Blocks(Vec::new())
}

fn pose(x: f64, y: f64, z: f64) -> ShipPose {
    ShipPose::new(DVec3::new(x, y, z), DQuat::IDENTITY)
}

fn target(x: f64, y: f64, z: f64) -> NavigationTarget {
    NavigationTarget { dimension: Dimension::Overworld, position: DVec3::new(x, y, z) }
}

/// Everything a route's ship passes, a tick apart.
fn fly(route: Route) -> Vec<DVec3> {
    let mut travel = Travel::new(route);
    let mut passed = Vec::new();
    for _ in 0..100_000 {
        travel.tick(TICK);
        passed.push(travel.position());
        if travel.finished() {
            return passed;
        }
    }
    panic!("the ship never got there");
}

#[test]
fn a_direct_route_ends_exactly_on_the_target() {
    let to = DVec3::new(123.456, 78.9, -321.75);
    let passed = fly(Route::direct(DVec3::new(1.0, 80.0, 2.0), to));
    assert_eq!(*passed.last().unwrap(), to);
}

#[test]
fn a_route_over_the_world_stays_on_its_three_legs() {
    let (from, to) = (DVec3::new(0.0, 100.0, 0.0), DVec3::new(-900.0, 80.0, 700.0));
    let passed = fly(Route::over_the_world(from, to, 350.0));
    assert_eq!(*passed.last().unwrap(), to);
    assert!(passed.iter().any(|p| (p.y - 350.0).abs() < 1e-6));
    for p in &passed {
        let over_start = p.x.abs() < 1e-6 && p.z.abs() < 1e-6;
        let over_target = (p.x + 900.0).abs() < 1e-6 && (p.z - 700.0).abs() < 1e-6;
        assert!(over_start || over_target || (p.y - 350.0).abs() < 1e-6, "{p}");
    }
}

#[test]
fn a_route_starting_at_its_height_has_no_climb() {
    let from = DVec3::new(0.0, 350.0, 0.0);
    let route = Route::over_the_world(from, DVec3::new(200.0, 80.0, 0.0), 350.0);
    assert_eq!(route.legs().len(), 2);
}

#[test]
fn a_route_climbs_past_its_height_for_a_target_above_it() {
    let (from, to) = (DVec3::new(0.0, 100.0, 0.0), DVec3::new(500.0, 400.0, 0.0));
    let passed = fly(Route::over_the_world(from, to, 350.0));
    assert_eq!(*passed.last().unwrap(), to);
    assert!(passed.iter().all(|p| p.y <= 400.0 + 1e-6));
}

#[test]
fn no_way_to_go_is_over_at_once() {
    let at = DVec3::new(5.0, 6.0, 7.0);
    let mut travel = Travel::new(Route::direct(at, at));
    travel.tick(TICK);
    assert!(travel.finished());
    assert_eq!(travel.position(), at);
}

#[test]
fn the_ship_gathers_speed_and_loses_it_on_each_leg() {
    let route = Route::direct(DVec3::ZERO, DVec3::new(5000.0, 0.0, 0.0));
    let top = route.legs()[0].pace.top;
    let rate = route.legs()[0].pace.acceleration;
    let mut travel = Travel::new(route);
    let mut before = 0.0;
    let mut fastest: f64 = 0.0;
    loop {
        travel.tick(TICK);
        if travel.finished() {
            break;
        }
        let speed = travel.velocity().length();
        assert!(speed <= top + 1e-9, "{speed}");
        assert!((speed - before).abs() <= rate * TICK * 1.5 + 2.0, "{before} {speed}");
        fastest = fastest.max(speed);
        before = speed;
    }
    assert!(fastest > top * 0.99, "{fastest}");
    assert!(before < 40.0, "it is still going fast at the end: {before}");
    assert_eq!(travel.velocity(), DVec3::ZERO);
}

#[test]
fn braking_stops_the_ship_on_its_route_short_of_the_target() {
    let to = DVec3::new(5000.0, 0.0, 0.0);
    let mut travel = Travel::new(Route::direct(DVec3::ZERO, to));
    for _ in 0..100 {
        travel.tick(TICK);
    }
    let braked_at = travel.position().x;
    travel.brake();
    for _ in 0..2000 {
        travel.tick(TICK);
    }
    assert!(travel.finished());
    assert_eq!(travel.velocity(), DVec3::ZERO);
    assert!(travel.position().x > braked_at && travel.position().x < to.x, "{}", travel.position());
    assert_eq!(travel.position().y, 0.0);
}

#[test]
fn braking_on_a_climb_ends_the_trip_before_the_cross() {
    let route = Route::over_the_world(DVec3::ZERO, DVec3::new(3000.0, 0.0, 0.0), 2000.0);
    let mut travel = Travel::new(route);
    for _ in 0..40 {
        travel.tick(TICK);
    }
    travel.brake();
    while !travel.finished() {
        travel.tick(TICK);
    }
    assert_eq!(travel.position().x, 0.0);
    assert!(travel.position().y > 0.0 && travel.position().y < 2000.0);
}

#[test]
fn distance_left_falls_to_nothing() {
    let route = Route::direct(DVec3::ZERO, DVec3::new(300.0, 0.0, 400.0));
    let mut travel = Travel::new(route);
    assert!((travel.left() - 500.0).abs() < 1e-9);
    while !travel.finished() {
        travel.tick(TICK);
    }
    assert_eq!(travel.left(), 0.0);
}

#[test]
fn the_ship_faces_where_it_goes() {
    let from = pose(10.0, 80.0, 10.0);
    let rotation = facing(&from, DVec3::new(110.0, 300.0, 10.0));
    let forward = rotation * DVec3::Z;
    assert!((forward - DVec3::X).length() < 1e-9, "{forward}");
}

#[test]
fn straight_up_keeps_the_ship_as_it_stands() {
    let from = ShipPose::new(DVec3::ZERO, DQuat::from_rotation_y(1.0));
    assert_eq!(facing(&from, DVec3::new(0.2, 100.0, 0.2)), from.rotation);
}

#[test]
fn a_clear_line_goes_direct() {
    let route = plan(&pose(0.0, 100.0, 0.0), &target(300.0, 100.0, -50.0), &LIMITS, Some(350.0), &open());
    assert_eq!(route.unwrap().legs().len(), 1);
}

#[test]
fn a_mountain_in_the_way_is_flown_over() {
    let terrain = Blocks(vec![[100.0, -64.0, -500.0, 140.0, 300.0, 500.0]]);
    let route = plan(&pose(0.0, 100.0, 0.0), &target(300.0, 100.0, 0.0), &LIMITS, Some(350.0), &terrain);
    assert_eq!(route.unwrap().legs().len(), 3);
}

#[test]
fn with_no_sky_a_blocked_line_is_refused() {
    let terrain = Blocks(vec![[100.0, -64.0, -500.0, 140.0, 300.0, 500.0]]);
    let route = plan(&pose(0.0, 100.0, 0.0), &target(300.0, 100.0, 0.0), &LIMITS, None, &terrain);
    assert_eq!(route, Err(DashError::Blocked));
}

#[test]
fn a_roof_over_the_target_is_refused_rather_than_broken_through() {
    let terrain = Blocks(vec![
        [100.0, -64.0, -500.0, 140.0, 300.0, 500.0],
        [250.0, 150.0, -50.0, 350.0, 160.0, 50.0],
    ]);
    let route = plan(&pose(0.0, 100.0, 0.0), &target(300.0, 100.0, 0.0), &LIMITS, Some(350.0), &terrain);
    assert_eq!(route, Err(DashError::Blocked));
}

#[test]
fn a_roof_over_the_ship_is_refused_too() {
    let terrain = Blocks(vec![
        [100.0, -64.0, -500.0, 140.0, 300.0, 500.0],
        [-50.0, 150.0, -50.0, 50.0, 160.0, 50.0],
    ]);
    let route = plan(&pose(0.0, 100.0, 0.0), &target(300.0, 100.0, 0.0), &LIMITS, Some(350.0), &terrain);
    assert_eq!(route, Err(DashError::Blocked));
}

#[test]
fn an_occupied_target_is_refused_not_moved() {
    let terrain = Blocks(vec![[-50.0, 0.0, 250.0, 50.0, 95.0, 350.0]]);
    let route = plan(&pose(0.0, 200.0, 0.0), &target(0.0, 90.0, 300.0), &LIMITS, Some(350.0), &terrain);
    assert_eq!(route, Err(DashError::Occupied { lowest: Some(106.0) }));
}

#[test]
fn a_target_past_the_edges_of_the_world_is_refused() {
    let plan_for = |at: NavigationTarget| plan(&pose(0.0, 100.0, 0.0), &at, &LIMITS, Some(350.0), &open());
    for at in [
        target(2000.0, 100.0, 0.0),
        target(0.0, 100.0, -2000.0),
        target(0.0, -80.0, 0.0),
        target(0.0, 330.0, 0.0),
    ] {
        assert!(matches!(plan_for(at), Err(DashError::Outside(_))), "{at:?}");
    }
}

#[test]
fn a_target_that_is_no_number_is_refused() {
    for at in [target(f64::NAN, 100.0, 0.0), target(0.0, f64::INFINITY, 0.0)] {
        let route = plan(&pose(0.0, 100.0, 0.0), &at, &LIMITS, Some(350.0), &open());
        assert!(matches!(route, Err(DashError::Outside(_))), "{at:?}");
    }
}

#[test]
fn a_ship_with_no_room_to_turn_is_told_so() {
    // Hemmed in along z: a long hull fits, but cannot swing round to face x.
    let terrain = Blocks(vec![
        [-500.0, 0.0, 16.0, 500.0, 300.0, 40.0],
        [-500.0, 0.0, -40.0, 500.0, 300.0, -16.0],
    ]);
    struct Long<'a>(&'a Blocks);
    impl CollisionQuery for Long<'_> {
        fn is_clear(&self, pose: &ShipPose, _half: DVec3) -> bool {
            let along = pose.rotation * DVec3::new(0.0, 0.0, 30.0);
            (0..=6).all(|i| {
                let bit = ShipPose::new(pose.position + along * (f64::from(i) / 3.0 - 1.0), DQuat::IDENTITY);
                self.0.is_clear(&bit, DVec3::splat(2.0))
            })
        }
    }
    let from = ShipPose::new(DVec3::new(0.0, 100.0, 0.0), DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2));
    let route = plan(&from, &target(0.0, 100.0, 200.0), &LIMITS, Some(350.0), &Long(&terrain));
    assert_eq!(route, Err(DashError::NoRoomToTurn));
}

#[test]
fn the_world_check_needs_no_world() {
    let from = pose(0.0, 100.0, 0.0);
    assert_eq!(within_world(&from, &target(10.0, 100.0, 0.0), &LIMITS, true), Ok(()));
    assert!(within_world(&from, &target(5000.0, 100.0, 0.0), &LIMITS, true).is_err());
}

#[test]
fn without_a_sky_to_cross_in_a_dash_has_a_limited_reach() {
    let from = pose(0.0, 100.0, 0.0);
    let limits = NavigationLimits { border: 100_000.0, ..LIMITS };
    let far = target(20_000.0, 100.0, 0.0);
    assert_eq!(within_world(&from, &far, &limits, true), Ok(()));
    assert_eq!(within_world(&from, &far, &limits, false), Err(DashError::TooFar));
}

#[test]
fn with_a_sky_only_the_two_ends_are_loaded() {
    let (from, to) = (DVec3::new(0.0, 100.0, 0.0), DVec3::new(-3200.0, 80.0, 1600.0));
    assert_eq!(dash_areas(from, to, 7, true), vec![((0, 0), 7), ((-200, 100), 7)]);
}

#[test]
fn without_a_sky_the_whole_way_is_loaded() {
    let (from, to) = (DVec3::new(0.0, 60.0, 0.0), DVec3::new(320.0, 60.0, 0.0));
    let areas = dash_areas(from, to, 7, false);
    for x in (0..=20).map(|c| c * 16) {
        let (cx, cz) = (x >> 4, 0);
        assert!(
            areas.iter().any(|&((ax, az), r)| (cx - ax).abs() <= r && (cz - az).abs() <= r),
            "chunk {cx} is left out of {areas:?}"
        );
    }
}

#[test]
fn a_short_hop_loads_one_area() {
    let (from, to) = (DVec3::new(5.0, 60.0, 5.0), DVec3::new(6.0, 90.0, 7.0));
    assert_eq!(dash_areas(from, to, 7, true).len(), 1);
}

#[test]
fn the_lowest_fit_is_the_first_clear_height_above() {
    let terrain = Blocks(vec![[-50.0, 0.0, -50.0, 50.0, 95.0, 50.0]]);
    assert_eq!(lowest_fit(&terrain, DVec3::new(0.0, 90.0, 0.0), DQuat::IDENTITY, HALF, 320.0), Some(106.0));
}

#[test]
fn no_height_fits_under_a_solid_column() {
    let terrain = Blocks(vec![[-50.0, 0.0, -50.0, 50.0, 400.0, 50.0]]);
    assert_eq!(lowest_fit(&terrain, DVec3::new(0.0, 90.0, 0.0), DQuat::IDENTITY, HALF, 320.0), None);
}

#[test]
fn a_dash_holds_the_ship_until_its_place_has_loaded() {
    let start = pose(0.0, 100.0, 0.0);
    let mut dash = Dash::new(start, target(300.0, 100.0, 0.0));
    assert!(matches!(dash.tick(TICK), Moment::Held));
    assert_eq!(dash.stage(), Stage::Warming);
}

#[test]
fn a_place_that_never_loads_ends_the_dash() {
    let mut dash = Dash::new(pose(0.0, 100.0, 0.0), target(300.0, 100.0, 0.0));
    let moments: Vec<Moment> = (0..2000).map(|_| dash.tick(TICK)).collect();
    assert!(moments.contains(&Moment::TimedOut));
}

#[test]
fn once_loaded_the_ship_turns_while_it_charges_and_then_goes() {
    let start = pose(0.0, 100.0, 0.0);
    let at = target(0.0, 100.0, 0.0).position + DVec3::new(300.0, 0.0, 0.0);
    let mut dash = Dash::new(start, NavigationTarget { dimension: Dimension::Overworld, position: at });
    dash.warmed(Route::direct(start.position, at));
    assert_eq!(dash.stage(), Stage::Charging);
    let mut turned = start.rotation;
    let mut flown = Vec::new();
    for _ in 0..4000 {
        match dash.tick(TICK) {
            Moment::Moved { pose, .. } if dash.stage() == Stage::Charging => {
                assert_eq!(pose.position, start.position);
                turned = pose.rotation;
            }
            Moment::Moved { pose, .. } => flown.push(pose.position),
            Moment::Arrived(pose) => {
                assert_eq!(pose.position, at);
                assert!(((pose.rotation * DVec3::Z) - DVec3::X).length() < 1e-9);
                assert!(!flown.is_empty());
                assert!(((turned * DVec3::Z) - DVec3::X).length() < 1e-6);
                return;
            }
            other => panic!("{other:?}"),
        }
    }
    panic!("the dash never ended");
}

#[test]
fn cancelling_before_the_flight_is_over_at_once() {
    let start = pose(0.0, 100.0, 0.0);
    let mut warming = Dash::new(start, target(300.0, 100.0, 0.0));
    assert!(warming.cancel());
    let mut charging = Dash::new(start, target(300.0, 100.0, 0.0));
    charging.warmed(Route::direct(start.position, DVec3::new(300.0, 100.0, 0.0)));
    assert!(charging.cancel());
}

#[test]
fn cancelling_in_flight_brakes_and_stops_short() {
    let start = pose(0.0, 100.0, 0.0);
    let at = DVec3::new(5000.0, 100.0, 0.0);
    let mut dash = Dash::new(start, NavigationTarget { dimension: Dimension::Overworld, position: at });
    dash.warmed(Route::direct(start.position, at));
    for _ in 0..120 {
        dash.tick(TICK);
    }
    assert_eq!(dash.stage(), Stage::Flying);
    assert!(!dash.cancel());
    assert_eq!(dash.stage(), Stage::Braking);
    for _ in 0..4000 {
        match dash.tick(TICK) {
            Moment::Moved { .. } => {}
            Moment::Stopped(pose) => {
                assert!(pose.position.x > 0.0 && pose.position.x < at.x);
                return;
            }
            other => panic!("{other:?}"),
        }
    }
    panic!("the ship never stopped");
}

#[test]
fn a_coordinate_is_a_number_or_counts_from_the_ship() {
    assert_eq!(coordinate("-120.5", 10.0), Some(-120.5));
    assert_eq!(coordinate("~", 10.0), Some(10.0));
    assert_eq!(coordinate("~25", 10.0), Some(35.0));
    assert_eq!(coordinate("~-25", 10.0), Some(-15.0));
    for word in ["", "x", "nan", "inf", "-inf", "~nan", "~x", "1e999"] {
        assert_eq!(coordinate(word, 10.0), None, "{word}");
    }
}

#[test]
fn the_chunks_of_an_area_are_a_square_round_its_middle() {
    let chunks = chunks_of(((-3, 7), 1));
    assert_eq!(chunks.len(), 9);
    assert!(chunks.contains(&(-4, 6)) && chunks.contains(&(-2, 8)) && chunks.contains(&(-3, 7)));
}

#[test]
fn a_route_set_to_a_speed_flies_only_its_fast_legs_at_it() {
    let (from, to) = (DVec3::new(0.0, 100.0, 0.0), DVec3::new(900.0, 80.0, 0.0));
    let plain = Route::over_the_world(from, to, 350.0);
    let quick = plain.clone().at_speed(2000.0);
    assert_eq!(quick.legs()[1].pace.top, 2000.0);
    assert!(quick.legs()[1].pace.acceleration > plain.legs()[1].pace.acceleration);
    assert_eq!(quick.legs()[0].pace, plain.legs()[0].pace);
    assert_eq!(quick.legs()[2].pace, plain.legs()[2].pace);
}

#[test]
fn a_ship_put_on_a_route_at_speed_is_going_from_the_first_tick() {
    let route = Route::paced(
        DVec3::ZERO,
        DVec3::new(200.0, 0.0, 0.0),
        astral_dash::Pace { top: 500.0, acceleration: 625.0 },
    );
    let mut travel = Travel::at_speed(route, 500.0);
    travel.tick(TICK);
    assert!(travel.velocity().length() > 450.0, "{}", travel.velocity());
    let mut ticks = 1;
    while !travel.finished() {
        travel.tick(TICK);
        ticks += 1;
        assert!(ticks < 100, "it never stopped");
    }
    assert_eq!(travel.position(), DVec3::new(200.0, 0.0, 0.0));
}

#[test]
fn in_flight_only_the_arrival_stays_loaded() {
    let areas = [((0, 0), 6), ((100, 0), 6)];
    assert_eq!(areas_in_flight(&areas, true), &areas[1..]);
    assert_eq!(areas_in_flight(&areas, false), &areas[..]);
}

#[test]
fn over_the_world_the_fog_stands_past_the_ground_below() {
    assert!(fog_reach(5, true) * 16 > 400);
    assert_eq!(fog_reach(5, false), 5);
    assert_eq!(fog_reach(64, true), 64);
}

#[test]
fn far_terrain_is_a_window_round_the_ship_and_ahead_of_it() {
    assert_eq!(far_reach(0.0), (0.0, 5));
    assert_eq!(far_reach(800.0), (140.0, 14));
    for speed in [0.0, 800.0, 2000.0] {
        let (middle, radius) = far_reach(speed);
        let back_edge = f64::from(radius * 16) - middle;
        let front_edge = f64::from(radius * 16) + middle;
        assert!(back_edge >= 80.0 && back_edge <= 112.0, "back edge {back_edge} at {speed}");
        assert!(front_edge >= speed * 0.35, "front edge {front_edge} at {speed}");
    }
}

#[test]
fn short_dashes_have_no_far_terrain() {
    assert!(!dash_shows_far_terrain(SHORT_DASH));
    assert!(!dash_shows_far_terrain(SHORT_DASH - 1.0));
    assert!(!dash_shows_far_terrain(0.0));
    assert!(!dash_shows_far_terrain(-100.0));
    assert!(dash_shows_far_terrain(SHORT_DASH + 0.1));
    assert!(dash_shows_far_terrain(5000.0));
}

