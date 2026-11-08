#[allow(dead_code, unused_imports)]
#[path = "../mixins/astral_cruiser/lang.rs"]
mod astral_lang;
#[path = "../mixins/astral_cruiser/tablet.rs"]
mod astral_tablet;

use astral_tablet::{Hit, Tab, TabletState, hit, layout};

#[test]
fn layout_scales_with_screen() {
    let wide = layout(800.0, 600.0);
    assert_eq!(wide.panel.w, 440.0);
    assert_eq!(wide.panel.h, 270.0);
    assert_eq!(wide.panel.center(), [400.0, 300.0]);
    let small = layout(320.0, 180.0);
    assert!(small.scale < 1.0);
    assert!(small.panel.x >= 0.0 && small.panel.y >= 0.0);
    assert!(small.panel.x + small.panel.w <= 320.0);
    assert!(small.panel.y + small.panel.h <= 180.0);
    assert!((small.map.w / wide.map.w - small.scale).abs() < 0.0001);
    for rect in small.tabs { assert!(small.panel.contains(rect.x, rect.y)); }
}

#[test]
fn click_outside_panel_is_none() {
    let l = layout(800.0, 600.0);
    for tab in Tab::ALL {
        let state = TabletState { tab, ..Default::default() };
        for (x, y) in [(0.0, 0.0), (l.panel.x - 1.0, l.panel.y + 40.0),
            (l.panel.x + l.panel.w, l.panel.y + 40.0), (f32::NAN, 40.0)] {
            assert_eq!(hit(&l, &state, x, y), None);
        }
    }
}

#[test]
fn tabs_hit_in_order() {
    let l = layout(800.0, 600.0);
    for (tab, rect) in Tab::ALL.into_iter().zip(l.tabs) {
        let [x, y] = rect.center();
        assert_eq!(hit(&l, &TabletState::default(), x, y), Some(Hit::Tab(tab)));
    }
}

#[test]
fn map_click_maps_to_offset() {
    for (width, height) in [(800.0, 600.0), (320.0, 180.0)] {
        let l = layout(width, height);
        let mut state = TabletState { tab: Tab::Map, ..Default::default() };
        let [x, y] = l.map.center();
        assert_eq!(hit(&l, &state, x, y), Some(Hit::MapPoint { dx: 0.0, dz: 0.0 }));
        for zoom in [1, 2, 4] {
            state.zoom = zoom;
            let Some(Hit::MapPoint { dx, dz }) = hit(&l, &state, x + l.map.w / 4.0, y - l.map.h / 4.0) else { panic!("map hit missing") };
            assert!((dx - 64.0 * zoom as f64).abs() < 0.001);
            assert!((dz + 64.0 * zoom as f64).abs() < 0.001);
        }
    }
}

#[test]
fn weapon_rows_scroll() {
    for (width, height) in [(800.0, 600.0), (320.0, 180.0)] {
        let l = layout(width, height);
        let mut state = TabletState::default();
        let x = l.weapon_list.x + 10.0 * l.scale;
        let y = l.weapon_list.y + 5.0 * l.scale;
        assert_eq!(hit(&l, &state, x, y), Some(Hit::WeaponRow(0)));
        state.scroll = 60.0;
        assert_eq!(hit(&l, &state, x, y), Some(Hit::WeaponRow(2)));
        let row = l.weapon_row(2, state.scroll);
        assert!((row.y - l.weapon_list.y).abs() < 0.001);
        state.scroll = -10.0;
        assert_eq!(hit(&l, &state, x, y), Some(Hit::WeaponRow(0)));
    }
}

#[test]
fn status_lang_hit_in_order() {
    let l = layout(800.0, 600.0);
    let state = TabletState { tab: Tab::Status, ..Default::default() };
    for i in 0..3 {
        let [x, y] = l.status_lang[i].center();
        assert_eq!(hit(&l, &state, x, y), Some(Hit::StatusLang(i)));
    }
}
