fn astral_hull_item_tick(
    game: &mut crate::game::Game<'_>,
    running: bool,
) -> std::ops::ControlFlow<Result<()>> {
    crate::game::astral_cruiser::hull_item_tick(game, running)
}
