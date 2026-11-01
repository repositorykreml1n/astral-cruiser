fn astral_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::astral_cruiser::command(game, command, ran);
}
