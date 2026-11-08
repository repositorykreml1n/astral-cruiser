#[path = "../mixins/astral_cruiser/lance.rs"]
mod astral_lance;

use astral_lance::{Lance, LANCE_DAMAGE, LANCE_DEPTH, LANCE_RADIUS, LANCE_SPEED, LANCE_WIDTH};
use glam::DVec3;

#[test]
fn follow_moves_at_most_twenty_a_second() {
    assert_eq!(LANCE_SPEED, 20.0);
    assert_eq!(LANCE_WIDTH, 4.0);
    assert_eq!(LANCE_DEPTH, 8.0);
    assert_eq!(LANCE_DAMAGE, 30.0);
    assert_eq!(LANCE_RADIUS, 3.0);

    for aim in [DVec3::new(100.0, 64.0, 0.0), DVec3::new(60.0, 64.0, 80.0)] {
        let mut lance = Lance::start(DVec3::new(0.0, 64.0, 0.0));
        lance.follow(aim, 0.05);
        let distance = lance.at.x.hypot(lance.at.z);
        assert!((distance - 1.0).abs() < 1e-9);
        assert!((lance.at.x - aim.x / 100.0).abs() < 1e-9);
        assert!((lance.at.z - aim.z / 100.0).abs() < 1e-9);
    }
}

#[test]
fn follow_reaches_a_near_aim() {
    let mut lance = Lance::start(DVec3::new(0.0, 64.0, 0.0));
    let aim = DVec3::new(0.5, 64.0, 0.0);
    lance.follow(aim, 0.05);
    assert_eq!(lance.at, aim);
    lance.follow(aim, 0.05);
    assert_eq!(lance.at, aim);
}

#[test]
fn follow_returns_the_swept_segment() {
    let start = DVec3::new(5.0, 64.0, 7.0);
    let mut lance = Lance::start(start);
    assert_eq!(lance.at, start);
    let (from, to) = lance.follow(DVec3::new(105.0, 72.0, 7.0), 0.05);
    assert_eq!(from, start);
    assert_eq!(to, DVec3::new(6.0, 72.0, 7.0));
    assert_eq!(to, lance.at);

    let (next_from, next_to) = lance.follow(DVec3::new(6.0, 72.0, 107.0), 0.05);
    assert_eq!(next_from, to);
    assert_eq!(next_to, DVec3::new(6.0, 72.0, 8.0));
    assert_eq!(next_to, lance.at);
}

#[test]
fn follow_takes_height_from_aim() {
    let mut lance = Lance::start(DVec3::new(0.0, 64.0, 0.0));
    lance.follow(DVec3::new(100.0, 128.0, 0.0), 0.05);
    assert_eq!(lance.at, DVec3::new(1.0, 128.0, 0.0));

    let aim = DVec3::new(1.0, -32.0, 0.0);
    lance.follow(aim, 0.05);
    assert_eq!(lance.at, aim);
}
