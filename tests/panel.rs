#[path = "../mixins/astral_cruiser/core.rs"]
mod astral_core;
#[path = "../mixins/astral_cruiser/panel.rs"]
mod astral_panel;

use astral_core::Dimension;
use astral_panel::{PanelState, Place, dialog};

fn aboard() -> PanelState {
    PanelState {
        russian: true,
        place: Place::Aboard,
        status: "Overworld at 10 90 -20".to_string(),
        dimension: Dimension::Overworld,
        target: [1200.0, 150.0, -340.0],
        cruise: 64.0,
        dash: 800.0,
        blasting: false,
        busy: false,
        portal_open: false,
    }
}

#[test]
fn jump_and_dash_take_the_coordinates_and_the_speeds() {
    let text = dialog(&aboard());
    assert!(text.contains("astral panel jump $(x) $(y) $(z) $(cruise) $(dash)"), "{text}");
    assert!(text.contains("astral panel dash $(x) $(y) $(z) $(cruise) $(dash)"), "{text}");
}

#[test]
fn the_fields_start_from_the_last_target() {
    let text = dialog(&aboard());
    assert!(text.contains("key:\"x\",label:\"X\""), "{text}");
    assert!(text.contains("initial:\"1200\"") && text.contains("initial:\"-340\""), "{text}");
}

#[test]
fn the_sliders_start_from_the_ships_speeds() {
    let text = dialog(&PanelState { cruise: 128.0, dash: 2000.0, ..aboard() });
    assert!(text.contains("initial:128") && text.contains("initial:2000"), "{text}");
}

#[test]
fn the_labels_follow_the_language() {
    assert!(dialog(&aboard()).contains("Прыжок"));
    assert!(dialog(&PanelState { russian: false, ..aboard() }).contains("\"Jump\""));
}

#[test]
fn no_portal_is_offered_to_where_the_ship_already_is() {
    let text = dialog(&aboard());
    assert!(!text.contains("astral portal overworld"), "{text}");
    assert!(text.contains("astral portal nether") && text.contains("astral portal end"), "{text}");
}

#[test]
fn something_under_way_can_be_stopped_and_starts_nothing_new() {
    let text = dialog(&PanelState { busy: true, ..aboard() });
    assert!(text.contains("astral stop"), "{text}");
    assert!(!text.contains("astral portal nether"), "{text}");
    assert!(!dialog(&aboard()).contains("astral stop"));
}

#[test]
fn an_open_portal_can_be_flown_through_from_the_panel() {
    let text = dialog(&PanelState { portal_open: true, busy: true, ..aboard() });
    assert!(text.contains("astral portal go"), "{text}");
}

#[test]
fn blasting_shows_as_it_stands_and_flips() {
    assert!(dialog(&aboard()).contains("astral blast on"));
    assert!(dialog(&PanelState { blasting: true, ..aboard() }).contains("astral blast off"));
}

#[test]
fn with_no_ship_the_panel_calls_one() {
    let text = dialog(&PanelState { place: Place::NoShip, ..aboard() });
    assert!(text.contains("astral spawn"), "{text}");
    assert!(!text.contains("astral panel jump"), "{text}");
}

#[test]
fn outside_the_panel_takes_you_aboard() {
    let text = dialog(&PanelState { place: Place::Outside, ..aboard() });
    assert!(text.contains("astral board") && text.contains("astral remove"), "{text}");
    assert!(!text.contains("astral panel jump"), "{text}");
}

#[test]
fn quotes_in_the_status_cannot_break_the_dialog() {
    let text = dialog(&PanelState { status: "a \"b\" \\ c".to_string(), ..aboard() });
    assert!(text.contains("\"a \\\"b\\\" \\\\ c\""), "{text}");
}

#[test]
fn the_nether_has_two_ways_out() {
    let text = dialog(&aboard());
    assert!(text.contains("command:\"astral portal nether\""), "{text}");
    assert!(text.contains("command:\"astral portal nether roof\""), "{text}");
}

#[test]
fn from_the_nether_itself_neither_is_offered() {
    let text = dialog(&PanelState { dimension: Dimension::Nether, ..aboard() });
    assert!(!text.contains("astral portal nether"), "{text}");
    assert!(text.contains("astral portal overworld"), "{text}");
}

#[test]
fn the_tutorial_is_a_button_away_wherever_the_player_stands() {
    for place in [Place::NoShip, Place::Outside, Place::Aboard] {
        assert!(dialog(&PanelState { place, ..aboard() }).contains("astral tutorial"));
    }
}

#[test]
fn the_ships_own_view_is_not_on_the_panel() {
    assert!(!dialog(&aboard()).contains("astral view"));
}
