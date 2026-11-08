#[path = "../mixins/astral_cruiser/strike_math.rs"]
mod astral_strike_math;

use astral_strike_math::{falloff_damage, fire_spots, knockback, surface_y};
use glam::DVec3;

#[test]
fn falloff_is_full_without_falloff() {
    let amount = 300.0;
    let radius = 32.0;
    assert_eq!(falloff_damage(amount, radius, 0.0, false), 300.0);
    assert_eq!(falloff_damage(amount, radius, 16.0, false), 300.0);
    assert_eq!(falloff_damage(amount, radius, 32.0, false), 300.0);
}

#[test]
fn falloff_falls_linearly_to_the_edge() {
    let amount = 300.0;
    let radius = 32.0;
    assert_eq!(falloff_damage(amount, radius, 0.0, true), 300.0);
    assert_eq!(falloff_damage(amount, radius, 16.0, true), 150.0);
    assert_eq!(falloff_damage(amount, radius, 32.0, true), 0.0);
}

#[test]
fn nothing_beyond_the_radius() {
    let amount = 300.0;
    let radius = 32.0;
    assert_eq!(falloff_damage(amount, radius, 33.0, true), 0.0);
    assert_eq!(falloff_damage(amount, radius, 33.0, false), 0.0);
}

#[test]
fn knockback_points_away_and_up() {
    let at = DVec3::new(10.0, 64.0, 20.0);
    let radius = 32.0;
    let strength = 10.0;
    let target = DVec3::new(18.0, 64.0, 26.0);
    let kb = knockback(at, radius, strength, target);
    assert!(kb.x > 0.0);
    assert!(kb.z > 0.0);
    assert!(kb.y > 0.0);
    let scale = strength * (1.0 - 10.0 / 32.0);
    assert!((kb.x - 0.8 * scale * 1.5).abs() < 1e-9);
    assert!((kb.z - 0.6 * scale * 1.5).abs() < 1e-9);
    assert!((kb.y - scale * 0.6).abs() < 1e-9);
}

#[test]
fn knockback_is_zero_beyond_radius() {
    let at = DVec3::new(0.0, 64.0, 0.0);
    let radius = 32.0;
    let strength = 10.0;
    let target = DVec3::new(33.0, 64.0, 0.0);
    assert_eq!(knockback(at, radius, strength, target), DVec3::ZERO);
    let target_up = DVec3::new(0.0, 100.0, 0.0);
    assert_eq!(knockback(at, radius, strength, target_up), DVec3::ZERO);
}

#[test]
fn knockback_straight_above_still_pushes() {
    let at = DVec3::new(5.0, 64.0, 5.0);
    let radius = 32.0;
    let strength = 10.0;
    let target = DVec3::new(5.0, 80.0, 5.0);
    let kb = knockback(at, radius, strength, target);
    let scale = strength * (1.0 - 16.0 / 32.0);
    assert!(kb.x > 0.0);
    assert_eq!(kb.z, 0.0);
    assert!(kb.y > 0.0);
    assert!((kb.x - scale * 1.5).abs() < 1e-9);
    assert!((kb.y - scale * 0.6).abs() < 1e-9);

    let kb_at = knockback(at, radius, strength, at);
    assert!((kb_at.x - strength * 1.5).abs() < 1e-9);
    assert_eq!(kb_at.z, 0.0);
    assert!((kb_at.y - strength * 0.6).abs() < 1e-9);
}

#[test]
fn fire_spots_lie_in_the_outer_ring() {
    let at = DVec3::new(100.0, 64.0, -50.0);
    let radius = 32.0;
    let spots = fire_spots(at, radius, 12345, 10);
    assert_eq!(spots.len(), 10);
    for &(x, z) in &spots {
        let cx = x as f64 + 0.5;
        let cz = z as f64 + 0.5;
        let d = (cx - at.x).hypot(cz - at.z);
        assert!(d >= 0.75 * radius - 1e-9 && d <= radius + 1e-9);
    }
}

#[test]
fn fire_spots_are_distinct_and_deterministic() {
    let at = DVec3::new(0.0, 64.0, 0.0);
    let radius = 24.0;
    let spots1 = fire_spots(at, radius, 42, 12);
    let spots2 = fire_spots(at, radius, 42, 12);
    assert_eq!(spots1, spots2);

    let spots3 = fire_spots(at, radius, 999, 12);
    assert_ne!(spots1, spots3);

    let mut deduped = spots1.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(deduped.len(), spots1.len());
}

#[test]
fn surface_y_finds_the_top_solid_block() {
    let found = surface_y(100, 0, |y| y <= 70);
    assert_eq!(found, Some(70));
}

#[test]
fn surface_y_none_over_air() {
    assert_eq!(surface_y(100, 0, |_| false), None);
}
