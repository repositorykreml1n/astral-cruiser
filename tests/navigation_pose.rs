use astral_core::{CollisionQuery, Dimension, NavigationLimits, NavigationTarget, ShipPose};
use glam::{DQuat, DVec3};
struct Clear;
impl CollisionQuery for Clear {
    fn is_clear(&self, _: &ShipPose, _: DVec3) -> bool { true }
}
#[test]
fn arrival_border_check_uses_rotated_hull() {
    let limits = NavigationLimits { min_y: -64.0, max_y:320.0, border:1000.0,
        half_extents:DVec3::new(48.0,18.0,80.0) };
    let target = NavigationTarget { dimension:Dimension::Overworld,
        position:DVec3::new(940.0,100.0,0.0) };
    assert!(target.validate_pose(&limits, DQuat::IDENTITY, &Clear).is_ok());
    assert!(target.validate_pose(&limits, DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2), &Clear).is_err());
}
#[test]
fn arrival_collision_receives_actual_rotation() {
    struct OrientationCheck;
    impl CollisionQuery for OrientationCheck {
        fn is_clear(&self, pose:&ShipPose, _:DVec3)->bool {
            (pose.rotation * DVec3::Z).x.abs() < 0.5
        }
    }
    let limits = NavigationLimits { min_y:-64.0, max_y:320.0, border:1000.0,
        half_extents:DVec3::new(48.0,18.0,80.0) };
    let target = NavigationTarget { dimension:Dimension::End, position:DVec3::new(0.0,100.0,0.0) };
    assert!(target.validate_pose(&limits,DQuat::IDENTITY,&OrientationCheck).is_ok());
    assert!(target.validate_pose(&limits,DQuat::from_rotation_y(1.0),&OrientationCheck).is_err());
}
