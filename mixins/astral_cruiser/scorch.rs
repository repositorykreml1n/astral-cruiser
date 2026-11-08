// Surface scorch rules and ring-by-ring column iteration for orbital strikes.

// Convert only existing dry solid terrain in the shallow local surface band.
pub fn rim_material(x: i32, y: i32, z: i32, depth: i32, solid: bool, dry: bool) -> Option<&'static str> {
    if !solid || !dry || !(0..=25).contains(&depth) { return None; }
    let mut hash = (x as u64).wrapping_mul(73856093)
        ^ (z as u64).wrapping_mul(83492791)
        ^ (y.div_euclid(3) as u64).wrapping_mul(19349663);
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
    let mix = (hash ^ (hash >> 31)) % 100;
    Some(if mix < 2 { "obsidian" }
        else if mix < 17 { "magma_block" }
        else if mix < 50 { "blackstone" }
        else if depth > 12 || mix < 75 { "basalt" }
        else { "smooth_basalt" })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Burn {
    Keep,
    Air,
    Dirt,
    CoarseDirt,
    Fire,
}

fn is_foliage_or_plant(id: &str) -> bool {
    if id.ends_with("_leaves")
        || id.ends_with("_sapling")
        || id.ends_with("_carpet")
        || id.ends_with("_flower")
        || id.ends_with("_tulip")
        || id.ends_with("_plant")
        || id.ends_with("_bush")
    {
        return true;
    }
    matches!(
        id,
        "short_grass"
            | "grass"
            | "tall_grass"
            | "fern"
            | "large_fern"
            | "dead_bush"
            | "dandelion"
            | "poppy"
            | "blue_orchid"
            | "allium"
            | "azure_bluet"
            | "red_tulip"
            | "orange_tulip"
            | "white_tulip"
            | "pink_tulip"
            | "oxeye_daisy"
            | "cornflower"
            | "lily_of_the_valley"
            | "wither_rose"
            | "sunflower"
            | "lilac"
            | "rose_bush"
            | "peony"
            | "sweet_berry_bush"
            | "vine"
            | "vines"
            | "weeping_vines"
            | "twisting_vines"
            | "cave_vines"
            | "cave_vines_plant"
            | "snow"
            | "moss_carpet"
            | "torchflower"
            | "torchflower_crop"
            | "pitcher_plant"
            | "pitcher_crop"
            | "hanging_roots"
            | "spore_blossom"
            | "big_dripleaf"
            | "small_dripleaf"
            | "bamboo"
            | "sugar_cane"
            | "cactus"
            | "lily_pad"
            | "glow_lichen"
            | "sculk_vein"
    )
}

fn is_log_or_wood(id: &str) -> bool {
    id.ends_with("_log") || id.ends_with("_wood") || id.ends_with("_stem") || id.ends_with("_hyphae")
}

fn is_ground(id: &str) -> bool {
    if id.contains("deepslate") {
        return false;
    }
    if id == "terracotta" || id.ends_with("_terracotta") {
        return true;
    }
    if id == "sandstone"
        || id == "red_sandstone"
        || id.ends_with("_sandstone")
    {
        return true;
    }
    matches!(
        id,
        "grass_block"
            | "dirt"
            | "coarse_dirt"
            | "podzol"
            | "mycelium"
            | "dirt_path"
            | "sand"
            | "red_sand"
            | "gravel"
            | "stone"
            | "andesite"
            | "diorite"
            | "granite"
    )
}

/// Evaluates what a block at `depth_from_top` in a column becomes during scorching.
/// `depth_from_top = 0` for the topmost non-air block.
pub fn burn(block: &str, depth_from_top: i32, rng: u64) -> Burn {
    let id = block.strip_prefix("minecraft:").unwrap_or(block);

    let base = if matches!(id, "grass_block" | "mycelium" | "podzol" | "dirt_path") {
        if rng % 3 == 0 {
            Burn::CoarseDirt
        } else {
            Burn::Dirt
        }
    } else if is_foliage_or_plant(id) {
        Burn::Air
    } else if is_log_or_wood(id) {
        if depth_from_top <= 12 {
            Burn::Air
        } else {
            Burn::Keep
        }
    } else {
        Burn::Keep
    };

    if depth_from_top == 0 && (base == Burn::Keep || base == Burn::Dirt) && rng % 23 == 0 {
        Burn::Fire
    } else {
        base
    }
}

/// What a shockwave front does to a column block at relative distance d/r (0 centre .. 1 edge).
pub fn erode(
    block: &str,
    d_over_r: f64,
    depth_from_top: i32,
    damaging: bool,
    strength: f64,
    rng: u64,
) -> Burn {
    let id = block.strip_prefix("minecraft:").unwrap_or(block);

    // Never remove bedrock/obsidian/water/lava
    if matches!(
        id,
        "bedrock"
            | "obsidian"
            | "crying_obsidian"
            | "water"
            | "lava"
            | "flowing_water"
            | "flowing_lava"
    ) {
        return Burn::Keep;
    }

    if d_over_r < 0.0 || d_over_r > 1.0 {
        return Burn::Keep;
    }
    let d = d_over_r.clamp(0.0, 1.0);

    // Leaves, plants, flowers, grass, snow layer, carpets
    if is_foliage_or_plant(id) {
        let p_base = if d < 0.4 {
            1.0
        } else {
            1.0 - ((d - 0.4) / 0.6) * 0.85
        };
        let strength_scale = (strength / 0.5).min(1.0).max(0.0);
        let p = p_base * strength_scale;
        if p >= 1.0 {
            return Burn::Air;
        }
        let r = (rng as f64) / 18446744073709551616.0;
        return if r < p { Burn::Air } else { Burn::Keep };
    }

    // Logs: Air if depth_from_top <= 12 and d < 0.5 and damaging, else Keep
    if is_log_or_wood(id) {
        return if damaging && depth_from_top >= 0 && depth_from_top <= 12 && d < 0.5 {
            Burn::Air
        } else {
            Burn::Keep
        };
    }

    // Ground: only when damaging
    if damaging && is_ground(id) {
        let max_depth = (3.0 * (1.0 - d) * (1.0 - d)).round() as i32;
        if depth_from_top >= 0 && depth_from_top < max_depth {
            let prob = 1.0 - d;
            if prob >= 1.0 {
                return Burn::Air;
            }
            let r = (rng as f64) / 18446744073709551616.0;
            if r < prob {
                return Burn::Air;
            }
            if id == "grass_block" {
                return Burn::Dirt;
            }
            return Burn::Keep;
        } else if id == "grass_block" {
            return Burn::Dirt;
        } else {
            return Burn::Keep;
        }
    }

    // Everything else Keep
    Burn::Keep
}

/// Scans downwards from `top` down to `bottom`, returning the first y where `solid(y)` is true.
pub fn scan_surface(top: i32, bottom: i32, solid: impl Fn(i32) -> bool) -> Option<i32> {
    for y in (bottom..=top).rev() {
        if solid(y) {
            return Some(y);
        }
    }
    None
}

/// Generates integer grid offsets (dx, dz) where r <= hypot(dx, dz) < r + 1,
/// sorted by distance to the origin and deterministically ordered.
fn generate_ring(r: i32) -> Vec<[i32; 2]> {
    if r < 0 {
        return Vec::new();
    }
    if r == 0 {
        return vec![[0, 0]];
    }
    let r_i64 = r as i64;
    let min_r_sq = r_i64 * r_i64;
    let max_r_sq = (r_i64 + 1) * (r_i64 + 1) - 1;

    let mut points = Vec::new();

    for dx in -r..=r {
        let dx_sq = (dx as i64) * (dx as i64);
        let min_dz_sq = (min_r_sq - dx_sq).max(0);
        let max_dz_sq = max_r_sq - dx_sq;
        if max_dz_sq < 0 {
            continue;
        }

        let mut min_dz = (min_dz_sq as f64).sqrt().ceil() as i64;
        let mut max_dz = (max_dz_sq as f64).sqrt().floor() as i64;

        while (min_dz - 1) * (min_dz - 1) >= min_dz_sq && min_dz > 0 {
            min_dz -= 1;
        }
        while min_dz * min_dz < min_dz_sq {
            min_dz += 1;
        }
        while (max_dz + 1) * (max_dz + 1) <= max_dz_sq {
            max_dz += 1;
        }
        while max_dz * max_dz > max_dz_sq && max_dz >= 0 {
            max_dz -= 1;
        }

        if min_dz > max_dz {
            continue;
        }

        for dz in min_dz..=max_dz {
            let dz_i32 = dz as i32;
            points.push([dx, dz_i32]);
            if dz_i32 > 0 {
                points.push([dx, -dz_i32]);
            }
        }
    }

    points.sort_by(|a, b| {
        let da = (a[0] as i64) * (a[0] as i64) + (a[1] as i64) * (a[1] as i64);
        let db = (b[0] as i64) * (b[0] as i64) + (b[1] as i64) * (b[1] as i64);
        da.cmp(&db).then_with(|| a[0].cmp(&b[0])).then_with(|| a[1].cmp(&b[1]))
    });

    points
}

/// Outward expanding cursor iterating over column coordinates ring by ring.
pub struct RingCursor {
    center: [i32; 2],
    current_r: i32,
    current_ring: Vec<[i32; 2]>,
    current_idx: usize,
    reached: f64,
}

impl RingCursor {
    pub fn new(center: [i32; 2]) -> Self {
        Self {
            center,
            current_r: 0,
            current_ring: generate_ring(0),
            current_idx: 0,
            reached: 0.0,
        }
    }

    /// Radius fully covered so far.
    pub fn reached(&self) -> f64 {
        self.reached
    }

    /// Unlike reached(), this includes every column at the final radius.
    pub fn exhausted(&self, outer: f64) -> bool {
        if let Some(&[dx, dz]) = self.current_ring.get(self.current_idx) {
            (dx as f64).hypot(dz as f64) > outer + 1e-9
        } else {
            (self.current_r + 1) as f64 > outer + 1e-9
        }
    }

    /// Advances the cursor returning columns (x, z) not returned before whose distance
    /// to center is <= outer, nearest rings first, at most `limit` per call.
    /// Unloaded columns are skipped and counted as done.
    pub fn advance(
        &mut self,
        outer: f64,
        limit: usize,
        loaded: impl Fn(i32, i32) -> bool,
    ) -> Vec<[i32; 2]> {
        if limit == 0 || outer < 0.0 {
            return Vec::new();
        }
        let mut result = Vec::new();

        loop {
            if self.current_idx >= self.current_ring.len() {
                let next_r = self.current_r + 1;
                if next_r as f64 > outer + 1e-9 {
                    self.reached = self.reached.max(outer);
                    break;
                }
                self.current_r = next_r;
                self.current_ring = generate_ring(self.current_r);
                self.current_idx = 0;
                continue;
            }

            let [dx, dz] = self.current_ring[self.current_idx];
            let d = (dx as f64).hypot(dz as f64);
            if d > outer + 1e-9 {
                self.reached = self.reached.max(outer);
                break;
            }

            self.current_idx += 1;
            self.reached = self.reached.max(d);
            let col = [self.center[0] + dx, self.center[1] + dz];
            if loaded(col[0], col[1]) {
                result.push(col);
                if result.len() >= limit {
                    break;
                }
            }
        }

        result
    }
}
