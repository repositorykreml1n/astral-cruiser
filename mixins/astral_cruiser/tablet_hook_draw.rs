fn astral_tablet_draw(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> std::ops::ControlFlow<()> {
    let _timer = crate::game::astral_cruiser::tablet_perf();
    if crate::game::astral_cruiser::wheel_active(game) {
        return crate::game::astral_cruiser::draw_wheel(game, batch);
    }
    crate::game::astral_tablet_draw::draw(game, batch)
}
