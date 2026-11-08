fn astral_tablet_scroll(game: &mut crate::game::Game<'_>, delta: f32) -> std::ops::ControlFlow<()> {
    if crate::game::astral_cruiser::wheel_active(game) {
        return std::ops::ControlFlow::Break(());
    }
    crate::game::astral_tablet_draw::scroll(game, delta)
}
