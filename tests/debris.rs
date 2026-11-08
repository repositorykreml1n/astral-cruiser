#[path = "../mixins/astral_cruiser/debris.rs"]
mod debris;
use glam::DVec3;

#[test]
fn outward_gravity_drag_spin_and_expiry() {
    let mut pool = debris::Debris::default();
    pool.spawn([10, 0, 0], 42, DVec3::ZERO, 1.0, 7);
    let before = pool.particles[0].clone();
    assert!(before.vel.x > 0.0 && before.vel.y > 0.0);
    pool.tick(0.1);
    let after = &pool.particles[0];
    assert!(after.vel.x < before.vel.x && after.vel.y < before.vel.y);
    assert_ne!(after.spin, before.spin);
    assert!(after.heat < before.heat && after.life < before.life);
    pool.tick(6.0);
    assert!(pool.particles.is_empty());
}

#[test]
fn deterministic_and_drops_oldest_at_cap() {
    let mut pool = debris::Debris::default();
    for i in 0..4002 { pool.spawn([i, 0, 0], i as u32, DVec3::ZERO, 1.0, i as u64); }
    assert_eq!(pool.particles.len(), 1500);
    assert_eq!(pool.particles.front().unwrap().block, 2502);
    let mut other = debris::Debris::default();
    other.spawn([4001, 0, 0], 4001, DVec3::ZERO, 1.0, 4001);
    assert_eq!(pool.particles.back().unwrap().vel, other.particles[0].vel);
}
