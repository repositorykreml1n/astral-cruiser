#[path = "../mixins/astral_cruiser/core.rs"]
mod astral_core;
#[path = "../mixins/astral_cruiser/panel.rs"]
mod astral_panel;
#[path = "../mixins/astral_cruiser/tutorial.rs"]
mod astral_tutorial;

use astral_tutorial::{Keys, PAGES, dialog, key_name};

fn keys() -> Keys {
    Keys {
        seat: "V".to_string(),
        panel: "J".to_string(),
        guns: ["1", "2", "3", "4"].map(str::to_string),
        stop: "B".to_string(),
        help: "H".to_string(),
    }
}

#[test]
fn a_key_is_shown_as_the_player_knows_it() {
    assert_eq!(key_name("key.keyboard.n"), "N");
    assert_eq!(key_name("key.keyboard.left.shift"), "LEFT SHIFT");
    assert_eq!(key_name("key.mouse.4"), "MOUSE 4");
}

#[test]
fn the_pages_lead_from_one_to_the_next() {
    let first = dialog(0, true, &keys());
    assert!(first.contains("astral tutorial 1"), "{first}");
    assert!(!first.contains("astral tutorial -"), "{first}");
    let last = dialog(PAGES - 1, true, &keys());
    assert!(last.contains(&format!("astral tutorial {}", PAGES - 2)), "{last}");
    assert!(!last.contains(&format!("astral tutorial {PAGES}")), "{last}");
}

#[test]
fn the_text_names_the_players_own_keys() {
    let text: String = (0..PAGES).map(|page| dialog(page, true, &keys())).collect();
    assert!(text.contains("[J]"), "{text}");
}

#[test]
fn it_says_where_the_keys_are_changed() {
    let text: String = (0..PAGES).map(|page| dialog(page, true, &keys())).collect();
    assert!(text.contains("Настройки"), "{text}");
}

#[test]
fn it_follows_the_language() {
    assert!(dialog(0, true, &keys()).contains("Далее"));
    assert!(dialog(0, false, &keys()).contains("Next"));
}

#[test]
fn a_page_past_the_end_is_the_last_one() {
    assert_eq!(dialog(99, true, &keys()), dialog(PAGES - 1, true, &keys()));
}

#[test]
fn it_tells_of_the_ships_own_view() {
    assert!(dialog(1, true, &keys()).contains("от лица корабля"));
}
