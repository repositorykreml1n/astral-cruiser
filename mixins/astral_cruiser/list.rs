// The mod's row in Options > Mods, with every key it listens to.
fn astral_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>, game: &mut crate::game::Game<'_>) {
    use crate::game::astral_cruiser as m;
    use crate::game::astral_lang::{resolve, Lang};
    let russian = resolve(m::saved_lang_setting(), &game.settings.language) == Lang::Ru;
    let key = |name: &str, english: &str, ours: &str, default: &str| {
        [name.to_string(), (if russian { ours } else { english }).to_string(), default.to_string()]
    };
    let mut keys = vec![
        key(m::SEAT_KEY, "Sit down or stand up", "Сесть или встать", m::SEAT_KEY_DEFAULT),
        key(m::PANEL_KEY, "Control panel", "Панель управления", m::PANEL_KEY_DEFAULT),
        key(m::STOP_KEY, "Stop the manoeuvre", "Прервать манёвр", m::STOP_KEY_DEFAULT),
        key(m::HELP_KEY, "Tutorial", "Обучение", m::HELP_KEY_DEFAULT),
        key(m::ORBIT_KEY, "Orbit and strikes", "Орбита и удары", m::ORBIT_KEY_DEFAULT),
        key(m::HUD_KEY, "Show or hide the strike panel", "Показать или скрыть панель ударов", m::HUD_KEY_DEFAULT),
        key(m::WHEEL_KEY, "Weapon wheel", "Колесо оружия", m::WHEEL_KEY_DEFAULT),
        key(m::CALL_KEY, "Call the ship", "Вызвать корабль", m::CALL_KEY_DEFAULT),
        key(m::BOARD_KEY, "Board the ship", "Подняться на борт", m::BOARD_KEY_DEFAULT),
    ];
    keys.extend(m::GUN_KEYS.iter().map(|&(name, english, ours, default)| key(name, english, ours, default)));
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("{} {}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys,
    });
}
