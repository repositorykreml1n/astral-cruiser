use astral_core::{CollisionQuery, Flight, ShipInput, ShipPose, HULL_HALF_EXTENTS};
use glam::{DQuat, DVec3};

struct Clear;
impl CollisionQuery for Clear {
    fn is_clear(&self, _: &ShipPose, _: DVec3) -> bool { true }
}
struct XWall(f64);
impl CollisionQuery for XWall {
    fn is_clear(&self, pose: &ShipPose, half: DVec3) -> bool {
        let q = pose.rotation;
        let extent = (q * DVec3::X).x.abs() * half.x
            + (q * DVec3::Y).x.abs() * half.y
            + (q * DVec3::Z).x.abs() * half.z;
        pose.position.x + extent < self.0
    }
}
fn origin() -> ShipPose {
    ShipPose { position: DVec3::ZERO, rotation: DQuat::IDENTITY }
}
#[test]
fn rotating_in_place_never_enters_wall() {
    let wall = XWall(51.0);
    let mut flight = Flight::new(origin());
    let input = ShipInput { yaw_rate: 1.0, ..Default::default() };
    for _ in 0..30 {
        let snapshot = flight.tick(&input, 0.05, &wall).unwrap();
        assert!(wall.is_clear(&snapshot.pose, HULL_HALF_EXTENTS),
            "rotating hull penetrated the wall");
    }
}
#[test]
fn moving_and_turning_keep_complete_last_clear_pose() {
    let wall = XWall(58.0);
    let mut flight = Flight::new(origin());
    let input = ShipInput { thrust: DVec3::Z, yaw_rate: 1.0, ..Default::default() };
    for _ in 0..30 {
        let snapshot = flight.tick(&input, 0.05, &wall).unwrap();
        assert!(wall.is_clear(&snapshot.pose, HULL_HALF_EXTENTS),
            "translation rejection committed an obstructed rotation");
    }
}
/// Any turn hits something, as for a long hull with a corner against a slope.
struct NoTurn;
impl CollisionQuery for NoTurn {
    fn is_clear(&self, pose: &ShipPose, _: DVec3) -> bool {
        pose.rotation.angle_between(DQuat::IDENTITY) < 1e-9
    }
}
#[test]
fn a_ship_that_cannot_turn_can_still_climb() {
    let mut flight = Flight::new(origin());
    let input = ShipInput { thrust: DVec3::Y, yaw_rate: 0.5, ..Default::default() };
    for _ in 0..20 {
        flight.tick(&input, 0.05, &NoTurn).unwrap();
    }
    let snapshot = flight.snapshot();
    // One second at 16 blocks/s²: 8 blocks up and 16 blocks/s.
    assert!((snapshot.pose.position.y - 8.0).abs() < 0.01, "{}", snapshot.pose.position);
    assert!((snapshot.velocity.y - 16.0).abs() < 1e-6, "{}", snapshot.velocity);
    assert_eq!(snapshot.pose.rotation, DQuat::IDENTITY);
}
#[test]
fn a_ship_held_against_a_wall_slides_along_it() {
    let wall = XWall(60.0);
    let mut flight = Flight::new(origin());
    let input = ShipInput { thrust: DVec3::new(1.0, 0.0, 1.0), ..Default::default() };
    for _ in 0..40 {
        let snapshot = flight.tick(&input, 0.05, &wall).unwrap();
        assert!(wall.is_clear(&snapshot.pose, HULL_HALF_EXTENTS));
    }
    let snapshot = flight.snapshot();
    assert!(snapshot.pose.position.x > 11.9 && snapshot.pose.position.x < 12.0,
        "stopped short of the wall: {}", snapshot.pose.position);
    assert_eq!(snapshot.velocity.x, 0.0);
    assert!(snapshot.pose.position.z > 10.0, "no slide: {}", snapshot.pose.position);
    assert!(snapshot.velocity.z > 0.0);
}
#[test]
fn turning_thrust_matches_across_frame_rates() {
    fn simulate(hz: usize) -> DVec3 {
        let mut flight = Flight::new(origin());
        let input = ShipInput { thrust: DVec3::Z, yaw_rate: 0.7, ..Default::default() };
        for _ in 0..hz*10 { flight.tick(&input, 1.0/hz as f64, &Clear).unwrap(); }
        flight.snapshot().pose.position
    }
    let a = simulate(30);
    let b = simulate(60);
    let c = simulate(144);
    assert!((a-b).length() <= 0.03, "30/60 Hz diverged: {}", (a-b).length());
    assert!((b-c).length() <= 0.03, "60/144 Hz diverged: {}", (b-c).length());
}
#[test]
fn finite_huge_quaternion_stays_a_unit_rotation() {
    let flight = Flight::new(ShipPose {
        position: DVec3::ZERO,
        rotation: DQuat::from_xyzw(1e308, -1e308, 1e308, 1e308),
    });
    let q = flight.snapshot().pose.rotation;
    assert!(q.is_finite());
    assert!((q.length()-1.0).abs() < 1e-9);
}
#[test]
fn finite_huge_thrust_keeps_its_direction_and_bounded_acceleration() {
    let mut flight = Flight::new(origin());
    let input = ShipInput { thrust: DVec3::new(1e308, 0.0, 0.0), ..Default::default() };
    let snapshot = flight.tick(&input, 0.05, &Clear).unwrap();
    assert!(snapshot.velocity.is_finite());
    assert!(snapshot.velocity.x > 0.0, "finite input overflow erased the thrust direction");
    assert!(snapshot.velocity.length() <= 0.800001);
}
