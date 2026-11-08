// Lives inside the streaming module, which keeps what it has lit and meshed private.
use super::Streaming;
use crate::world::World;

impl Streaming {
    /// Takes every column `from` holds in `from_world` into this streaming and
    /// `world`: the chunk, its block entities, and that it is lit. With `meshed`
    /// its meshes count as made too (they went over with the renderer's sections),
    /// and only the sections `from` still had to mesh are meshed here; without,
    /// every taken section is meshed here again. Returns how many columns came.
    pub fn astral_take_columns(
        &mut self,
        world: &mut World,
        from: &mut Streaming,
        from_world: &mut World,
        meshed: bool,
    ) -> usize {
        let sections = world.layout().sections();
        let keys: Vec<(i32, i32)> = from_world.chunks.keys().copied().collect();
        let mut taken = std::collections::HashSet::new();
        for &(x, z) in &keys {
            // Server-sent columns win over the portal's older snapshot. The
            // renderer was swapped, so rebuild these from the real world.
            if world.chunks.contains_key(&(x, z)) {
                for sy in 0..sections {
                    self.meshing.remove(&(x, sy as i32, z));
                    self.compile_started.remove(&(x, sy as i32, z));
                    world.dirty_sections.insert((x, sy as i32, z));
                }
                continue;
            }
            let Some(chunk) = from_world.chunks.remove(&(x, z)) else {
                continue;
            };
            taken.insert((x, z));
            world.chunks.insert((x, z), chunk);
            if from.lit.remove(&(x, z)) {
                self.lit.insert((x, z));
            }
            let made = meshed && from.meshed.remove(&(x, z));
            if made {
                self.meshed.insert((x, z));
            }
            let pending = from_world.dirty_sections.take_column(x, z);
            for sy in 0..sections {
                self.meshing.remove(&(x, sy as i32, z));
                self.compile_started.remove(&(x, sy as i32, z));
                let in_flight = from.meshing.remove(&(x, sy as i32, z)).is_some();
                if !made || in_flight || pending.contains(sy) {
                    world.dirty_sections.insert((x, sy as i32, z));
                }
            }
        }
        let positions: Vec<_> = from_world.block_entities.keys().copied().collect();
        for pos in positions {
            if !taken.contains(&(pos.0 >> 4, pos.2 >> 4)) {
                continue;
            }
            if let Some(entity) = from_world.block_entities.remove(&pos) {
                world.block_entities.insert(pos, entity);
            }
        }
        taken.len()
    }
}
