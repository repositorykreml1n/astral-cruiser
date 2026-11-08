// Use the existing movement solver and context, adding only entity boxes.
fn astral_hull_move(
    scene: &mut crate::collisions::SceneCollisions<'_, '_>,
    bounds: Aabb,
    movement: Vec3,
    max_step: f32,
    on_ground: bool,
    context: CollisionContext<'_>,
) -> std::ops::ControlFlow<Result<Vec3>> {
    let area = horizon_core::geometry::entity_collision_bounds(movement, bounds, max_step);
    let boxes = crate::collisions::astral_hull_collision::boxes_near(scene.world, area);
    if boxes.is_empty() { return std::ops::ControlFlow::Continue(()) }
    let mut collisions = crate::collisions::EntityCollisions {
        blocks: crate::collisions::SceneCollisions {
            world: scene.world,
            binding: scene.binding,
            subject: scene.subject,
        },
        entities: &boxes,
        border: None,
    };
    std::ops::ControlFlow::Break(
        resolve_world_movement(&mut collisions, MovementRequest {
            entity: EntityIdentity(0), context, bounds, movement, max_step,
            on_ground, candidate_budget: 1_000_000,
        }).map_err(Into::into)
    )
}
