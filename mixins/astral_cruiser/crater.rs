// Crater excavation shapes: bowl, sphere, and trench carving rules for orbital strikes.
// Ordered from the center out so block updates peel outward without sudden spikes.

use std::collections::HashMap;

use glam::DVec3;

pub const BATCH: usize = 4000;

/// Circular horizontal footprint, nearest columns first (including the rim).
pub fn clear_columns(center: [i32; 3], radius: f64) -> Vec<[i32; 2]> {
    if !radius.is_finite() || radius < 0.0 { return Vec::new(); }
    let r = radius.floor() as i32;
    let mut columns = Vec::new();
    for dx in -r..=r {
        for dz in -r..=r {
            if (dx as f64).powi(2) + (dz as f64).powi(2) <= radius * radius {
                if let (Some(x), Some(z)) = (center[0].checked_add(dx), center[2].checked_add(dz)) {
                    columns.push([x, z]);
                }
            }
        }
    }
    columns.sort_unstable_by_key(|&[x, z]| {
        let dx = x as i64 - center[0] as i64;
        let dz = z as i64 - center[2] as i64;
        (dx * dx + dz * dz, x, z)
    });
    columns
}

/// Absolute inclusive top Y, not an offset: full height centrally, 40% at rim.
pub fn clear_top(dx2dz2_over_r2: f64, center_y: i32, height: i32) -> i32 {
    let radial = dx2dz2_over_r2.clamp(0.0, 1.0).sqrt();
    let rise = (height.max(0) as f64 * (1.0 - 0.6 * radial)).round() as i32;
    center_y.saturating_add(rise)
}

/// Cap direct mesh invalidations, including the AO halo on section faces.
/// Lighting and block lifecycle side effects may invalidate additional sections.
#[derive(Default)]
pub struct SectionBudget {
    touched: std::collections::HashSet<[i32; 3]>,
}

impl SectionBudget {
    pub fn admit(&mut self, cell: [i32; 3], limit: usize) -> bool {
        let section = cell.map(|v| v >> 4);
        let halo = cell.map(|v| match v & 15 { 0 => -1, 15 => 1, _ => 0 });
        let mut affected = [[0; 3]; 8];
        let mut count = 0;
        for dx in [0, halo[0]] {
            for dy in [0, halo[1]] {
                for dz in [0, halo[2]] {
                    let s = [section[0] + dx, section[1] + dy, section[2] + dz];
                    if !affected[..count].contains(&s) {
                        affected[count] = s;
                        count += 1;
                    }
                }
            }
        }
        let extra = affected[..count].iter().filter(|s| !self.touched.contains(*s)).count();
        if self.touched.len() + extra > limit { return false; }
        self.touched.extend(affected[..count].iter().copied());
        true
    }
}

#[inline]
fn dist(a: [i32; 3], b: [i32; 3]) -> f64 {
    let dx = (a[0] - b[0]) as f64;
    let dy = (a[1] - b[1]) as f64;
    let dz = (a[2] - b[2]) as f64;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Integer hash of `(x, z, seed)` mapping to a radius offset in `-1.0..=1.0`.
#[inline]
fn noise_offset(x: i32, z: i32, seed: u64) -> f64 {
    let mut h = seed.wrapping_add(0x9E3779B97F4A7C15);
    h ^= (x as u32 as u64).wrapping_mul(0xBF58476D1CE4E5B9);
    h = (h ^ (h >> 30)).wrapping_mul(0x94D049BB133111EB);
    h ^= (z as u32 as u64).wrapping_mul(0x9E3779B97F4A7C15);
    h = (h ^ (h >> 27)).wrapping_mul(0xBF58476D1CE4E5B9);
    h ^= h >> 31;
    let rem = (h % 3) as i32;
    (rem - 1) as f64
}

/// Integer hash of `(x, z, seed)` mapping to a radius offset in `-1.5..=1.5`.
#[inline]
fn noise_offset_1_5(x: i32, z: i32, seed: u64) -> f64 {
    noise_offset(x, z, seed) * 1.5
}

/// Half ellipsoid downwards from `center` with semi-axes (radius, depth, radius),
/// an uneven rim with +/-1 block noise from `seed`, excluding cells below `floor`,
/// ordered by non-decreasing distance to `center`.
pub fn bowl(center: [i32; 3], radius: f64, depth: f64, seed: u64, floor: i32) -> Vec<[i32; 3]> {
    if radius < 0.0 || depth < 0.0 || center[1] < floor {
        return Vec::new();
    }
    if radius == 0.0 && depth == 0.0 {
        return vec![center];
    }
    let max_r = (radius + 1.0).ceil() as i32;
    let max_d = depth.ceil() as i32;
    let min_x = center[0] - max_r;
    let max_x = center[0] + max_r;
    let min_z = center[2] - max_r;
    let max_z = center[2] + max_r;
    let min_y = (center[1] - max_d).max(floor);
    let max_y = center[1];

    if min_y > max_y {
        return Vec::new();
    }

    let depth_sq = depth * depth;
    let mut cells = Vec::new();

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            let dx = (x - center[0]) as f64;
            let dz = (z - center[2]) as f64;
            let h_dist_sq = dx * dx + dz * dz;
            let dr = noise_offset(x, z, seed);
            let r_eff = radius + dr;
            if r_eff <= 0.0 {
                if h_dist_sq == 0.0 {
                    for y in min_y..=max_y {
                        let dy = (center[1] - y) as f64;
                        if dy <= depth + 1e-9 {
                            cells.push([x, y, z]);
                        }
                    }
                }
                continue;
            }
            let r_eff_sq = r_eff * r_eff;
            for y in min_y..=max_y {
                let dy = (center[1] - y) as f64;
                let norm = if depth_sq <= 1e-12 {
                    if dy == 0.0 {
                        h_dist_sq / r_eff_sq
                    } else {
                        f64::INFINITY
                    }
                } else {
                    (h_dist_sq / r_eff_sq) + ((dy * dy) / depth_sq)
                };
                if norm <= 1.0 + 1e-9 {
                    cells.push([x, y, z]);
                }
            }
        }
    }

    cells.sort_by(|a, b| {
        let da = dist(*a, center);
        let db = dist(*b, center);
        da.total_cmp(&db).then_with(|| a.cmp(b))
    });

    cells
}

/// Sphere centered at `center`, excluding cells below `floor`,
/// ordered by non-decreasing distance to `center`.
pub fn sphere(center: [i32; 3], radius: f64, floor: i32) -> Vec<[i32; 3]> {
    if radius < 0.0 {
        return Vec::new();
    }
    if radius == 0.0 {
        return if center[1] >= floor {
            vec![center]
        } else {
            Vec::new()
        };
    }
    let max_r = radius.ceil() as i32;
    let min_x = center[0] - max_r;
    let max_x = center[0] + max_r;
    let min_z = center[2] - max_r;
    let max_z = center[2] + max_r;
    let min_y = (center[1] - max_r).max(floor);
    let max_y = center[1] + max_r;

    if min_y > max_y {
        return Vec::new();
    }

    let r_sq = radius * radius;
    let mut cells = Vec::new();

    for x in min_x..=max_x {
        let dx = (x - center[0]) as f64;
        let dx_sq = dx * dx;
        for z in min_z..=max_z {
            let dz = (z - center[2]) as f64;
            let h_sq = dx_sq + dz * dz;
            if h_sq > r_sq + 1e-9 {
                continue;
            }
            for y in min_y..=max_y {
                let dy = (y - center[1]) as f64;
                if h_sq + dy * dy <= r_sq + 1e-9 {
                    cells.push([x, y, z]);
                }
            }
        }
    }

    cells.sort_by(|a, b| {
        let da = dist(*a, center);
        let db = dist(*b, center);
        da.total_cmp(&db).then_with(|| a.cmp(b))
    });

    cells
}

/// All cells whose horizontal distance to the segment `from..=to` is `<= width / 2.0`
/// and `top - depth < y <= top`, where `top = from.y.max(to.y).floor() as i32`,
/// with `y >= floor`, with no duplicates.
pub fn trench(from: DVec3, to: DVec3, width: f64, depth: f64, floor: i32) -> Vec<[i32; 3]> {
    if width < 0.0 || depth <= 0.0 {
        return Vec::new();
    }
    let top = from.y.max(to.y).floor() as i32;
    let min_y = ((top as f64 - depth).floor() as i32 + 1).max(floor);
    if min_y > top {
        return Vec::new();
    }

    let half_w = width / 2.0;
    let min_x = (from.x.min(to.x) - half_w).floor() as i32;
    let max_x = (from.x.max(to.x) + half_w).ceil() as i32;
    let min_z = (from.z.min(to.z) - half_w).floor() as i32;
    let max_z = (from.z.max(to.z) + half_w).ceil() as i32;

    let ab_x = to.x - from.x;
    let ab_z = to.z - from.z;
    let ab_len_sq = ab_x * ab_x + ab_z * ab_z;
    let half_w_sq = half_w * half_w;

    let mut cells = Vec::new();

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            let p_x = x as f64;
            let p_z = z as f64;
            let h_dist_sq = if ab_len_sq <= 1e-12 {
                let dx = p_x - from.x;
                let dz = p_z - from.z;
                dx * dx + dz * dz
            } else {
                let ap_x = p_x - from.x;
                let ap_z = p_z - from.z;
                let t = ((ap_x * ab_x + ap_z * ab_z) / ab_len_sq).clamp(0.0, 1.0);
                let proj_x = from.x + t * ab_x;
                let proj_z = from.z + t * ab_z;
                let dx = p_x - proj_x;
                let dz = p_z - proj_z;
                dx * dx + dz * dz
            };

            if h_dist_sq <= half_w_sq + 1e-9 {
                for y in min_y..=top {
                    cells.push([x, y, z]);
                }
            }
        }
    }

    cells
}

/// Batched block removal to avoid freezing the game server.
#[derive(Clone, Debug)]
pub struct Excavation {
    cells: Vec<[i32; 3]>,
    cursor: usize,
}

impl Excavation {
    /// Keep each section contiguous and peel it top-down. The time/write
    /// budget may still split a dense section across ticks.
    pub fn sectioned(mut cells: Vec<[i32; 3]>) -> Self {
        let mut rank = HashMap::new();
        let mut sections: Vec<Vec<[i32; 3]>> = Vec::new();
        for cell in cells.drain(..) {
            let index = *rank.entry(cell.map(|v| v >> 4)).or_insert_with(|| {
                sections.push(Vec::new());
                sections.len() - 1
            });
            sections[index].push(cell);
        }
        // Hash each cell only once, then sort small section-local buckets.
        // Avoid millions of hash lookups inside a whole-crater comparison sort.
        for mut section in sections {
            section.sort_unstable_by_key(|p| (std::cmp::Reverse(p[1]), p[0], p[2]));
            cells.append(&mut section);
        }
        Self::new(cells)
    }

    pub fn new(cells: Vec<[i32; 3]>) -> Self {
        Self { cells, cursor: 0 }
    }

    pub fn next_batch(&mut self, limit: usize) -> Vec<[i32; 3]> {
        if self.cursor >= self.cells.len() || limit == 0 {
            return Vec::new();
        }
        let end = (self.cursor + limit).min(self.cells.len());
        let batch = self.cells[self.cursor..end].to_vec();
        self.cursor = end;
        batch
    }

    pub fn is_done(&self) -> bool {
        self.cursor >= self.cells.len()
    }

    pub fn remaining(&self) -> usize {
        self.cells.len().saturating_sub(self.cursor)
    }
}

/// Flat-bottomed bowl: inner 60% of the radius goes straight down to center.y - depth;
/// from 0.6r to r the depth eases to 0 (smoothstep), the rim edge gets +-1.5 block noise from seed.
/// Includes every block from the surface center.y up to center.y+depth*0.25 (to blow away hills)
/// and down to the bottom, excluding cells below floor + 1.
pub fn flat_bowl(center: [i32; 3], radius: f64, depth: f64, seed: u64, floor: i32) -> Vec<[i32; 3]> {
    if radius < 0.0 || depth < 0.0 || center[1] <= floor {
        return Vec::new();
    }
    if radius == 0.0 && depth == 0.0 {
        return vec![center];
    }

    let max_r = (radius + 2.0).ceil() as i32;
    let min_x = center[0] - max_r;
    let max_x = center[0] + max_r;
    let min_z = center[2] - max_r;
    let max_z = center[2] + max_r;

    let top_y = (center[1] as f64 + depth * 0.25).floor() as i32;
    let r_inner = 0.6 * radius;

    let mut cells = Vec::new();

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            let dx = (x - center[0]) as f64;
            let dz = (z - center[2]) as f64;
            let h_dist = (dx * dx + dz * dz).sqrt();

            let dr = noise_offset_1_5(x, z, seed);
            let r_rim = radius + dr;

            if h_dist > r_rim {
                continue;
            }

            let cur_depth = if h_dist <= r_inner {
                depth
            } else {
                let span = (r_rim - r_inner).max(1e-6);
                let t = ((h_dist - r_inner) / span).clamp(0.0, 1.0);
                let s = t * t * (3.0 - 2.0 * t);
                depth * (1.0 - s)
            };

            let bottom_y = (center[1] as f64 - cur_depth).ceil() as i32;
            let min_y = bottom_y.max(floor + 1);

            if min_y <= top_y {
                for y in min_y..=top_y {
                    cells.push([x, y, z]);
                }
            }
        }
    }

    cells.sort_by(|a, b| {
        let da = (a[0] - center[0]) as i64 * (a[0] - center[0]) as i64
            + (a[1] - center[1]) as i64 * (a[1] - center[1]) as i64
            + (a[2] - center[2]) as i64 * (a[2] - center[2]) as i64;
        let db = (b[0] - center[0]) as i64 * (b[0] - center[0]) as i64
            + (b[1] - center[1]) as i64 * (b[1] - center[1]) as i64
            + (b[2] - center[2]) as i64 * (b[2] - center[2]) as i64;
        da.cmp(&db).then_with(|| a.cmp(b))
    });

    cells
}

/// Vertical cylinder, dx² + dz² <= r², from y = top down to floor + 1.
pub fn shaft(center: [i32; 3], radius: f64, top: i32, floor: i32) -> Vec<[i32; 3]> {
    if radius < 0.0 || top <= floor {
        return Vec::new();
    }
    let min_y = floor + 1;
    let max_r = radius.ceil() as i32;
    let r_sq = radius * radius;
    let mut cells = Vec::new();

    for x in (center[0] - max_r)..=(center[0] + max_r) {
        let dx = (x - center[0]) as f64;
        let dx_sq = dx * dx;
        for z in (center[2] - max_r)..=(center[2] + max_r) {
            let dz = (z - center[2]) as f64;
            let h_sq = dx_sq + dz * dz;
            if h_sq <= r_sq + 1e-9 {
                for y in min_y..=top {
                    cells.push([x, y, z]);
                }
            }
        }
    }

    cells.sort_by(|a, b| {
        let da = (a[0] - center[0]) as i64 * (a[0] - center[0]) as i64
            + (a[1] - center[1]) as i64 * (a[1] - center[1]) as i64
            + (a[2] - center[2]) as i64 * (a[2] - center[2]) as i64;
        let db = (b[0] - center[0]) as i64 * (b[0] - center[0]) as i64
            + (b[1] - center[1]) as i64 * (b[1] - center[1]) as i64
            + (b[2] - center[2]) as i64 * (b[2] - center[2]) as i64;
        da.cmp(&db).then_with(|| a.cmp(b))
    });

    cells
}

/// `count` radial cracks starting 6 blocks from center at evenly spread angles jittered by seed,
/// each a jagged polyline (segment ~8 blocks, lateral jitter) of total length `length`;
/// width tapers from `width` to 1 and depth from `depth` to 2 toward the tip;
/// dig from center.y + 3 down to center.y - local depth.
pub fn fissures(
    center: [i32; 3],
    count: u32,
    length: f64,
    depth: f64,
    width: f64,
    seed: u64,
    floor: i32,
) -> Vec<[i32; 3]> {
    if count == 0 || length <= 0.0 || depth <= 0.0 || width <= 0.0 {
        return Vec::new();
    }

    let mut cells = Vec::new();

    let seg_len = 8.0;
    let num_segs = ((length / seg_len).ceil() as usize).max(1);

    for crack_idx in 0..count {
        let mut h = seed
            .wrapping_add(0x9E3779B97F4A7C15)
            .wrapping_add((crack_idx as u64).wrapping_mul(0xBF58476D1CE4E5B9));
        let mut prng = || -> f64 {
            h = (h ^ (h >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            h = (h ^ (h >> 27)).wrapping_mul(0x94D049BB133111EB);
            h ^= h >> 31;
            ((h & 0xFFFF) as f64) / 65535.0
        };

        let base_angle = (crack_idx as f64 / count as f64) * 2.0 * std::f64::consts::PI;
        let angle_jitter = (prng() - 0.5) * (0.5 * 2.0 * std::f64::consts::PI / count as f64);
        let angle = base_angle + angle_jitter;

        let dir_x = angle.cos();
        let dir_z = angle.sin();
        let lat_x = -dir_z;
        let lat_z = dir_x;

        let start_dist = 6.0;
        let mut cur_x = center[0] as f64 + start_dist * dir_x;
        let mut cur_z = center[2] as f64 + start_dist * dir_z;

        for seg in 0..num_segs {
            let t0 = seg as f64 / num_segs as f64;
            let t1 = ((seg + 1) as f64 / num_segs as f64).min(1.0);
            let t_mid = (t0 + t1) * 0.5;

            let local_w = width + t_mid * (1.0 - width);
            let local_d = depth + t_mid * (2.0 - depth);

            let nominal_dist = (start_dist + (seg + 1) as f64 * seg_len).min(start_dist + length);
            let lat_offset = (prng() - 0.5) * 3.0;
            let next_x = center[0] as f64 + nominal_dist * dir_x + lat_x * lat_offset;
            let next_z = center[2] as f64 + nominal_dist * dir_z + lat_z * lat_offset;

            let half_w = (local_w / 2.0).max(0.6);
            let min_x = (cur_x.min(next_x) - half_w).floor() as i32;
            let max_x = (cur_x.max(next_x) + half_w).ceil() as i32;
            let min_z = (cur_z.min(next_z) - half_w).floor() as i32;
            let max_z = (cur_z.max(next_z) + half_w).ceil() as i32;

            let top_y = center[1] + 3;
            let bot_y = (center[1] as f64 - local_d).round() as i32;
            let min_y = bot_y.max(floor + 1);

            if min_y <= top_y {
                let seg_dx = next_x - cur_x;
                let seg_dz = next_z - cur_z;
                let seg_len_sq = seg_dx * seg_dx + seg_dz * seg_dz;
                let half_w_sq = half_w * half_w;

                for x in min_x..=max_x {
                    for z in min_z..=max_z {
                        let px = x as f64;
                        let pz = z as f64;
                        let h_dist_sq = if seg_len_sq <= 1e-12 {
                            let ddx = px - cur_x;
                            let ddz = pz - cur_z;
                            ddx * ddx + ddz * ddz
                        } else {
                            let apx = px - cur_x;
                            let apz = pz - cur_z;
                            let u = ((apx * seg_dx + apz * seg_dz) / seg_len_sq).clamp(0.0, 1.0);
                            let proj_x = cur_x + u * seg_dx;
                            let proj_z = cur_z + u * seg_dz;
                            let ddx = px - proj_x;
                            let ddz = pz - proj_z;
                            ddx * ddx + ddz * ddz
                        };

                        if h_dist_sq <= half_w_sq + 1e-9 {
                            for y in min_y..=top_y {
                                cells.push([x, y, z]);
                            }
                        }
                    }
                }
            }

            cur_x = next_x;
            cur_z = next_z;
        }
    }

    cells.sort_by(|a, b| {
        let da = (a[0] - center[0]) as i64 * (a[0] - center[0]) as i64
            + (a[1] - center[1]) as i64 * (a[1] - center[1]) as i64
            + (a[2] - center[2]) as i64 * (a[2] - center[2]) as i64;
        let db = (b[0] - center[0]) as i64 * (b[0] - center[0]) as i64
            + (b[1] - center[1]) as i64 * (b[1] - center[1]) as i64
            + (b[2] - center[2]) as i64 * (b[2] - center[2]) as i64;
        da.cmp(&db).then_with(|| a.cmp(b))
    });
    cells.dedup();

    cells
}

/// Trenches along x = cx + k*step and z = cz + k*step for all k with |k*step| <= half,
/// each line spanning [-half, half], width `width`, from center.y + 2 down to center.y - depth.
pub fn grid(
    center: [i32; 3],
    half: f64,
    step: f64,
    width: f64,
    depth: f64,
    floor: i32,
) -> Vec<[i32; 3]> {
    if half <= 0.0 || step <= 0.0 || width <= 0.0 || depth <= 0.0 {
        return Vec::new();
    }

    let mut cells = Vec::new();
    let top_y = center[1] + 2;
    let bot_y = (center[1] as f64 - depth).round() as i32;
    let min_y = bot_y.max(floor + 1);

    if min_y > top_y {
        return Vec::new();
    }

    let half_w = width / 2.0;
    let max_k = (half / step).floor() as i32;

    // Trenches along x = cx + k * step, spanning z in [cz - half, cz + half]
    for k in -max_k..=max_k {
        let line_x = center[0] as f64 + (k as f64) * step;
        let min_x = (line_x - half_w).floor() as i32;
        let max_x = (line_x + half_w).ceil() as i32;
        let min_z = (center[2] as f64 - half - half_w).floor() as i32;
        let max_z = (center[2] as f64 + half + half_w).ceil() as i32;

        for x in min_x..=max_x {
            let dx = (x as f64 - line_x).abs();
            if dx <= half_w + 1e-9 {
                for z in min_z..=max_z {
                    let dz = z as f64 - center[2] as f64;
                    let dist_z = if dz < -half {
                        -half - dz
                    } else if dz > half {
                        dz - half
                    } else {
                        0.0
                    };
                    if dx * dx + dist_z * dist_z <= half_w * half_w + 1e-9 {
                        for y in min_y..=top_y {
                            cells.push([x, y, z]);
                        }
                    }
                }
            }
        }
    }

    // Trenches along z = cz + k * step, spanning x in [cx - half, cx + half]
    for k in -max_k..=max_k {
        let line_z = center[2] as f64 + (k as f64) * step;
        let min_z = (line_z - half_w).floor() as i32;
        let max_z = (line_z + half_w).ceil() as i32;
        let min_x = (center[0] as f64 - half - half_w).floor() as i32;
        let max_x = (center[0] as f64 + half + half_w).ceil() as i32;

        for z in min_z..=max_z {
            let dz = (z as f64 - line_z).abs();
            if dz <= half_w + 1e-9 {
                for x in min_x..=max_x {
                    let dx = x as f64 - center[0] as f64;
                    let dist_x = if dx < -half {
                        -half - dx
                    } else if dx > half {
                        dx - half
                    } else {
                        0.0
                    };
                    if dz * dz + dist_x * dist_x <= half_w * half_w + 1e-9 {
                        for y in min_y..=top_y {
                            cells.push([x, y, z]);
                        }
                    }
                }
            }
        }
    }

    cells.sort_by(|a, b| {
        let da = (a[0] - center[0]) as i64 * (a[0] - center[0]) as i64
            + (a[1] - center[1]) as i64 * (a[1] - center[1]) as i64
            + (a[2] - center[2]) as i64 * (a[2] - center[2]) as i64;
        let db = (b[0] - center[0]) as i64 * (b[0] - center[0]) as i64
            + (b[1] - center[1]) as i64 * (b[1] - center[1]) as i64
            + (b[2] - center[2]) as i64 * (b[2] - center[2]) as i64;
        da.cmp(&db).then_with(|| a.cmp(b))
    });
    cells.dedup();

    cells
}

/// Hollow cylinder shell radius..radius+thickness, from center.y down to floor + 1.
pub fn rim(center: [i32; 3], radius: f64, thickness: f64, floor: i32) -> Vec<[i32; 3]> {
    if radius < 0.0 || thickness <= 0.0 || center[1] <= floor {
        return Vec::new();
    }
    let min_y = floor + 1;
    let top_y = center[1];
    let max_r = (radius + thickness).ceil() as i32;
    let r_inner_sq = radius * radius;
    let r_outer_sq = (radius + thickness) * (radius + thickness);

    let mut cells = Vec::new();

    for x in (center[0] - max_r)..=(center[0] + max_r) {
        let dx = (x - center[0]) as f64;
        let dx_sq = dx * dx;
        for z in (center[2] - max_r)..=(center[2] + max_r) {
            let dz = (z - center[2]) as f64;
            let h_sq = dx_sq + dz * dz;
            if h_sq >= r_inner_sq - 1e-9 && h_sq <= r_outer_sq + 1e-9 {
                for y in min_y..=top_y {
                    cells.push([x, y, z]);
                }
            }
        }
    }

    cells.sort_by(|a, b| {
        let da = (a[0] - center[0]) as i64 * (a[0] - center[0]) as i64
            + (a[1] - center[1]) as i64 * (a[1] - center[1]) as i64
            + (a[2] - center[2]) as i64 * (a[2] - center[2]) as i64;
        let db = (b[0] - center[0]) as i64 * (b[0] - center[0]) as i64
            + (b[1] - center[1]) as i64 * (b[1] - center[1]) as i64
            + (b[2] - center[2]) as i64 * (b[2] - center[2]) as i64;
        da.cmp(&db).then_with(|| a.cmp(b))
    });

    cells
}

/// For each (radius, width) ring, every column whose horizontal distance is within
/// width/2 of radius gets a lift of 1..=4 (larger near the ring's middle, +-1 noise from seed,
/// clamped to 1..=4). One entry per column; if rings overlap keep the larger lift.
pub fn uplift(center: [i32; 2], rings: &[(f64, f64)], seed: u64) -> Vec<([i32; 2], i32)> {
    if rings.is_empty() {
        return Vec::new();
    }

    let mut map = HashMap::new();

    for (radius, width) in rings {
        if *radius < 0.0 || *width <= 0.0 {
            continue;
        }
        let half_w = *width / 2.0;
        let max_r = (radius + half_w).ceil() as i32;

        for x in (center[0] - max_r)..=(center[0] + max_r) {
            let dx = (x - center[0]) as f64;
            let dx_sq = dx * dx;
            for z in (center[1] - max_r)..=(center[1] + max_r) {
                let dz = (z - center[1]) as f64;
                let d = (dx_sq + dz * dz).sqrt();
                let diff = (d - radius).abs();
                if diff <= half_w + 1e-9 {
                    let u = if half_w > 1e-9 { diff / half_w } else { 0.0 };
                    let base_lift = 4.0 - 2.5 * u;
                    let noise = noise_offset(x, z, seed);
                    let lift = ((base_lift + noise).round() as i32).clamp(1, 4);

                    map.entry([x, z])
                        .and_modify(|existing: &mut i32| *existing = (*existing).max(lift))
                        .or_insert(lift);
                }
            }
        }
    }

    let mut res: Vec<([i32; 2], i32)> = map.into_iter().collect();
    res.sort_by_key(|entry| entry.0);
    res
}

/// Every cell with dx²+dz² ≤ radius² and plane_y < y ≤ top_y,
/// ordered top-down then outward so the land visibly melts from the top.
pub fn erase(center: [i32; 3], radius: f64, plane_y: i32, top_y: i32) -> Vec<[i32; 3]> {
    if radius < 0.0 || top_y <= plane_y {
        return Vec::new();
    }
    let max_r = radius.ceil() as i32;
    let r_sq = radius * radius;
    let mut cells = Vec::new();

    for x in (center[0] - max_r)..=(center[0] + max_r) {
        let dx = (x - center[0]) as f64;
        let dx_sq = dx * dx;
        for z in (center[2] - max_r)..=(center[2] + max_r) {
            let dz = (z - center[2]) as f64;
            let h_sq = dx_sq + dz * dz;
            if h_sq <= r_sq + 1e-9 {
                for y in (plane_y + 1)..=top_y {
                    cells.push([x, y, z]);
                }
            }
        }
    }

    cells.sort_by(|a, b| {
        // 1. Top-down (descending y)
        b[1].cmp(&a[1])
            // 2. Outward from center (horizontal distance squared)
            .then_with(|| {
                let da = (a[0] - center[0]) as i64 * (a[0] - center[0]) as i64
                    + (a[2] - center[2]) as i64 * (a[2] - center[2]) as i64;
                let db = (b[0] - center[0]) as i64 * (b[0] - center[0]) as i64
                    + (b[2] - center[2]) as i64 * (b[2] - center[2]) as i64;
                da.cmp(&db)
            })
            // 3. Stable tie-breaker
            .then_with(|| a[0].cmp(&b[0]))
            .then_with(|| a[2].cmp(&b[2]))
    });

    cells
}

/// The plane cells (y == plane_y) for the glassy floor of Erasure,
/// with dx²+dz² ≤ radius², ordered outward from center.
pub fn erase_floor(center: [i32; 3], radius: f64, plane_y: i32) -> Vec<[i32; 3]> {
    if radius < 0.0 {
        return Vec::new();
    }
    let max_r = radius.ceil() as i32;
    let r_sq = radius * radius;
    let mut cells = Vec::new();

    for x in (center[0] - max_r)..=(center[0] + max_r) {
        let dx = (x - center[0]) as f64;
        let dx_sq = dx * dx;
        for z in (center[2] - max_r)..=(center[2] + max_r) {
            let dz = (z - center[2]) as f64;
            let h_sq = dx_sq + dz * dz;
            if h_sq <= r_sq + 1e-9 {
                cells.push([x, plane_y, z]);
            }
        }
    }

    cells.sort_by(|a, b| {
        let da = (a[0] - center[0]) as i64 * (a[0] - center[0]) as i64
            + (a[2] - center[2]) as i64 * (a[2] - center[2]) as i64;
        let db = (b[0] - center[0]) as i64 * (b[0] - center[0]) as i64
            + (b[2] - center[2]) as i64 * (b[2] - center[2]) as i64;
        da.cmp(&db)
            .then_with(|| a[0].cmp(&b[0]))
            .then_with(|| a[2].cmp(&b[2]))
    });

    cells
}
