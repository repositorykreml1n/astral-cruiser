// The tablet's loaded-world map. The atlas has no runtime allocation API:
// HudAtlas::slots reserves map/minimap regions while constructing the atlas.
// Those regions belong to the engine, so this module uses the specified
// 128-square fill fallback instead of overwriting another map's texture.
use super::Game;
use glam::DVec3;
use horizon_client::{
    blocks::{BlockFacts, Fluid, MAP_COLORS},
    chunk::Chunk,
    hud::{HudBatch, rgba},
};
use horizon_world::registry::BlockStateId;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Weak},
    time::{Duration, Instant},
};

const SIZE: usize = 256;
const CELLS: usize = 128;
const REFRESH_LIMIT: usize = 16;
const BUDGET: Duration = Duration::from_millis(2);
const RESCAN: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Default)]
struct Column {
    color: u8,
    height: i32,
    depth: u16,
}

struct Tile {
    columns: [Column; 256],
    chunk: Weak<Chunk>,
    revision: u64,
    scanned: Instant,
}

#[derive(Default)]
pub(super) struct TabletMap {
    tiles: HashMap<(i32, i32), Tile>,
    world: Option<(PathBuf, String)>,
    image: Vec<u8>,
    view: Option<(i32, i32, u8)>,
}

fn scan(chunk: &Arc<Chunk>, facts: &BlockFacts, air: u32, revision: u64) -> Tile {
    // Skip wholly empty upper sections, as the engine's Minimap does.
    let top = (0..chunk.sections.len())
        .rev()
        .find(|&i| !chunk.sections[i].is_uniform(air))
        .map_or(chunk.min_y(), |i| chunk.min_y() + i as i32 * 16 + 15);
    let bottom = chunk.min_y();
    let mut columns = [Column::default(); 256];
    for z in 0..16 {
        for x in 0..16 {
            for y in (bottom..=top).rev() {
                let block = facts.get(BlockStateId(chunk.get(x, y, z)));
                if block.map_color == 0 {
                    continue;
                }
                let mut color = block.map_color;
                let mut depth = 0u16;
                if block.fluid != Fluid::None {
                    let mut below = y;
                    while below >= bottom
                        && facts.get(BlockStateId(chunk.get(x, below, z))).fluid != Fluid::None
                    {
                        depth = depth.saturating_add(1);
                        below -= 1;
                    }
                    // A waterlogged block sturdy on top keeps its own colour.
                    if block.sturdy_full >> 1 & 1 == 0 {
                        color = if block.fluid == Fluid::Lava { 4 } else { 12 };
                    }
                }
                columns[(z * 16 + x) as usize] = Column {
                    color,
                    height: y,
                    depth,
                };
                break;
            }
        }
    }
    Tile {
        columns,
        chunk: Arc::downgrade(chunk),
        revision,
        scanned: Instant::now(),
    }
}

impl TabletMap {
    fn column(&self, x: i32, z: i32) -> Option<Column> {
        let tile = self.tiles.get(&(x >> 4, z >> 4))?;
        Some(tile.columns[((z & 15) * 16 + (x & 15)) as usize])
    }

    fn compose(&mut self, center: (i32, i32), zoom: u8) {
        let mut image = vec![0; SIZE * SIZE * 4];
        for py in 0..SIZE {
            for px in 0..SIZE {
                let x = center.0 + (px as i32 - 128) * i32::from(zoom);
                let z = center.1 + (py as i32 - 128) * i32::from(zoom);
                let color = self.column(x, z).filter(|c| c.color != 0);
                let pixel = if let Some(c) = color {
                    let parity = f64::from((x + z) & 1);
                    let shade = if c.color == 12 {
                        let depth = f64::from(c.depth) * 0.1 + parity * 0.2;
                        if depth < 0.5 {
                            255
                        } else if depth > 0.9 {
                            180
                        } else {
                            220
                        }
                    } else {
                        let north = self.column(x, z - 1).filter(|n| n.color != 0);
                        let slope = f64::from(c.height - north.map_or(c.height, |n| n.height))
                            * 0.8
                            + (parity - 0.5) * 0.4;
                        if slope > 0.6 {
                            255
                        } else if slope < -0.6 {
                            180
                        } else {
                            220
                        }
                    };
                    let rgb = MAP_COLORS.get(usize::from(c.color)).copied().unwrap_or(0);
                    let channel = |shift: u32| (((rgb >> shift) & 255) * shade as u32 / 255) as u8;
                    [channel(16), channel(8), channel(0), 255]
                } else {
                    // The grid is world-aligned and remains visible at every zoom.
                    let edge =
                        x.rem_euclid(16) < i32::from(zoom) || z.rem_euclid(16) < i32::from(zoom);
                    if edge {
                        [25, 38, 49, 255]
                    } else {
                        [10, 17, 25, 255]
                    }
                };
                let offset = (py * SIZE + px) * 4;
                image[offset..offset + 4].copy_from_slice(&pixel);
            }
        }
        self.image = image;
    }

    pub(super) fn draw(
        &mut self,
        game: &Game<'_>,
        batch: &mut HudBatch,
        rect: [f32; 4],
        center: DVec3,
        zoom: u8,
    ) {
        let zoom = match zoom {
            2 => 2,
            4 => 4,
            _ => 1,
        };
        let identity = (game.world_root.clone(), format!("{:?}", game.dim));
        if self.world.as_ref() != Some(&identity) {
            *self = Self {
                world: Some(identity),
                ..Self::default()
            };
        }
        let origin = (center.x.floor() as i32, center.z.floor() as i32);
        let chunk_center = (origin.0 >> 4, origin.1 >> 4);
        let radius = 8 * i32::from(zoom) + 1;
        let before = self.tiles.len();
        // Weak pointers detect a replaced world or chunk, even if revisions
        // and coordinates happen to match. They retain no world block data.
        self.tiles.retain(|key, tile| {
            (key.0 - chunk_center.0).abs() <= radius
                && (key.1 - chunk_center.1).abs() <= radius
                && game
                    .world
                    .chunks
                    .get(key)
                    .is_some_and(|chunk| tile.chunk.as_ptr() == Arc::as_ptr(chunk))
        });
        let mut changed = before != self.tiles.len();
        let started = Instant::now();
        let mut candidates = Vec::new();
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let key = (chunk_center.0 + dx, chunk_center.1 + dz);
                if !game.world.chunks.contains_key(&key) {
                    continue;
                }
                match self.tiles.get(&key) {
                    None => candidates.push((false, started, dx * dx + dz * dz, key)),
                    Some(tile)
                        if tile.revision != game.world.revision
                            && tile.scanned.elapsed() >= RESCAN =>
                    {
                        candidates.push((true, tile.scanned, dx * dx + dz * dz, key));
                    }
                    _ => {}
                }
            }
        }
        // Missing tiles first, nearest first; dirty tiles oldest first. This
        // prevents busy nearby chunks from starving the rest of a zoomed map.
        candidates.sort_unstable();
        for (_, _, _, key) in candidates.into_iter().take(REFRESH_LIMIT) {
            if let Some(chunk) = game.world.chunks.get(&key) {
                self.tiles.insert(
                    key,
                    scan(
                        chunk,
                        game.res.facts(),
                        game.world.air.0,
                        game.world.revision,
                    ),
                );
                changed = true;
            }
            if started.elapsed() >= BUDGET {
                break;
            }
        }
        let view = (origin.0, origin.1, zoom);
        if changed || self.view != Some(view) || self.image.is_empty() {
            self.compose(origin, zoom);
            self.view = Some(view);
        }
        let [x, y, w, h] = rect;
        let (cw, ch) = (w / CELLS as f32, h / CELLS as f32);
        for row in 0..CELLS {
            for col in 0..CELLS {
                // Average each 2x2 source cell: preserve thin coastlines and
                // the unloaded grid while reducing draw calls to 128x128.
                let mut sum = [0u32; 3];
                for dy in 0..2 {
                    for dx in 0..2 {
                        let offset = ((row * 2 + dy) * SIZE + col * 2 + dx) * 4;
                        for (channel, value) in sum.iter_mut().enumerate() {
                            *value += u32::from(self.image[offset + channel]);
                        }
                    }
                }
                batch.fill(
                    &game.gui.atlas,
                    x + col as f32 * cw,
                    y + row as f32 * ch,
                    cw,
                    ch,
                    rgba(
                        (sum[0] / 4) as u8,
                        (sum[1] / 4) as u8,
                        (sum[2] / 4) as u8,
                        255,
                    ),
                );
            }
        }
    }
}
