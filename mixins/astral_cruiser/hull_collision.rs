// Collision cells lent by the hosting Game for one entity phase. No window
// state, Game references or world pointers are dereferenced or shared across threads.
use crate::world::World;
use horizon_core::geometry::{Aabb, Vec3};
use std::{cell::RefCell, collections::HashSet};

struct Scope {
    phase: u8,
    world: *const World,
    boxes: Vec<Aabb>,
}
thread_local! {
    static SCOPES: RefCell<Vec<Scope>> = const { RefCell::new(Vec::new()) };
}

pub struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        SCOPES.with_borrow_mut(|scopes| { scopes.pop(); });
    }
}

pub fn active(phase: u8) -> bool {
    SCOPES.with_borrow(|scopes| scopes.iter().any(|s| s.phase == phase))
}

pub fn enter(phase: u8, world: &World, mut boxes: Vec<Aabb>) -> Guard {
    // Nearby bodies often query the same cells. Keep each cell only once.
    let mut seen = HashSet::new();
    boxes.retain(|b| {
        let (lo, hi) = (b.min_position(), b.max_position());
        seen.insert([lo.x, lo.y, lo.z, hi.x, hi.y, hi.z].map(f64::to_bits))
    });
    SCOPES.with_borrow_mut(|scopes| scopes.push(Scope { phase, world, boxes }));
    Guard
}

pub fn boxes_near(world: &World, area: Aabb) -> Vec<Aabb> {
    let area = area.inflate(Vec3::new(1.0e-7, 1.0e-7, 1.0e-7));
    SCOPES.with_borrow(|scopes| {
        scopes.last().filter(|s| std::ptr::eq(s.world, world)).map_or_else(Vec::new, |s| {
            s.boxes.iter().copied().filter(|b| b.intersects(area)).collect()
        })
    })
}
