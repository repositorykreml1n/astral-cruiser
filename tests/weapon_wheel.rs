#[path = "../mixins/astral_cruiser/lang.rs"]
#[allow(unused)]
mod astral_lang;

#[path = "../mixins/astral_cruiser/weapons.rs"]
#[allow(unused)]
mod astral_weapons;

#[path = "../mixins/astral_cruiser/arsenal.rs"]
#[allow(unused)]
mod astral_arsenal;

#[path = "../mixins/astral_cruiser/weapon_wheel.rs"]
mod astral_weapon_wheel;

use astral_arsenal::Weapon;
use astral_weapons::WeaponKind;
use astral_weapon_wheel::{hit, item_pos, layout, Hover, Pick};
use std::collections::HashSet;
use std::f32::consts::TAU;

#[test]
fn sectors_cover_full_circle() {
    let w = layout(800.0, 600.0);
    assert_eq!(w.sectors.len(), 6);
    assert_eq!(w.sectors[0].start, 0.0);

    for i in 0..w.sectors.len() {
        let sec = &w.sectors[i];
        assert!(sec.end > sec.start, "sector {i} has empty or inverted span");
        if i + 1 < w.sectors.len() {
            assert!(
                (sec.end - w.sectors[i + 1].start).abs() < 1e-5,
                "gap between sector {i} and {}",
                i + 1
            );
        }
    }
    assert!(
        (w.sectors.last().unwrap().end - TAU).abs() < 1e-5,
        "last sector does not reach TAU"
    );
}

#[test]
fn dead_zone_is_none() {
    let w = layout(800.0, 600.0);
    // Center point
    assert_eq!(hit(&w, w.center.0, w.center.1, None), Hover::None);

    // Points inside dead circle
    for step in 0..12 {
        let angle = step as f32 * (TAU / 12.0);
        let r = w.dead * 0.5;
        let x = w.center.0 + r * angle.sin();
        let y = w.center.1 - r * angle.cos();
        assert_eq!(hit(&w, x, y, None), Hover::None);
    }

    // Points outside outer radius
    for step in 0..8 {
        let angle = step as f32 * (TAU / 8.0);
        let r = w.outer * 1.5;
        let x = w.center.0 + r * angle.sin();
        let y = w.center.1 - r * angle.cos();
        assert_eq!(hit(&w, x, y, None), Hover::None);
    }
}

#[test]
fn angle_maps_to_sector() {
    let w = layout(800.0, 600.0);
    let r = (w.inner_r0 + w.inner) * 0.5;

    for (i, sec) in w.sectors.iter().enumerate() {
        let mid = (sec.start + sec.end) * 0.5;
        let x = w.center.0 + r * mid.sin();
        let y = w.center.1 - r * mid.cos();
        assert_eq!(
            hit(&w, x, y, None),
            Hover::Sector(i),
            "mid-angle sector mismatch for sector {i}"
        );

        // Near start
        let near_start = sec.start + 0.05 * (sec.end - sec.start);
        let x_ns = w.center.0 + r * near_start.sin();
        let y_ns = w.center.1 - r * near_start.cos();
        assert_eq!(hit(&w, x_ns, y_ns, None), Hover::Sector(i));

        // Near end
        let near_end = sec.end - 0.05 * (sec.end - sec.start);
        let x_ne = w.center.0 + r * near_end.sin();
        let y_ne = w.center.1 - r * near_end.cos();
        assert_eq!(hit(&w, x_ne, y_ne, None), Hover::Sector(i));
    }
}

#[test]
fn outer_ring_picks_item() {
    let w = layout(800.0, 600.0);

    // Default (open_sector: None): each item_pos maps to its exact Hover::Item(sec, item)
    for (s_idx, sec) in w.sectors.iter().enumerate() {
        for (i_idx, _) in sec.items.iter().enumerate() {
            let (x, y) = item_pos(&w, s_idx, i_idx);
            assert_eq!(
                hit(&w, x, y, None),
                Hover::Item(s_idx, i_idx),
                "item_pos mismatch for sector {s_idx}, item {i_idx}"
            );
        }
    }

    // With open_sector: Some(s), hovering within sector s returns an item of sector s
    let target_sec = 2;
    let sec = &w.sectors[target_sec];
    let mid = (sec.start + sec.end) * 0.5;
    let r = (w.inner + w.outer) * 0.5;
    let x = w.center.0 + r * mid.sin();
    let y = w.center.1 - r * mid.cos();
    match hit(&w, x, y, Some(target_sec)) {
        Hover::Item(s, _) => assert_eq!(s, target_sec),
        other => panic!("expected Hover::Item for open_sector, got {other:?}"),
    }
}

#[test]
fn every_weapon_is_on_the_wheel() {
    let w = layout(800.0, 600.0);

    let mut laser_kinds = Vec::new();
    let mut strike_weapons = HashSet::new();
    let mut total_count = 0;

    for sec in &w.sectors {
        for item in &sec.items {
            total_count += 1;
            match item {
                Pick::Lasers(kind) => {
                    assert!(
                        !laser_kinds.contains(kind),
                        "duplicate laser kind {kind:?} found on wheel"
                    );
                    laser_kinds.push(*kind);
                }
                Pick::Strike(weapon) => {
                    assert!(
                        strike_weapons.insert(*weapon),
                        "duplicate strike weapon {weapon:?} found on wheel"
                    );
                }
            }
        }
    }

    assert_eq!(total_count, 22);
    assert_eq!(laser_kinds.len(), WeaponKind::ALL.len());
    for kind in WeaponKind::ALL {
        assert!(laser_kinds.contains(&kind), "missing laser kind {kind:?}");
    }

    assert_eq!(strike_weapons.len(), Weapon::ALL.len());
    for weapon in &Weapon::ALL {
        assert!(
            strike_weapons.contains(weapon),
            "missing strike weapon {weapon:?}"
        );
    }

    let mut reachable = HashSet::new();
    for (s_idx, sec) in w.sectors.iter().enumerate() {
        for (i_idx, _) in sec.items.iter().enumerate() {
            let (x, y) = item_pos(&w, s_idx, i_idx);
            if let Hover::Item(s, i) = hit(&w, x, y, None) {
                if let Pick::Strike(weapon) = w.sectors[s].items[i] {
                    reachable.insert(weapon);
                }
            }
        }
    }
    assert_eq!(reachable.len(), Weapon::ALL.len());
    for weapon in &Weapon::ALL {
        assert!(
            reachable.contains(weapon),
            "weapon {weapon:?} is not reachable through hit()"
        );
    }
}

#[test]
fn layout_scales() {
    let wide = layout(1920.0, 1080.0);
    assert_eq!(wide.center, (960.0, 540.0));
    let min_dim_wide = 1080.0;
    assert!((wide.dead - min_dim_wide * 0.05).abs() < 1e-4);
    assert!((wide.inner_r0 - min_dim_wide * 0.10).abs() < 1e-4);
    assert!((wide.inner - min_dim_wide * 0.20).abs() < 1e-4);
    assert!((wide.outer - min_dim_wide * 0.42).abs() < 1e-4);

    let small = layout(400.0, 300.0);
    assert_eq!(small.center, (200.0, 150.0));
    let min_dim_small = 300.0;
    assert!((small.dead - min_dim_small * 0.05).abs() < 1e-4);
    assert!((small.inner_r0 - min_dim_small * 0.10).abs() < 1e-4);
    assert!((small.inner - min_dim_small * 0.20).abs() < 1e-4);
    assert!((small.outer - min_dim_small * 0.42).abs() < 1e-4);
}
