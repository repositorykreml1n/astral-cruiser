// What the player's own motion collides with, besides blocks.
fn astral_solid_hull(game: &crate::game::Game<'_>) -> std::ops::ControlFlow<Vec<Aabb>> {
    crate::game::astral_cruiser::solid_hull(game)
}
