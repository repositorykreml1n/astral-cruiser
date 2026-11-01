// The ship's control panel, as one of the game's own dialog screens. The game
// draws its fields, sliders and buttons; a button simply runs an /astral command
// with the values filled in. This only writes the dialog down.
use super::astral_core::Dimension;

/// Where the player stands with regard to the ship.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    NoShip,
    /// Outside it, or with the ship in another dimension.
    Outside,
    Aboard,
}

/// What the panel shows.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelState {
    pub russian: bool,
    pub place: Place,
    pub status: String,
    pub dimension: Dimension,
    pub target: [f64; 3],
    pub cruise: f64,
    pub dash: f64,
    pub blasting: bool,
    /// A dash, jump or portal is under way, and can be called off.
    pub busy: bool,
    pub portal_open: bool,
}

const COORDINATE_LENGTH: u32 = 16;
const CRUISE_SLIDER: (f64, f64, f64) = (16.0, 256.0, 8.0);
const DASH_SLIDER: (f64, f64, f64) = (200.0, 4000.0, 100.0);
const WIDE: u32 = 220;
const BUTTON: u32 = 104;

pub fn quoted(text: &str) -> String {
    let inner: String = text
        .chars()
        .flat_map(|c| match c {
            '"' | '\\' => vec!['\\', c],
            c => vec![c],
        })
        .collect();
    format!("\"{inner}\"")
}

fn button(label: &str, action: String) -> String {
    format!("{{label:{},width:{BUTTON},action:{action}}}", quoted(label))
}

/// A button that runs a command as it stands.
fn runs(label: &str, command: &str) -> String {
    button(label, format!("{{type:\"run_command\",command:{}}}", quoted(command)))
}

/// A button that runs a command with the values of the inputs filled in.
fn fills_in(label: &str, template: &str) -> String {
    let template = quoted(template);
    button(label, format!("{{type:\"dynamic/run_command\",template:{template}}}"))
}

fn field(key: &str, label: &str, value: f64) -> String {
    format!(
        "{{type:\"text\",key:{},label:{},width:{WIDE},max_length:{COORDINATE_LENGTH},initial:{}}}",
        quoted(key),
        quoted(label),
        quoted(&format!("{value:.0}")),
    )
}

fn slider(key: &str, label: &str, (start, end, step): (f64, f64, f64), value: f64) -> String {
    format!(
        "{{type:\"number_range\",key:{},label:{},width:{WIDE},start:{start},end:{end},step:{step},initial:{}}}",
        quoted(key),
        quoted(label),
        value.clamp(start, end),
    )
}

fn portal_buttons(state: &PanelState, say: &dyn Fn(&'static str, &'static str) -> &'static str) -> Vec<String> {
    let all = [
        (Dimension::Overworld, say("Portal: Overworld", "Портал: Мир"), "astral portal overworld"),
        (Dimension::Nether, say("Nether: find room", "Незер: поиск места"), "astral portal nether"),
        (Dimension::Nether, say("Nether: the roof", "Незер: крыша"), "astral portal nether roof"),
        (Dimension::End, say("Portal: End", "Портал: Энд"), "astral portal end"),
    ];
    all.iter()
        .filter(|(dimension, ..)| *dimension != state.dimension)
        .map(|(_, label, command)| runs(label, command))
        .collect()
}

/// The dialog, in the game's text form for tags.
pub fn dialog(state: &PanelState) -> String {
    let russian = state.russian;
    let say = move |english: &'static str, ours: &'static str| if russian { ours } else { english };
    let mut inputs = Vec::new();
    let mut actions = Vec::new();
    match state.place {
        Place::NoShip => actions.push(runs(say("Call the ship", "Вызвать корабль"), "astral spawn")),
        Place::Outside => {
            actions.push(runs(say("Board", "На борт"), "astral board"));
            actions.push(runs(say("Dismiss the ship", "Убрать корабль"), "astral remove"));
        }
        Place::Aboard => {
            let [x, y, z] = state.target;
            inputs.push(field("x", "X", x));
            inputs.push(field("y", "Y", y));
            inputs.push(field("z", "Z", z));
            inputs.push(slider("cruise", say("Flight speed", "Скорость полёта"), CRUISE_SLIDER, state.cruise));
            inputs.push(slider("dash", say("Dash speed", "Скорость рывка"), DASH_SLIDER, state.dash));
            let go = |how: &str| format!("astral panel {how} $(x) $(y) $(z) $(cruise) $(dash)");
            actions.push(fills_in(say("Jump", "Прыжок"), &go("jump")));
            actions.push(fills_in(say("Dash", "Рывок"), &go("dash")));
            actions.push(fills_in(say("Apply speeds", "Скорости"), "astral speed $(cruise) $(dash)"));
            if state.portal_open {
                actions.push(runs(say("Through the portal", "В портал"), "astral portal go"));
            }
            if state.busy {
                actions.push(runs(say("Stop", "Стоп"), "astral stop"));
            } else {
                actions.extend(portal_buttons(state, &say));
            }
            let blast = if state.blasting {
                runs(say("Blasting: on", "Разрушение: вкл"), "astral blast off")
            } else {
                runs(say("Blasting: off", "Разрушение: выкл"), "astral blast on")
            };
            actions.push(blast);
            actions.push(runs(say("Controls / bridge", "Штурвал / мостик"), "astral walk"));
            actions.push(runs(say("Leave the ship", "Выйти"), "astral leave"));
        }
    }
    actions.push(runs(say("Tutorial", "Обучение"), "astral tutorial"));
    let title = quoted(say("Astral Cruiser: control panel", "Astral Cruiser: панель управления"));
    let close = quoted(say("Close", "Закрыть"));
    format!(
        "{{type:\"minecraft:multi_action\",title:{title},pause:0b,columns:3,\
         body:{{type:\"plain_message\",contents:{},width:320}},inputs:[{}],actions:[{}],\
         exit_action:{{label:{close},width:{BUTTON}}}}}",
        quoted(&state.status),
        inputs.join(","),
        actions.join(","),
    )
}
