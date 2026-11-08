#[path = "../mixins/astral_cruiser/shockwave.rs"]
mod astral_shockwave;

use astral_shockwave::*;
use glam::DVec3;

#[test]
fn front_moves_at_speed() {
    let wave = Wave {
        center: DVec3::ZERO,
        start: 10.0,
        radius: 500.0,
        strength: 1.0,
        damaging: true,
    };

    assert_eq!(wave.front(9.0), 0.0);
    assert_eq!(wave.front(10.0), 0.0);
    assert!((wave.front(11.0) - SPEED).abs() < 1e-9);
    assert!((wave.front(11.2) - 1.2 * SPEED).abs() < 1e-9);
    assert_eq!(wave.front(11.5), 500.0); // capped at radius
    assert_eq!(wave.front(12.0), 500.0);
    assert_eq!(wave.front(20.0), 500.0);
}

#[test]
fn falloff_zero_at_radius() {
    let wave = Wave {
        center: DVec3::ZERO,
        start: 0.0,
        radius: 100.0,
        strength: 1.0,
        damaging: false,
    };

    assert_eq!(wave.falloff(0.0), 1.0);
    assert!((wave.falloff(50.0) - (0.5_f64).powf(1.5)).abs() < 1e-9);
    assert_eq!(wave.falloff(100.0), 0.0);
    assert_eq!(wave.falloff(150.0), 0.0);
}

#[test]
fn arrival_none_beyond_radius() {
    let wave = Wave {
        center: DVec3::new(10.0, 20.0, 30.0),
        start: 5.0,
        radius: 100.0,
        strength: 0.8,
        damaging: false,
    };

    let p_in = DVec3::new(10.0 + 34.0, 20.0, 30.0);
    assert_eq!(wave.arrival(p_in), Some(5.0 + 34.0 / SPEED));

    let p_edge = DVec3::new(10.0 + 100.0, 20.0, 30.0);
    assert_eq!(wave.arrival(p_edge), Some(5.0 + 100.0 / SPEED));

    let p_out = DVec3::new(10.0 + 101.0, 20.0, 30.0);
    assert_eq!(wave.arrival(p_out), None);
}

#[test]
fn crossed_once() {
    let wave = Wave {
        center: DVec3::ZERO,
        start: 1.0,
        radius: 300.0,
        strength: 1.0,
        damaging: true,
    };

    // Arrival time: 1.0 + 170.0 / 340.0 = 1.50
    let target = DVec3::new(170.0, 0.0, 0.0);
    let dt = 0.05;
    let mut crossed_count = 0;

    let mut now = 0.0;
    while now <= 3.0 {
        let before = now - dt;
        if wave.crossed(target, before, now) {
            crossed_count += 1;
        }
        now += dt;
    }

    assert_eq!(crossed_count, 1);
}

#[test]
fn shake_sum_is_capped() {
    let at = DVec3::new(0.0, 64.0, 0.0);
    let now = 2.0;
    // Three waves that all arrive at or before `now` at listener's position with full strength
    let waves = [
        Wave {
            center: at,
            start: 2.0,
            radius: 100.0,
            strength: 1.0,
            damaging: true,
        },
        Wave {
            center: at,
            start: 2.0,
            radius: 100.0,
            strength: 1.0,
            damaging: true,
        },
        Wave {
            center: at,
            start: 2.0,
            radius: 100.0,
            strength: 1.0,
            damaging: true,
        },
    ];

    let total = shake_total(&waves, at, now);
    assert!(total <= SHAKE_CAP);
    assert_eq!(total, SHAKE_CAP);
}

#[test]
fn no_shake_beyond_radius() {
    let wave = Wave {
        center: DVec3::ZERO,
        start: 0.0,
        radius: 50.0,
        strength: 1.0,
        damaging: true,
    };
    let outside = DVec3::new(60.0, 0.0, 0.0);
    assert_eq!(wave.shake_at(outside, 1.0), 0.0);
    assert_eq!(shake_total(&[wave], outside, 1.0), 0.0);
}

#[test]
fn shake_offset_bounded() {
    // Sample 1000 times across various timestamps and shake amounts
    for i in 0..1000 {
        let now = i as f64 * 0.037 + 0.123;
        let amount = ((i % 25) as f64) * 0.1; // 0.0 to 2.4 (some exceeding SHAKE_CAP)
        let effective_amt = amount.clamp(0.0, SHAKE_CAP);

        let (pos, yaw, pitch) = shake_offset(amount, now);

        let max_pos = 0.6 * effective_amt + 1e-9;
        assert!(
            pos.x.abs() <= max_pos,
            "pos.x {} exceeds {}",
            pos.x.abs(),
            max_pos
        );
        assert!(
            pos.y.abs() <= max_pos,
            "pos.y {} exceeds {}",
            pos.y.abs(),
            max_pos
        );
        assert!(
            pos.z.abs() <= max_pos,
            "pos.z {} exceeds {}",
            pos.z.abs(),
            max_pos
        );

        let max_angle = (2.5 * effective_amt) as f32 + 1e-4;
        assert!(
            yaw.abs() <= max_angle,
            "yaw {} exceeds {}",
            yaw.abs(),
            max_angle
        );
        assert!(
            pitch.abs() <= max_angle,
            "pitch {} exceeds {}",
            pitch.abs(),
            max_angle
        );
    }
}

#[test]
fn ship_push_points_away() {
    let wave = Wave {
        center: DVec3::new(10.0, 0.0, 10.0),
        start: 0.0,
        radius: 100.0,
        strength: 0.5,
        damaging: false,
    };

    // Ship at (40, 0, 50): dx = 30, dz = 40, dist = 50
    let ship = DVec3::new(40.0, 0.0, 50.0);
    let push = ship_push(&wave, ship);
    assert!(push.x > 0.0);
    assert!(push.z > 0.0);
    assert!(push.y > 0.0);
    let fo = wave.falloff(50.0);
    let expected_h = 8.0 * 0.5 * fo;
    let expected_y = 3.0 * 0.5 * fo;
    assert!((push.x - 0.6 * expected_h).abs() < 1e-9);
    assert!((push.z - 0.8 * expected_h).abs() < 1e-9);
    assert!((push.y - expected_y).abs() < 1e-9);

    // Ship directly above center: pushes +x
    let ship_above = DVec3::new(10.0, 40.0, 10.0);
    let push_above = ship_push(&wave, ship_above);
    let fo_above = wave.falloff(40.0);
    assert!((push_above.x - 8.0 * 0.5 * fo_above).abs() < 1e-9);
    assert_eq!(push_above.z, 0.0);
    assert!((push_above.y - 3.0 * 0.5 * fo_above).abs() < 1e-9);

    // Ship beyond radius: zero push
    let ship_far = DVec3::new(200.0, 0.0, 0.0);
    assert_eq!(ship_push(&wave, ship_far), DVec3::ZERO);
}

#[test]
fn done_after_linger() {
    let wave = Wave {
        center: DVec3::ZERO,
        start: 2.0,
        radius: 340.0,
        strength: 1.0,
        damaging: true,
    };
    // Front reaches radius at start + radius / SPEED = 2.0 + 1.0 = 3.0.
    // Linger is 1.5 s, so done at 4.5.
    assert!(!wave.done(2.0));
    assert!(!wave.done(3.0));
    assert!(!wave.done(4.49));
    assert!(wave.done(4.5));
    assert!(wave.done(5.0));
}
