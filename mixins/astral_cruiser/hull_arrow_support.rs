fn astral_hull_arrow_support(scene: &ArrowScene<'_, '_>, bounds: Aabb) -> std::ops::ControlFlow<bool> {
    if crate::collisions::astral_hull_collision::boxes_near(scene.world, bounds).is_empty() {
        std::ops::ControlFlow::Continue(())
    } else {
        std::ops::ControlFlow::Break(false)
    }
}
