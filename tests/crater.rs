#[path = "../mixins/astral_cruiser/crater.rs"]
mod astral_crater;

use astral_crater::{
    bowl, erase, erase_floor, fissures, flat_bowl, grid, rim, shaft, sphere, trench, uplift,
    Excavation, BATCH,
};
use glam::DVec3;

#[test]
fn excavation_remaining_tracks_consumed_cells() {
    let mut excavation = Excavation::new(vec![[0, 1, 0]; 5]);
    assert_eq!(excavation.remaining(), 5);
    assert_eq!(excavation.next_batch(3).len(), 3);
    assert_eq!(excavation.remaining(), 2);
    excavation.next_batch(10);
    assert_eq!(excavation.remaining(), 0);
}

#[test]
fn upward_clear_footprint_and_cone() {
    use astral_crater::{clear_columns, clear_top};
    let columns = clear_columns([-17, 70, 15], 5.0);
    assert_eq!(columns[0], [-17, 15]);
    let d2 = |p: [i32; 2]| (p[0] + 17).pow(2) + (p[1] - 15).pow(2);
    assert!(columns.iter().all(|&p| d2(p) <= 25));
    assert!(columns.windows(2).all(|p| d2(p[0]) <= d2(p[1])));
    assert_eq!(columns.len(), 81);
    assert_eq!(clear_columns([0; 3], 0.0), vec![[0, 0]]);
    assert!(clear_columns([0; 3], -1.0).is_empty());
    assert!(clear_columns([0; 3], f64::NAN).is_empty());
    assert_eq!(clear_top(0.0, 70, 64), 134);
    assert_eq!(clear_top(1.0, 70, 64), 96);
    assert_eq!(clear_top(0.0, -20, -1), -20);
    assert_eq!(clear_top(0.0, i32::MAX, 64), i32::MAX);
    assert!((0..100).all(|i| clear_top(i as f64 / 100.0, 70, 64)
        >= clear_top((i + 1) as f64 / 100.0, 70, 64)));
}

#[test]
fn section_order_preserves_cells_and_finishes_sections_top_down() {
    let cells = vec![[16, 1, 0], [-1, -1, 0], [0, 0, 0], [0, 15, 0], [15, 1, 0], [-1, -16, 0]];
    let mut dig = Excavation::sectioned(cells.clone());
    let ordered = dig.next_batch(100);
    let mut before = cells;
    let mut after = ordered.clone();
    before.sort(); after.sort();
    assert_eq!(before, after);
    let mut completed = std::collections::HashSet::new();
    for pair in ordered.windows(2) {
        let section = |p: [i32; 3]| p.map(|v| v >> 4);
        if section(pair[0]) == section(pair[1]) {
            assert!(pair[0][1] >= pair[1][1]);
        } else {
            completed.insert(section(pair[0]));
            assert!(!completed.contains(&section(pair[1])));
        }
    }
}

#[test]
fn section_budget_counts_halo_and_does_not_consume_rejected_cells() {
    use astral_crater::SectionBudget;
    let mut budget = SectionBudget::default();
    assert!(budget.admit([-16, 0, 0], 8)); // negative-coordinate corner: eight sections
    assert!(budget.admit([-16, 0, 0], 8)); // repeated edits are free
    assert!(!budget.admit([100, 100, 100], 8));
    assert!(budget.admit([-15, 1, 1], 8));
    assert!(budget.admit([100, 100, 100], 9));
    let mut shared = SectionBudget::default();
    for x in 0..24 { assert!(shared.admit([x * 16 + 1, 1, 1], 24)); }
    assert!(!shared.admit([24 * 16 + 1, 1, 1], 24));
}

fn dist(a: [i32; 3], b: [i32; 3]) -> f64 {
    let dx = (a[0] - b[0]) as f64;
    let dy = (a[1] - b[1]) as f64;
    let dz = (a[2] - b[2]) as f64;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[test]
fn bowl_is_ordered_from_the_middle_out() {
    let c = bowl([0, 64, 0], 14.0, 16.0, 7, -64);
    let d: Vec<f64> = c.iter().map(|p| dist(*p, [0, 64, 0])).collect();
    assert!(d.windows(2).all(|w| w[0] <= w[1] + 1e-9));
    assert_eq!(c[0], [0, 64, 0]);
}

#[test]
fn bowl_stays_within_its_rim() {
    for p in bowl([0, 64, 0], 24.0, 32.0, 1, -64) {
        assert!(((p[0] * p[0] + p[2] * p[2]) as f64).sqrt() <= 25.0);
        assert!(p[1] <= 64 && p[1] >= 64 - 33);
    }
}

#[test]
fn bowl_skips_cells_below_floor() {
    assert!(bowl([0, -50, 0], 24.0, 32.0, 1, -64).iter().all(|p| p[1] >= -64));
}

#[test]
fn bowl_is_the_same_for_the_same_seed() {
    assert_eq!(
        bowl([5, 70, 5], 14.0, 16.0, 3, -64),
        bowl([5, 70, 5], 14.0, 16.0, 3, -64)
    );
    assert_ne!(
        bowl([5, 70, 5], 14.0, 16.0, 3, -64),
        bowl([5, 70, 5], 14.0, 16.0, 4, -64)
    );
}

#[test]
fn sphere_holds_about_its_volume() {
    let n = sphere([0, 0, 0], 6.0, -64).len() as f64;
    let v = 4.0 / 3.0 * std::f64::consts::PI * 216.0;
    assert!((n - v).abs() / v < 0.1);
}

#[test]
fn trench_is_four_wide_and_eight_deep() {
    let t = trench(DVec3::new(0., 64., 0.), DVec3::new(20., 64., 0.), 4.0, 8.0, -64);
    assert!(t.iter().all(|p| p[2].abs() <= 2 && p[1] > 56 && p[1] <= 64));
    assert!(t.contains(&[10, 60, 0]));
    let mut s = t.clone();
    s.sort();
    s.dedup();
    assert_eq!(s.len(), t.len());
}

#[test]
fn excavation_hands_out_batches_in_order() {
    let cells = sphere([0, 64, 0], 12.0, -64);
    let n = cells.len();
    let mut e = Excavation::new(cells.clone());
    let first = e.next_batch(BATCH);
    assert_eq!(first[..], cells[..BATCH.min(n)]);
    while !e.is_done() {
        assert!(e.next_batch(BATCH).len() <= BATCH);
    }
}

#[test]
fn flat_bowl_floor_is_flat() {
    let cells = flat_bowl([0, 64, 0], 20.0, 10.0, 7, -64);
    assert!(!cells.is_empty());
    let limit: f64 = 0.5 * 20.0 / 1.5;
    let set: std::collections::HashSet<[i32; 3]> = cells.iter().copied().collect();
    let max_coord = limit.floor() as i32;
    let mut count = 0;
    for dx in -max_coord..=max_coord {
        for dz in -max_coord..=max_coord {
            if (dx as f64).abs() < limit && (dz as f64).abs() < limit {
                count += 1;
                assert!(
                    set.contains(&[dx, 54, dz]),
                    "missing floor at dx={}, dz={}",
                    dx,
                    dz
                );
            }
        }
    }
    assert!(count > 0);
}

#[test]
fn rim_is_hollow_shell_above_floor() {
    let r = rim([0, 64, 0], 10.0, 3.0, -64);
    assert!(!r.is_empty());
    assert!(r.iter().all(|p| p[1] > -64 && p[1] <= 64));
    for p in &r {
        let h = ((p[0] * p[0] + p[2] * p[2]) as f64).sqrt();
        assert!(h >= 10.0 - 1e-9 && h <= 13.0 + 1e-9);
    }
}

#[test]
fn shaft_stops_above_floor() {
    let cells = shaft([0, 64, 0], 4.0, 64, -64);
    assert!(!cells.is_empty());
    let min_y = cells.iter().map(|p| p[1]).min().unwrap();
    assert_eq!(min_y, -63);
    for p in &cells {
        let dx = p[0] as f64;
        let dz = p[2] as f64;
        assert!(dx * dx + dz * dz <= 4.0 * 4.0 + 1e-9);
    }
}

#[test]
fn fissure_respects_floor() {
    let cells = fissures([0, -60, 0], 8, 50.0, 20.0, 3.0, 1, -64);
    assert!(!cells.is_empty());
    assert!(cells.iter().all(|p| p[1] > -64));
}

#[test]
fn fissures_reach_length() {
    let cells = fissures([0, 64, 0], 8, 150.0, 20.0, 3.0, 1, -64);
    assert!(!cells.is_empty());
    let max_dist = cells
        .iter()
        .map(|p| ((p[0] * p[0] + p[2] * p[2]) as f64).sqrt())
        .fold(0.0f64, f64::max);
    assert!(max_dist >= 0.8 * 150.0);
}

#[test]
fn grid_has_lines_every_step() {
    let cells = grid([0, 64, 0], 30.0, 12.0, 2.0, 8.0, -64);
    assert!(!cells.is_empty());
    for x in [0, 12, 24, -12, -24] {
        assert!(cells.iter().any(|p| p[0] == x), "missing line at x={}", x);
    }
    for z in [0, 12, 24, -12, -24] {
        assert!(cells.iter().any(|p| p[2] == z), "missing line at z={}", z);
    }
    assert!(cells.iter().all(|p| p[0].abs() <= 31 && p[2].abs() <= 31));
}

#[test]
fn uplift_lifts_one_to_four() {
    let rings = vec![(20.0, 8.0), (35.0, 10.0)];
    let lifts = uplift([0, 0], &rings, 42);
    assert!(!lifts.is_empty());
    assert!(lifts.iter().all(|(_, l)| *l >= 1 && *l <= 4));
    let mut cols: Vec<[i32; 2]> = lifts.iter().map(|(c, _)| *c).collect();
    let total = cols.len();
    cols.sort_unstable();
    cols.dedup();
    assert_eq!(cols.len(), total);
}

#[test]
fn antimatter_bowl_size() {
    let cells = flat_bowl([0, 64, 0], 100.0, 40.0, 3, -64);
    assert!(cells.len() < 1_500_000);
}

#[test]
fn shapes_have_no_duplicates() {
    let fb = flat_bowl([0, 64, 0], 20.0, 10.0, 7, -64);
    let mut fb_dedup = fb.clone();
    fb_dedup.sort_unstable();
    fb_dedup.dedup();
    assert_eq!(fb.len(), fb_dedup.len());

    let f = fissures([0, 64, 0], 8, 50.0, 10.0, 3.0, 1, -64);
    let mut f_dedup = f.clone();
    f_dedup.sort_unstable();
    f_dedup.dedup();
    assert_eq!(f.len(), f_dedup.len());

    let g = grid([0, 64, 0], 30.0, 12.0, 2.0, 8.0, -64);
    let mut g_dedup = g.clone();
    g_dedup.sort_unstable();
    g_dedup.dedup();
    assert_eq!(g.len(), g_dedup.len());
}

#[test]
fn erase_carves_top_down_and_outward() {
    let center = [10, 64, -20];
    let radius = 15.0;
    let plane_y = 60;
    let top_y = 70;
    let cells = erase(center, radius, plane_y, top_y);

    assert!(!cells.is_empty());
    let mut prev_y = i32::MAX;
    let mut prev_da = 0u64;

    for cell in &cells {
        let dx = (cell[0] - center[0]) as f64;
        let dz = (cell[2] - center[2]) as f64;
        let dist_sq = dx * dx + dz * dz;
        assert!(dist_sq <= radius * radius + 1e-9);
        assert!(cell[1] > plane_y && cell[1] <= top_y);

        let da = ((cell[0] - center[0]).pow(2) + (cell[2] - center[2]).pow(2)) as u64;
        if cell[1] < prev_y {
            // Started lower y layer
            prev_y = cell[1];
            prev_da = da;
        } else if cell[1] == prev_y {
            // Same layer: must be outward (non-decreasing distance)
            assert!(da >= prev_da, "outward order violated: {da} < {prev_da}");
            prev_da = da;
        } else {
            panic!("top-down violated: cell y {} > prev_y {}", cell[1], prev_y);
        }
    }

    // No duplicates
    let mut dedup = cells.clone();
    dedup.sort_unstable();
    dedup.dedup();
    assert_eq!(cells.len(), dedup.len());
}

#[test]
fn erase_floor_is_flat_and_outward() {
    let center = [5, 62, 5];
    let radius = 12.0;
    let plane_y = 62;
    let floor_cells = erase_floor(center, radius, plane_y);

    assert!(!floor_cells.is_empty());
    assert_eq!(floor_cells[0], center);

    let mut prev_da = 0u64;
    for cell in &floor_cells {
        assert_eq!(cell[1], plane_y);
        let dx = (cell[0] - center[0]) as f64;
        let dz = (cell[2] - center[2]) as f64;
        assert!(dx * dx + dz * dz <= radius * radius + 1e-9);

        let da = ((cell[0] - center[0]).pow(2) + (cell[2] - center[2]).pow(2)) as u64;
        assert!(da >= prev_da);
        prev_da = da;
    }

    let mut dedup = floor_cells.clone();
    dedup.sort_unstable();
    dedup.dedup();
    assert_eq!(floor_cells.len(), dedup.len());
}
