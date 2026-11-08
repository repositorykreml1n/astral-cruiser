#[path = "../mixins/astral_cruiser/scorch.rs"]
mod astral_scorch;

use astral_scorch::*;
use std::collections::HashSet;

#[test]
fn cursor_is_not_exhausted_at_the_first_column_on_the_outer_ring() {
    let mut cursor = RingCursor::new([0, 0]);
    assert!(!cursor.exhausted(2.0));
    let mut columns = Vec::new();
    while cursor.reached() < 2.0 {
        columns.extend(cursor.advance(2.0, 1, |_, _| true));
    }
    assert!(!cursor.exhausted(2.0));
    while !cursor.exhausted(2.0) {
        columns.extend(cursor.advance(2.0, 1, |_, _| true));
    }
    assert_eq!(columns.len(), 13);
    assert_eq!(columns.iter().collect::<HashSet<_>>().len(), 13);
}

#[test]
fn pillar_rim_never_fills_void_fluid_or_deep_columns() {
    assert_eq!(rim_material(0, 64, 0, 0, false, true), None);
    assert_eq!(rim_material(0, 64, 0, 0, true, false), None);
    for depth in [-1, 26, 100, 384] {
        assert_eq!(rim_material(0, 64, 0, depth, true, true), None);
    }
    assert!(rim_material(0, 64, 0, 25, true, true).is_some());
}

#[test]
fn pillar_rim_is_mostly_stone_with_sparse_magma_and_obsidian() {
    let mut magma = 0;
    let mut obsidian = 0;
    let mut same_neighbor = 0;
    for x in -100..100 {
        for z in -100..100 {
            let material = rim_material(x, 64, z, 5, true, true).unwrap();
            magma += usize::from(material == "magma_block");
            obsidian += usize::from(material == "obsidian");
            same_neighbor += usize::from(Some(material) == rim_material(x + 1, 64, z, 5, true, true));
        }
    }
    assert!((5200..6800).contains(&magma));
    assert!((400..1200).contains(&obsidian));
    assert!(same_neighbor > 4000, "must not alternate like a checkerboard");
}

#[test]
fn grass_becomes_dirt() {
    // Normal cases (rng % 3 != 0 and rng % 23 != 0)
    assert_eq!(burn("minecraft:grass_block", 1, 1), Burn::Dirt);
    assert_eq!(burn("minecraft:mycelium", 1, 1), Burn::Dirt);
    assert_eq!(burn("minecraft:podzol", 1, 1), Burn::Dirt);
    assert_eq!(burn("minecraft:dirt_path", 1, 1), Burn::Dirt);

    // Coarse dirt when rng % 3 == 0 (and not fire)
    assert_eq!(burn("minecraft:grass_block", 1, 3), Burn::CoarseDirt);
    assert_eq!(burn("minecraft:mycelium", 1, 6), Burn::CoarseDirt);
    assert_eq!(burn("minecraft:podzol", 1, 9), Burn::CoarseDirt);
    assert_eq!(burn("minecraft:dirt_path", 1, 12), Burn::CoarseDirt);

    // Top block with rng % 23 == 0 turns to Fire (when base is Dirt)
    assert_eq!(burn("minecraft:grass_block", 0, 23), Burn::Fire);
    assert_eq!(burn("minecraft:grass_block", 0, 46), Burn::Fire);

    // Below top (depth > 0) does not become Fire even if rng % 23 == 0
    assert_eq!(burn("minecraft:grass_block", 1, 23), Burn::Dirt);
}

#[test]
fn leaves_and_plants_burn_to_air() {
    assert_eq!(burn("minecraft:oak_leaves", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:birch_leaves", 5, 23), Burn::Air);
    assert_eq!(burn("minecraft:cherry_leaves", 15, 0), Burn::Air);

    assert_eq!(burn("minecraft:oak_sapling", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:white_carpet", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:moss_carpet", 0, 1), Burn::Air);

    assert_eq!(burn("minecraft:short_grass", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:tall_grass", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:fern", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:large_fern", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:dead_bush", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:dandelion", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:poppy", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:red_tulip", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:sweet_berry_bush", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:vine", 0, 1), Burn::Air);
    assert_eq!(burn("minecraft:snow", 0, 1), Burn::Air);
}

#[test]
fn logs_only_top_twelve() {
    // Top 12 blocks (depth 0..=12) become Air
    for depth in 0..=12 {
        assert_eq!(
            burn("minecraft:oak_log", depth, 1),
            Burn::Air,
            "depth {} should burn to Air",
            depth
        );
        assert_eq!(
            burn("minecraft:stripped_oak_log", depth, 1),
            Burn::Air,
            "depth {} stripped should burn to Air",
            depth
        );
        assert_eq!(
            burn("minecraft:spruce_wood", depth, 1),
            Burn::Air,
            "depth {} wood should burn to Air",
            depth
        );
        assert_eq!(
            burn("minecraft:crimson_stem", depth, 1),
            Burn::Air,
            "depth {} stem should burn to Air",
            depth
        );
    }

    // Depth > 12 are kept
    assert_eq!(burn("minecraft:oak_log", 13, 1), Burn::Keep);
    assert_eq!(burn("minecraft:stripped_oak_log", 20, 1), Burn::Keep);
    assert_eq!(burn("minecraft:crimson_stem", 50, 1), Burn::Keep);
}

#[test]
fn stone_kept() {
    // Stone is kept
    assert_eq!(burn("minecraft:stone", 1, 1), Burn::Keep);
    assert_eq!(burn("minecraft:stone", 0, 1), Burn::Keep);
    assert_eq!(burn("minecraft:deepslate", 5, 23), Burn::Keep);
    assert_eq!(burn("minecraft:bedrock", 0, 1), Burn::Keep);

    // Top block with rng % 23 == 0 catches fire
    assert_eq!(burn("minecraft:stone", 0, 23), Burn::Fire);
    assert_eq!(burn("minecraft:stone", 0, 46), Burn::Fire);
}

#[test]
fn ring_cursor_covers_every_column_once() {
    let mut cursor = RingCursor::new([0, 0]);
    let mut all_cols = Vec::new();
    let mut outer = 7.0;

    loop {
        let batch = cursor.advance(outer, 97, |_, _| true);
        all_cols.extend(batch);
        if outer < 50.0 {
            outer = (outer + 7.0).min(50.0);
        } else if cursor.reached() >= 50.0 {
            let more = cursor.advance(50.0, 97, |_, _| true);
            if more.is_empty() {
                break;
            }
            all_cols.extend(more);
        }
    }

    // Collect all expected columns with x^2 + z^2 <= 2500
    let mut expected = Vec::new();
    for x in -50..=50 {
        for z in -50..=50 {
            if x * x + z * z <= 2500 {
                expected.push([x, z]);
            }
        }
    }

    // Check none outside
    for &[x, z] in &all_cols {
        assert!(
            x * x + z * z <= 2500,
            "column [{}, {}] is outside radius 50",
            x,
            z
        );
    }

    // Check count and uniqueness
    assert_eq!(all_cols.len(), expected.len());
    let set: HashSet<[i32; 2]> = all_cols.iter().copied().collect();
    assert_eq!(set.len(), expected.len());
}

#[test]
fn ring_cursor_skips_unloaded() {
    let mut cursor = RingCursor::new([0, 0]);
    let mut returned = Vec::new();

    while cursor.reached() < 50.0 {
        let batch = cursor.advance(50.0, 97, |x, _z| x >= 0);
        for &[x, _z] in &batch {
            assert!(x >= 0, "returned column has x < 0: {}", x);
        }
        if batch.is_empty() && cursor.reached() >= 50.0 {
            break;
        }
        returned.extend(batch);
    }

    assert!(cursor.reached() >= 50.0);
    for &[x, _z] in &returned {
        assert!(x >= 0, "returned column has x < 0: {}", x);
    }
}

#[test]
fn scorch_column_without_heightmap_scans_down() {
    let found = scan_surface(100, 0, |y| y <= 64);
    assert_eq!(found, Some(64));

    let found_mid = scan_surface(128, -64, |y| y == 30);
    assert_eq!(found_mid, Some(30));

    let none = scan_surface(100, 0, |_| false);
    assert_eq!(none, None);

    let inverted = scan_surface(0, 100, |_| true);
    assert_eq!(inverted, None);
}

#[test]
fn ring_cursor_large_radius_fast() {
    let start = std::time::Instant::now();
    let mut cursor = RingCursor::new([0, 0]);
    let mut calls = 0;
    let mut total_cols = 0;

    while cursor.reached() < 500.0 {
        let batch = cursor.advance(500.0, 6000, |_, _| true);
        calls += 1;
        total_cols += batch.len();
        if batch.is_empty() && cursor.reached() >= 500.0 {
            break;
        }
    }

    let elapsed = start.elapsed();

    // 785k columns with limit 6000 takes ~131 calls
    assert!(calls >= 100 && calls <= 200, "call count was {}", calls);
    assert!(
        total_cols >= 780_000 && total_cols <= 790_000,
        "total cols was {}",
        total_cols
    );
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "elapsed time {:?} was >= 2.0s",
        elapsed
    );
}

#[test]
fn leaves_always_near_centre() {
    let rng_samples = [0, 1, 42, 9999, u64::MAX / 2, u64::MAX - 1, u64::MAX];
    let blocks = [
        "minecraft:oak_leaves",
        "minecraft:birch_leaves",
        "minecraft:short_grass",
        "minecraft:dandelion",
        "minecraft:snow",
        "minecraft:white_carpet",
    ];
    for d in [0.0, 0.1, 0.2, 0.35, 0.39] {
        for &block in &blocks {
            for &rng in &rng_samples {
                assert_eq!(
                    erode(block, d, 0, false, 1.0, rng),
                    Burn::Air,
                    "block {block} at d={d} with rng={rng} should be Air"
                );
                assert_eq!(
                    erode(block, d, 2, true, 0.8, rng),
                    Burn::Air,
                    "block {block} at d={d} damaging with rng={rng} should be Air"
                );
            }
        }
    }
}

#[test]
fn leaves_rare_at_edge() {
    let total = 20_000;
    let mut air_count = 0;
    let mut rng = 0x123456789abcdef0_u64;
    for _ in 0..total {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let res = erode("minecraft:oak_leaves", 0.95, 0, false, 1.0, rng);
        if res == Burn::Air {
            air_count += 1;
        }
    }
    let fraction = air_count as f64 / total as f64;
    assert!(
        fraction < 0.30,
        "fraction of leaves removed at d=0.95 was {fraction}, expected < 0.30"
    );
    assert!(
        fraction > 0.10,
        "fraction of leaves removed at d=0.95 was {fraction}, expected > 0.10"
    );
}

#[test]
fn ground_only_when_damaging() {
    let ground_blocks = [
        "minecraft:grass_block",
        "minecraft:dirt",
        "minecraft:stone",
        "minecraft:sand",
        "minecraft:gravel",
        "minecraft:andesite",
        "minecraft:terracotta",
        "minecraft:sandstone",
    ];

    // Non-damaging wave: ground is never touched
    for &b in &ground_blocks {
        for depth in 0..5 {
            assert_eq!(
                erode(b, 0.1, depth, false, 1.0, 0),
                Burn::Keep,
                "ground {b} at depth {depth} should be Keep when not damaging"
            );
        }
    }

    // Damaging wave: ground at surface (depth 0, d=0.1) can be removed to Air
    assert_eq!(
        erode("minecraft:stone", 0.1, 0, true, 1.0, 0),
        Burn::Air
    );
    assert_eq!(
        erode("minecraft:dirt", 0.1, 0, true, 1.0, 0),
        Burn::Air
    );
    // Grass block below max_depth turns to Dirt
    assert_eq!(
        erode("minecraft:grass_block", 0.1, 3, true, 1.0, 0),
        Burn::Dirt
    );
}

#[test]
fn ground_depth_shrinks_with_distance() {
    // At d=0.0: round(3 * 1.0^2) = 3 -> depth 0, 1, 2 removed (rng=0 gives r=0 < 1.0)
    for depth in 0..3 {
        assert_eq!(
            erode("minecraft:stone", 0.0, depth, true, 1.0, 0),
            Burn::Air,
            "depth {depth} at d=0.0 should be Air"
        );
    }
    assert_eq!(
        erode("minecraft:stone", 0.0, 3, true, 1.0, 0),
        Burn::Keep,
        "depth 3 at d=0.0 should be Keep"
    );

    // At d=0.5: round(3 * 0.5^2) = round(0.75) = 1 -> depth 0 removed, depth 1 kept
    assert_eq!(
        erode("minecraft:stone", 0.5, 0, true, 1.0, 0),
        Burn::Air,
        "depth 0 at d=0.5 should be Air"
    );
    assert_eq!(
        erode("minecraft:stone", 0.5, 1, true, 1.0, 0),
        Burn::Keep,
        "depth 1 at d=0.5 should be Keep"
    );

    // At d=0.85: round(3 * 0.15^2) = round(0.0675) = 0 -> even depth 0 is kept
    assert_eq!(
        erode("minecraft:stone", 0.85, 0, true, 1.0, 0),
        Burn::Keep,
        "depth 0 at d=0.85 should be Keep"
    );
}

#[test]
fn bedrock_never() {
    let unbreakables = [
        "minecraft:bedrock",
        "minecraft:obsidian",
        "minecraft:water",
        "minecraft:lava",
    ];
    for &b in &unbreakables {
        for &damaging in &[false, true] {
            assert_eq!(
                erode(b, 0.0, 0, damaging, 2.0, 0),
                Burn::Keep,
                "{b} must never be removed"
            );
        }
    }
}
