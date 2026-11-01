// The game asks far terrain round the player; in a dash it goes round a point
// ahead, by way of the same function entered again.
fn astral_far_lead(
    lod: &mut crate::lod::Lod,
    camera: [f64; 3],
    detail: i32,
    distance: u32,
    covered: &Option<crate::fast_hash::FxHashSet<(i32, i32)>>,
) -> std::ops::ControlFlow<Vec<(TileKey, SectionMesh)>> {
    thread_local! {
        static ENTERED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if ENTERED.get() {
        return std::ops::ControlFlow::Continue(());
    }
    let Some(lead) = crate::gfx::astral_ship::far_lead() else {
        return std::ops::ControlFlow::Continue(());
    };
    ENTERED.set(true);
    let result = lod.update(
        [camera[0] + lead[0], camera[1], camera[2] + lead[1]],
        detail,
        distance,
        covered.clone(),
    );
    ENTERED.set(false);
    std::ops::ControlFlow::Break(result)
}
