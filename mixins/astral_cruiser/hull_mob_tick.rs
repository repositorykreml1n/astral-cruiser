fn astral_hull_mob_tick(game: &mut crate::game::Game<'_>) -> std::ops::ControlFlow<Result<()>> {
    crate::game::astral_cruiser::hull_mob_tick(game)
}
