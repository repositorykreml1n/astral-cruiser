// The game backs its camera off the player: the pilot's stands further out.
fn astral_chase_view(camera: &mut crate::camera::Camera) -> std::ops::ControlFlow<()> {
    if crate::gfx::astral_ship::place_camera(&mut camera.position) {
        std::ops::ControlFlow::Break(())
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
