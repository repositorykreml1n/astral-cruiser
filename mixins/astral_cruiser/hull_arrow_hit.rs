// Let the ordinary hit search see only the segment before the hull. Blocks
// and entities in front still win; a hull hit uses the arrow's own stop routine.
fn astral_hull_arrow_hit(
    ticker: &mut Ticker<'_, '_, '_, '_>,
    from: Vec3,
    to: Vec3,
    targets: &[(Who, Aabb)],
) -> std::ops::ControlFlow<Hit> {
    thread_local! {
        static COLLECTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if COLLECTING.get() { return std::ops::ControlFlow::Continue(()) }
    let area = Aabb::new(from, from).expand_towards(to.subtract(from))
        .inflate(Vec3::new(1.0e-7, 1.0e-7, 1.0e-7));
    let boxes = crate::collisions::astral_hull_collision::boxes_near(ticker.scene.world, area);
    let hit = boxes.iter().filter_map(|b| b.clip(from, to))
        .min_by(|a, b| from.distance_squared(*a).total_cmp(&from.distance_squared(*b)));
    let Some(at) = hit else { return std::ops::ControlFlow::Continue(()) };
    let Some(air) = ticker.scene.registry.default_state("minecraft:air") else {
        return std::ops::ControlFlow::Continue(());
    };
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) { COLLECTING.set(false); }
    }
    COLLECTING.set(true);
    let _reset = Reset;
    let hit = ticker.step_move_and_hit(from, at, targets);
    if hit != Hit::None || ticker.st.removed || ticker.st.in_ground {
        return std::ops::ControlFlow::Break(hit);
    }
    // A virtual hull has no block reactions. AIR is passed only to the normal
    // stop routine, never to projectile_hit, so it cannot touch a real block.
    let pos = ticker.block_position();
    ticker.on_hit_block(pos, air);
    // Make AbstractArrow.shouldFall recheck the hull every tick: it may move.
    ticker.st.last_state = None;
    std::ops::ControlFlow::Break(Hit::None)
}
