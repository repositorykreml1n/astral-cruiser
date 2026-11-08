fn astral_tablet_key(
    game: &mut crate::game::Game<'_>,
    key: winit::keyboard::KeyCode,
    shortcut: Option<horizon_client::clipboard::EditShortcut>,
    text: Option<&str>,
) -> std::ops::ControlFlow<Result<()>> {
    if crate::game::astral_cruiser::wheel_active(game) {
        return crate::game::astral_cruiser::key_wheel(game, key);
    }
    crate::game::astral_tablet_draw::key(game, key, shortcut, text)
}
