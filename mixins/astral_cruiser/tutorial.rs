// A short tutorial, as pages of the game's own dialog screen. Its buttons run
// /astral tutorial N; the text names the keys the player has actually chosen.
use super::astral_panel::quoted;

pub const PAGES: usize = 5;

const WIDTH: u32 = 320;
const BUTTON: u32 = 104;

/// The player's keys, as they should read.
pub struct Keys {
    pub seat: String,
    pub panel: String,
    pub guns: [String; 4],
    pub stop: String,
    pub help: String,
}

/// `key.keyboard.left.shift` as `LEFT SHIFT`.
pub fn key_name(key: &str) -> String {
    let name = key.strip_prefix("key.keyboard.").or(key.strip_prefix("key.")).unwrap_or(key);
    name.replace('.', " ").to_uppercase()
}

fn page_text(page: usize, russian: bool, k: &Keys) -> (&'static str, String) {
    let (panel, seat, stop, help) = (&k.panel, &k.seat, &k.stop, &k.help);
    let [one, two, three, four] = &k.guns;
    match (page, russian) {
        (0, true) => ("Корабль", format!(
            "Всё управление собрано в панели: клавиша [{panel}]. Если корабля нет, вызовите его кнопкой в панели, затем поднимитесь на борт. Вернуться к обучению можно клавишей [{help}]."
        )),
        (0, false) => ("The ship", format!(
            "Everything is on the panel: press [{panel}]. With no ship about, call one from the panel, then go aboard. Press [{help}] to come back to this tutorial."
        )),
        (1, true) => ("Полёт", format!(
            "Управление как при ходьбе: вперёд, назад, вбок, прыжок и присед поднимают и опускают корабль, бег даёт ускорение. Нос следует за взглядом. [{seat}] встать из кресла и ходить внутри корабля, F5 переключает вид: мостик, от лица корабля, сзади, спереди."
        )),
        (1, false) => ("Flying", format!(
            "The walking keys fly it: forward, back, sideways, jump and sneak climb and dive, sprint boosts. The nose follows your view. [{seat}] gets up to walk around inside, F5 goes through the views: the bridge, the ship's own, behind, ahead."
        )),
        (2, true) => ("Орудия", format!(
            "Левая кнопка мыши стреляет. [{one}] турели, [{two}] носовые, [{three}] брюшной луч, [{four}] главный луч; колесо мыши листает их. Разрушение блоков включается в панели, по умолчанию оно выключено."
        )),
        (2, false) => ("Guns", format!(
            "Left mouse fires. [{one}] turrets, [{two}] nose guns, [{three}] belly beam, [{four}] main beam; the wheel goes through them. Blasting of blocks is switched on the panel, and starts off."
        )),
        (3, true) => ("Дальние перелёты", format!(
            "Введите координаты в панели. Рывок летит над миром, прыжок заряжается и телепортирует почти к цели. Скорости задаются ползунками. [{stop}] прерывает манёвр."
        )),
        (3, false) => ("Long trips", format!(
            "Type coordinates on the panel. A dash flies over the world, a jump charges up and teleports nearly there. The sliders set the speeds. [{stop}] calls it off."
        )),
        (_, true) => ("Порталы", "Портал показывает настоящее другое измерение, просто влети в него носом. Пока он открыт, можно летать и выходить, но если отлететь далеко, он закроется. В Незере корпус почти нигде не помещается в пещерах, поэтому поиск места обычно выводит на крышу. Клавиши меняются в Настройки > Моды > Astral Cruiser.".to_string()),
        (_, false) => ("Portals", "A portal shows the real far dimension: fly in nose first. While it stands you may fly about or go out, but go too far and it shuts. The hull hardly ever fits the Nether's caves, so looking for room usually ends on the roof. Change the keys in Options > Mods > Astral Cruiser.".to_string()),
    }
}

fn goes_to(label: &str, page: usize) -> String {
    let command = quoted(&format!("astral tutorial {page}"));
    format!("{{label:{},width:{BUTTON},action:{{type:\"run_command\",command:{command}}}}}", quoted(label))
}

/// A page, in the game's text form for tags.
pub fn dialog(page: usize, russian: bool, keys: &Keys) -> String {
    let page = page.min(PAGES - 1);
    let (title, text) = page_text(page, russian, keys);
    let say = |english, ours| if russian { ours } else { english };
    let mut actions = Vec::new();
    if page > 0 {
        actions.push(goes_to(say("Back", "Назад"), page - 1));
    }
    if page + 1 < PAGES {
        actions.push(goes_to(say("Next", "Далее"), page + 1));
    }
    let heading = quoted(&format!("Astral Cruiser: {title} ({}/{PAGES})", page + 1));
    let close = quoted(say("Close", "Закрыть"));
    format!(
        "{{type:\"minecraft:multi_action\",title:{heading},pause:0b,columns:2,\
         body:{{type:\"plain_message\",contents:{},width:{WIDTH}}},actions:[{}],\
         exit_action:{{label:{close},width:{BUTTON}}}}}",
        quoted(&text),
        actions.join(","),
    )
}
