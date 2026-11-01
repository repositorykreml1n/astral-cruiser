// A player's turn on the server, before their moves are judged.
fn astral_guest_tick(host: &mut crate::game::lan::LanHost, conn: u32) {
    crate::game::astral_cruiser::guest_tick(host, conn);
}
