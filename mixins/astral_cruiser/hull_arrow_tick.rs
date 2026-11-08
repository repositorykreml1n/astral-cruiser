fn astral_hull_arrow_tick(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<Result<()>> {
    crate::game::astral_cruiser::hull_arrow_tick(game, |game| game.tick_creature_entities())
}
