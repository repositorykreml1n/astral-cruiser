// Weapon selection wheel: layout, hit-testing, and item positioning.
// Pure module depending only on std and sibling modules astral_arsenal and astral_weapons.

use std::f32::consts::TAU;
use super::astral_arsenal::Weapon;
use super::astral_weapons::WeaponKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Lasers(WeaponKind),
    Strike(Weapon),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sector {
    pub label: &'static str,
    pub color: u32,
    pub items: Vec<Pick>,
    pub start: f32,
    pub end: f32, // radians, 0 = up, clockwise
}

#[derive(Clone, Debug, PartialEq)]
pub struct Wheel {
    pub center: (f32, f32),
    pub min_dim: f32,
    pub dead: f32,
    pub inner_r0: f32,
    pub inner: f32,
    pub outer: f32,
    pub sectors: Vec<Sector>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover {
    None,
    Sector(usize),
    Item(usize, usize),
}

/// Normalizes an angle in radians into the half-open range `[0.0, TAU)`.
#[inline]
fn norm_angle(a: f32) -> f32 {
    let rem = a.rem_euclid(TAU);
    if rem < 0.0 {
        rem + TAU
    } else {
        rem
    }
}

/// Computes the radial wheel layout centered in the screen.
///
/// Radii scale with `min(screen_w, screen_h)`:
/// - dead zone: 0.05 of min side
/// - inner ring: 0.10..0.20 of min side
/// - outer cards: ~0.30..0.36 of min side (outer hit radius: 0.42 of min side)
///
/// 6 sectors are created clockwise starting from top (0 = up):
/// 0. "Лазеры" (4 ship laser mounts)
/// 1. "Классика" (Group::Classic)
/// 2. "Супер" (Group::Super)
/// 3. "Зона" (Group::Zone)
/// 4. "Особые" (Group::Special)
/// 5. "Точные" (Group::Precise)
pub fn layout(screen_w: f32, screen_h: f32) -> Wheel {
    let min_dim = screen_w.min(screen_h).max(0.0);
    let dead = min_dim * 0.05;
    let inner_r0 = min_dim * 0.10;
    let inner = min_dim * 0.20;
    let outer = min_dim * 0.42;
    let center = (screen_w * 0.5, screen_h * 0.5);

    let sector_defs: [(&'static str, u32, Vec<Pick>); 6] = [
        (
            "Лазеры",
            0x59D9FF,
            WeaponKind::ALL.iter().copied().map(Pick::Lasers).collect(),
        ),
        (
            "Классика",
            0xFFB040,
            vec![
                Pick::Strike(Weapon::Ion),
                Pick::Strike(Weapon::Barrage),
                Pick::Strike(Weapon::Rod),
                Pick::Strike(Weapon::Lance),
            ],
        ),
        (
            "Супер",
            0xF4F0FF,
            vec![
                Pick::Strike(Weapon::Antimatter),
                Pick::Strike(Weapon::SolarPillar),
                Pick::Strike(Weapon::Tectonic),
                Pick::Strike(Weapon::Erasure),
                Pick::Strike(Weapon::Supernova),
            ],
        ),
        (
            "Зона",
            0xFF5020,
            vec![
                Pick::Strike(Weapon::Meteors),
                Pick::Strike(Weapon::Cluster),
                Pick::Strike(Weapon::Cryo),
            ],
        ),
        (
            "Особые",
            0xA040FF,
            vec![
                Pick::Strike(Weapon::BlackHole),
                Pick::Strike(Weapon::Chain),
                Pick::Strike(Weapon::PlasmaWall),
            ],
        ),
        (
            "Точные",
            0x40FF60,
            vec![
                Pick::Strike(Weapon::Missiles),
                Pick::Strike(Weapon::Drill),
                Pick::Strike(Weapon::Grid),
            ],
        ),
    ];

    let count = sector_defs.len();
    let step = TAU / count as f32;
    let sectors = sector_defs
        .into_iter()
        .enumerate()
        .map(|(i, (label, color, items))| {
            let start = i as f32 * step;
            let end = if i + 1 == count { TAU } else { (i + 1) as f32 * step };
            Sector {
                label,
                color,
                items,
                start,
                end,
            }
        })
        .collect();

    Wheel {
        center,
        min_dim,
        dead,
        inner_r0,
        inner,
        outer,
        sectors,
    }
}

/// Returns the center coordinates `(x, y)` for drawing `item` of `sector` in the outer ring.
///
/// Items are laid out along the sector's arc at radius ~0.30..0.36.
/// Cards are spaced so neighbouring cards are at least card width apart.
/// If a sector has 5 or more items, two concentric arcs are used.
pub fn item_pos(wheel: &Wheel, sector: usize, item: usize) -> (f32, f32) {
    let Some(sec) = wheel.sectors.get(sector) else {
        return wheel.center;
    };
    let count = sec.items.len().max(1);
    let item_idx = item.min(count - 1);
    let mid_angle = (sec.start + sec.end) * 0.5;

    let (arc_r, arc_idx, arc_count) = if count >= 5 {
        let half = (count + 1) / 2;
        if item_idx < half {
            (wheel.min_dim * 0.30, item_idx, half)
        } else {
            (wheel.min_dim * 0.36, item_idx - half, count - half)
        }
    } else {
        (wheel.min_dim * 0.33, item_idx, count)
    };

    let card_w = 84.0;
    let min_chord = card_w + 8.0;
    let min_step = if arc_r > 1.0 {
        2.0 * (min_chord / (2.0 * arc_r)).min(0.99).asin()
    } else {
        0.0
    };

    let base_span = sec.end - sec.start;
    let angle = if arc_count <= 1 {
        mid_angle
    } else {
        let base_step = base_span / arc_count as f32;
        let step = base_step.max(min_step);
        let total_span = (arc_count - 1) as f32 * step;
        mid_angle - total_span * 0.5 + arc_idx as f32 * step
    };

    (
        wheel.center.0 + arc_r * angle.sin(),
        wheel.center.1 - arc_r * angle.cos(),
    )
}

/// Hit-tests screen coordinates `(x, y)` against the wheel geometry.
///
/// - Inside `inner_r0` radius: returns `Hover::None`.
/// - Inner ring (`inner_r0` to `inner`): returns `Hover::Sector(s)` by cursor angle.
/// - Outer ring (`inner` to `outer`):
///   - If `open_sector` is `Some(s)`, items of sector `s` have priority within its spread arc.
///     Otherwise falls back to the sector containing the cursor.
///   - Returns `Hover::Item(sector, item_idx)` for the closest card.
/// - Outside `outer` radius: returns `Hover::None`.
pub fn hit(wheel: &Wheel, x: f32, y: f32, open_sector: Option<usize>) -> Hover {
    if !x.is_finite() || !y.is_finite() {
        return Hover::None;
    }
    let dx = x - wheel.center.0;
    let dy = y - wheel.center.1;
    let dist = dx.hypot(dy);

    if dist < wheel.inner_r0 || dist > wheel.outer || wheel.sectors.is_empty() {
        return Hover::None;
    }

    // 0 = up (dy < 0, dx = 0), clockwise angle
    let angle = norm_angle(dx.atan2(-dy));

    let normal_sector_idx = wheel
        .sectors
        .iter()
        .position(|s| angle >= s.start && angle < s.end)
        .unwrap_or(wheel.sectors.len() - 1);

    if dist < wheel.inner {
        return Hover::Sector(normal_sector_idx);
    }

    // Outer ring: check if open_sector has wider priority
    if let Some(target) = open_sector {
        if let Some(sec) = wheel.sectors.get(target) {
            let count = sec.items.len();
            let (arc_r, arc_count) = if count >= 5 {
                (wheel.min_dim * 0.30, (count + 1) / 2)
            } else {
                (wheel.min_dim * 0.33, count)
            };
            let min_step = if arc_r > 1.0 {
                2.0 * ((84.0 + 8.0) / (2.0 * arc_r)).min(0.99).asin()
            } else {
                0.0
            };
            let base_step = (sec.end - sec.start) / arc_count.max(1) as f32;
            let step = base_step.max(min_step);
            let total_span = (arc_count.max(1) - 1) as f32 * step;
            let half_wide = (total_span * 0.5 + step * 0.5).max((sec.end - sec.start) * 0.6);
            let mid = (sec.start + sec.end) * 0.5;

            let mut diff = (angle - mid).rem_euclid(TAU);
            if diff > std::f32::consts::PI {
                diff -= TAU;
            }

            if diff.abs() <= half_wide {
                let mut best_idx = 0;
                let mut best_dist = f32::MAX;
                for (i, _) in sec.items.iter().enumerate() {
                    let (ix, iy) = item_pos(wheel, target, i);
                    let d = (x - ix).hypot(y - iy);
                    if d < best_dist {
                        best_dist = d;
                        best_idx = i;
                    }
                }
                return Hover::Item(target, best_idx);
            }
        }
    }

    // Default (open_sector is None or cursor outside open_sector wide arc):
    // Find the closest item among all sectors
    let mut best_sec = normal_sector_idx;
    let mut best_item = 0;
    let mut best_dist = f32::MAX;
    for (s_idx, sec) in wheel.sectors.iter().enumerate() {
        for (i_idx, _) in sec.items.iter().enumerate() {
            let (ix, iy) = item_pos(wheel, s_idx, i_idx);
            let d = (x - ix).hypot(y - iy);
            if d < best_dist {
                best_dist = d;
                best_sec = s_idx;
                best_item = i_idx;
            }
        }
    }
    Hover::Item(best_sec, best_item)
}
