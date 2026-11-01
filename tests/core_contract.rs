use astral_core::{
    CollisionQuery, Dimension, Flight, FlightError, NavigationLimits, NavigationTarget,
    ShipInput, ShipPose, ShipSnapshot, BOOST_SPEED, CRUISE_SPEED, LINEAR_ACCELERATION,
};
use glam::{DQuat, DVec3};

struct ClearCollision;
impl CollisionQuery for ClearCollision {
    fn is_clear(&self, _pose: &ShipPose, _half_extents: DVec3) -> bool { true }
}

struct WallCollision { wall_x: f64 }
impl CollisionQuery for WallCollision {
    fn is_clear(&self, pose: &ShipPose, half_extents: DVec3) -> bool {
        assert_eq!(half_extents, DVec3::new(48.0, 18.0, 80.0));
        (pose.position.x + half_extents.x) < self.wall_x
    }
}

fn test_limits() -> NavigationLimits {
    NavigationLimits {
        min_y: -64.0, max_y: 320.0, border: 30_000_000.0,
        half_extents: DVec3::new(48.0, 18.0, 80.0),
    }
}

#[test]
fn test_constants() {
    assert_eq!(CRUISE_SPEED, 64.0);
    assert_eq!(BOOST_SPEED, 256.0);
    assert_eq!(LINEAR_ACCELERATION, 16.0);
}

#[test]
fn test_pose_world_local_roundtrip_far_coords() {
    let pose = ShipPose {
        position: DVec3::new(29_000_000.5, 128.0, -28_999_999.5),
        rotation: DQuat::from_axis_angle(DVec3::new(1.0, 2.0, 3.0).normalize(), 1.25),
    };
    let local = DVec3::new(12.34, -56.78, 90.12);
    let world = pose.local_to_world(local);
    let roundtrip = pose.world_to_local(world);
    assert!((roundtrip - local).length() <= 1e-7);
}

#[test]
fn test_pose_interpolate_endpoints() {
    let p0 = ShipPose {
        position: DVec3::new(100.0, 50.0, -200.0),
        rotation: DQuat::from_axis_angle(DVec3::Y, 0.2),
    };
    let p1 = ShipPose {
        position: DVec3::new(300.0, 80.0, 400.0),
        rotation: DQuat::from_axis_angle(DVec3::Y, 1.4),
    };
    let at_0 = ShipPose::interpolate(&p0, &p1, 0.0);
    let at_1 = ShipPose::interpolate(&p0, &p1, 1.0);
    assert!((at_0.position - p0.position).length() <= 1e-7);
    assert!((at_0.rotation.xyz() - p0.rotation.xyz()).length() <= 1e-7 && (at_0.rotation.w - p0.rotation.w).abs() <= 1e-7);
    assert!((at_1.position - p1.position).length() <= 1e-7);
    assert!((at_1.rotation.xyz() - p1.rotation.xyz()).length() <= 1e-7 && (at_1.rotation.w - p1.rotation.w).abs() <= 1e-7);
}

#[test]
fn test_pose_interpolate_shortest_path_yaw() {
    let p0 = ShipPose { position: DVec3::ZERO, rotation: DQuat::from_rotation_y((-179.0_f64).to_radians()) };
    let p1 = ShipPose { position: DVec3::ZERO, rotation: DQuat::from_rotation_y(179.0_f64.to_radians()) };
    let mid = ShipPose::interpolate(&p0, &p1, 0.5);
    assert!(mid.rotation.is_finite());
    assert!((mid.rotation.length() - 1.0).abs() <= 1e-7);
    let fwd = mid.rotation * DVec3::Z;
    assert!(fwd.z < -0.99, "slerp took long path instead of shortest 2 deg path");
}

#[test]
fn test_flight_invalid_dt_leaves_snapshot_intact() {
    let mut f = Flight::new(ShipPose { position: DVec3::new(10.0, 20.0, 30.0), rotation: DQuat::IDENTITY });
    let initial = f.snapshot();
    let input = ShipInput::default();
    for bad_dt in [0.0, -0.01, 0.15, f64::NAN, f64::INFINITY] {
        assert!(f.tick(&input, bad_dt, &ClearCollision).is_err());
        let snap = f.snapshot();
        assert_eq!((snap.ticks, snap.pose.position, snap.velocity), (initial.ticks, initial.pose.position, initial.velocity));
    }
}

#[test]
fn test_flight_invalid_input_leaves_snapshot_intact() {
    let mut f = Flight::new(ShipPose { position: DVec3::ZERO, rotation: DQuat::IDENTITY });
    let initial = f.snapshot();
    let nan_thrust = ShipInput { thrust: DVec3::new(f64::NAN, 0.0, 0.0), ..Default::default() };
    assert!(f.tick(&nan_thrust, 0.05, &ClearCollision).is_err());
    let inf_yaw = ShipInput { yaw_rate: f64::INFINITY, ..Default::default() };
    assert!(f.tick(&inf_yaw, 0.05, &ClearCollision).is_err());
    let snap = f.snapshot();
    assert_eq!((snap.ticks, snap.pose.position, snap.velocity), (initial.ticks, initial.pose.position, initial.velocity));
}

#[test]
fn test_flight_thrust_timestep_invariance() {
    let sim = |hz: usize| {
        let mut f = Flight::new(ShipPose { position: DVec3::ZERO, rotation: DQuat::IDENTITY });
        let dt = 1.0 / (hz as f64);
        let input = ShipInput { thrust: DVec3::Z, ..Default::default() };
        for _ in 0..hz {
            f.tick(&input, dt, &ClearCollision).unwrap();
        }
        (f.snapshot().pose.position, f.snapshot().velocity)
    };
    let (p30, v30) = sim(30);
    let (p60, v60) = sim(60);
    let (p144, v144) = sim(144);
    assert!((p30 - p60).length() <= 0.03);
    assert!((p60 - p144).length() <= 0.03);
    assert!((v30 - v60).length() <= 0.03);
    assert!((v60 - v144).length() <= 0.03);
}

#[test]
fn test_flight_speed_cap_and_diagonal_normalization() {
    let mut f = Flight::new(ShipPose { position: DVec3::ZERO, rotation: DQuat::IDENTITY });
    let diag = ShipInput { thrust: DVec3::new(1.0, 0.0, 1.0), ..Default::default() };
    for _ in 0..20 {
        f.tick(&diag, 0.05, &ClearCollision).unwrap();
    }
    assert!(f.snapshot().velocity.length() <= LINEAR_ACCELERATION + 0.03);

    for _ in 0..200 {
        f.tick(&ShipInput { thrust: DVec3::Z, ..Default::default() }, 0.05, &ClearCollision).unwrap();
    }
    assert!(f.snapshot().velocity.length() <= CRUISE_SPEED + 1e-6);
}

#[test]
fn test_flight_zero_input_brakes() {
    let mut f = Flight::new(ShipPose { position: DVec3::ZERO, rotation: DQuat::IDENTITY });
    for _ in 0..20 {
        f.tick(&ShipInput { thrust: DVec3::Z, ..Default::default() }, 0.05, &ClearCollision).unwrap();
    }
    let v_fast = f.snapshot().velocity.length();
    assert!(v_fast > 0.0);
    f.tick(&ShipInput::default(), 0.05, &ClearCollision).unwrap();
    assert!(f.snapshot().velocity.length() < v_fast);
}

#[test]
fn test_flight_wall_collision_rejection() {
    let mut f = Flight::new(ShipPose { position: DVec3::ZERO, rotation: DQuat::IDENTITY });
    let wall = WallCollision { wall_x: 50.0 };
    let input = ShipInput { thrust: DVec3::X, ..Default::default() };
    for _ in 0..60 {
        f.tick(&input, 0.05, &wall).unwrap();
    }
    let snap = f.snapshot();
    assert!(snap.pose.position.x + 48.0 <= 50.0);
    assert!(snap.velocity.x <= 1e-6);
}

#[test]
fn test_navigation_validate_valid_negative_coords() {
    let limits = test_limits();
    let target = NavigationTarget {
        dimension: Dimension::Overworld,
        position: DVec3::new(-100_000.0, 100.0, -200_000.0),
    };
    assert!(target.validate(&limits, &ClearCollision).is_ok());
    assert_eq!(target.position, DVec3::new(-100_000.0, 100.0, -200_000.0));
}

#[test]
fn test_navigation_validate_rejects_nan_and_infinite() {
    let limits = test_limits();
    for pos in [
        DVec3::new(f64::NAN, 100.0, 0.0),
        DVec3::new(0.0, f64::INFINITY, 0.0),
        DVec3::new(0.0, 100.0, f64::NEG_INFINITY),
    ] {
        let t = NavigationTarget { dimension: Dimension::End, position: pos };
        assert!(t.validate(&limits, &ClearCollision).is_err());
    }
}

#[test]
fn test_navigation_validate_rejects_border_and_altitude_crossing() {
    let limits = test_limits();
    let over_border = NavigationTarget {
        dimension: Dimension::Overworld,
        position: DVec3::new(30_000_000.0 - 10.0, 100.0, 0.0),
    };
    assert!(over_border.validate(&limits, &ClearCollision).is_err());

    let under_min_y = NavigationTarget {
        dimension: Dimension::Nether,
        position: DVec3::new(0.0, -60.0, 0.0),
    };
    assert!(under_min_y.validate(&limits, &ClearCollision).is_err());

    let over_max_y = NavigationTarget {
        dimension: Dimension::Nether,
        position: DVec3::new(0.0, 310.0, 0.0),
    };
    assert!(over_max_y.validate(&limits, &ClearCollision).is_err());
}

#[test]
fn test_navigation_validate_rejects_occupied_destination() {
    struct OccupiedCollision;
    impl CollisionQuery for OccupiedCollision {
        fn is_clear(&self, _pose: &ShipPose, _half: DVec3) -> bool { false }
    }
    let limits = test_limits();
    let target = NavigationTarget { dimension: Dimension::Overworld, position: DVec3::new(0.0, 100.0, 0.0) };
    assert!(target.validate(&limits, &OccupiedCollision).is_err());
}

fn top_speed(cruise: f64, boost: bool) -> f64 {
    let mut flight = Flight::new(ShipPose::new(DVec3::ZERO, DQuat::IDENTITY));
    flight.set_cruise(cruise);
    let input = ShipInput { thrust: DVec3::Z, yaw_rate: 0.0, boost };
    for _ in 0..400 {
        flight.tick(&input, 0.05, &AlwaysClearForSpeed).unwrap();
    }
    flight.snapshot().velocity.length()
}

struct AlwaysClearForSpeed;
impl CollisionQuery for AlwaysClearForSpeed {
    fn is_clear(&self, _pose: &ShipPose, _half_extents: DVec3) -> bool {
        true
    }
}

#[test]
fn the_cruise_speed_can_be_set() {
    assert!((top_speed(128.0, false) - 128.0).abs() < 1e-6);
}

#[test]
fn boost_keeps_its_share_of_the_cruise_speed() {
    assert!((top_speed(32.0, true) - 32.0 * BOOST_SPEED / CRUISE_SPEED).abs() < 1e-6);
}

#[test]
fn a_faster_ship_gets_up_to_speed_in_the_same_time() {
    let climb = |cruise: f64| {
        let mut flight = Flight::new(ShipPose::new(DVec3::ZERO, DQuat::IDENTITY));
        flight.set_cruise(cruise);
        let input = ShipInput { thrust: DVec3::Z, yaw_rate: 0.0, boost: false };
        for _ in 0..20 {
            flight.tick(&input, 0.05, &AlwaysClearForSpeed).unwrap();
        }
        flight.snapshot().velocity.length() / cruise
    };
    assert!((climb(64.0) - climb(256.0)).abs() < 1e-6);
}
