use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    Ru,
    En,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LangSetting {
    #[default]
    Auto,
    Ru,
    En,
}

impl LangSetting {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Ru => "ru",
            Self::En => "en",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().trim() {
            "ru" => Self::Ru,
            "en" => Self::En,
            _ => Self::Auto,
        }
    }
}

pub fn resolve(setting: LangSetting, game_language: &str) -> Lang {
    match setting {
        LangSetting::Auto => {
            if game_language.starts_with("ru") {
                Lang::Ru
            } else {
                Lang::En
            }
        }
        LangSetting::Ru => Lang::Ru,
        LangSetting::En => Lang::En,
    }
}

pub static STRINGS: &[(&str, &str, &str)] = &[
    ("waypoint.default_name", "Точка {}", "Waypoint {}"),
    // Orbit HUD
    ("ORBITAL ARSENAL", "ОРБИТАЛЬНЫЙ АРСЕНАЛ", "ORBITAL ARSENAL"),
    ("READY", "ГОТОВО", "READY"),
    ("ENERGY", "ЭНЕРГИЯ", "ENERGY"),
    ("COST", "РАСХОД", "COST"),
    ("TARGET", "ЦЕЛЬ", "TARGET"),
    ("AWAITING STRIKE", "ОЖИДАНИЕ УДАРА", "AWAITING STRIKE"),
    ("CLIMBING TO ORBIT", "ВЫХОД НА ОРБИТУ", "CLIMBING TO ORBIT"),
    ("NO TARGET", "НЕТ ЦЕЛИ", "NO TARGET"),
    ("OUT OF RANGE", "ВНЕ ЗОНЫ", "OUT OF RANGE"),
    ("ACTIVE", "АКТИВНО", "ACTIVE"),
    ("orbit.legend", "[O] покинуть  [1-4]/[Shift+колесо] удары  [ЛКМ] огонь  [колесо] зум  [{}] панель", "[O] leave  [1-4]/[Shift+wheel] strike  [LMB] fire  [wheel] zoom  [{}] panel"),
    ("orbit.cancel_climb", "[O] отменить подъём", "[O] cancel climb"),
    ("orbit.alt_blocks", "ВЫС {:.0} / {:.0} блоков", "ALT {:.0} / {:.0} blocks"),
    ("orbit.blocks", "блоков", "blocks"),
    ("orbit.cost_per_sec", "РАСХОД {:.0}/с", "COST {:.0}/s"),
    ("orbit.cost", "РАСХОД {:.0}", "COST {:.0}"),
    ("orbit.energy", "ЭНЕРГИЯ {:.0}/{:.0}", "ENERGY {:.0}/{:.0}"),

    // Flight HUD
    ("Turrets", "Турели", "Turrets"),
    ("Nose guns", "Носовые орудия", "Nose guns"),
    ("Belly beam", "Брюшной луч", "Belly beam"),
    ("Main beam", "Главный луч", "Main beam"),
    ("energy", "энергия", "energy"),
    ("heat", "нагрев", "heat"),
    ("overheated", "перегрев", "overheated"),
    ("charge", "заряд", "charge"),

    // Bottom Hints
    ("hint.board", "[{}] Подняться на борт", "[{}] Board the ship"),
    ("hint.call", "[{}] Вызвать корабль", "[{}] Call the ship"),
    ("hint.seat", "[{}] Сесть за штурвал", "[{}] Take the helm"),

    // Onboarding Card
    ("onboarding.title", "ОБУЧЕНИЕ", "TUTORIAL"),
    ("onboarding.hide", "скрыть", "hide"),
    ("onboarding.done", "Обучение пройдено", "Tutorial completed"),
    ("onboarding.call_ship", "Вызвать корабль", "Call the ship"),
    ("onboarding.board", "Подняться на борт", "Board the ship"),
    ("onboarding.seat", "Сесть за штурвал", "Take the helm"),
    ("onboarding.fly", "Полёт", "Flight"),
    ("onboarding.wheel", "Колесо оружия", "Weapon wheel"),
    ("onboarding.fire", "Выстрел", "Fire"),
    ("onboarding.orbit", "Орбита", "Orbit"),
    ("onboarding.strike", "Удар с орбиты", "Orbital strike"),
    ("onboarding.tablet", "Планшет", "Tablet"),
    ("onboarding.lmb", "ЛКМ", "LMB"),
    ("onboarding.lmb_orbit", "ЛКМ на орбите", "LMB in orbit"),

    // Weapon Wheel
    ("wheel.lasers", "Лазеры", "Lasers"),
    ("wheel.classic", "Классика", "Classic"),
    ("wheel.super", "Супер", "Super"),
    ("wheel.zone", "Зона", "Zone"),
    ("wheel.special", "Особые", "Special"),
    ("wheel.precise", "Точные", "Precise"),
    ("wheel.hover_placeholder", "Наведите на оружие", "Hover over a weapon"),
    ("wheel.options", "вариантов", "options"),
    ("wheel.laser_energy", "ГОТОВО • Энергия корабля", "READY • Ship energy"),
    ("wheel.cd", "КД", "CD"),
    ("wheel.cost", "Расход", "Cost"),
    ("wheel.cost_per_s", "Расход {:.0}/с", "Cost {:.0}/s"),
    ("wheel.cost_once", "Расход {:.0}", "Cost {:.0}"),
    ("wheel.ready", "ГОТОВО", "READY"),
    ("blurb.turrets", "Автоматические зенитные турели", "Automated anti-air turrets"),
    ("blurb.nose", "Сдвоенные курсовые импульсные пушки", "Dual forward pulse cannons"),
    ("blurb.belly", "Лучевой резак нижней полусферы", "Ventral beam cutter"),
    ("blurb.main", "Сверхмощный носовой луч", "High-power forward beam"),

    // Tablet
    ("tablet.title", "АСТРАЛЬНЫЙ КРЕЙСЕР // ТАКТИКА", "ASTRAL CRUISER // TACTICAL"),
    ("tablet.tab.weapons", "Оружие", "Weapons"),
    ("tablet.tab.map", "Карта", "Map"),
    ("tablet.tab.navigation", "Навигация", "Navigation"),
    ("tablet.tab.status", "Состояние", "Status"),
    ("tablet.en_short", "ЭН", "EN"),
    ("tablet.sec_short", "с", "s"),
    ("tablet.group_short", "Г", "G"),
    ("tablet.fire", "ОГОНЬ", "FIRE"),
    ("tablet.dash", "РЫВОК", "DASH"),
    ("tablet.radius", "Радиус", "Radius"),
    ("tablet.waves", "Волн", "Waves"),
    ("tablet.energy", "Энергия", "Energy"),
    ("tablet.target_x", "X цели", "Target X"),
    ("tablet.target_z", "Z цели", "Target Z"),
    ("tablet.target_mode_dash", "Цель: рывок", "Target: dash"),
    ("tablet.target_mode_strike", "Цель: удар", "Target: strike"),
    ("tablet.blocks_per_px", "бл/пикс", "b/px"),
    ("tablet.save_here", "Сохранить здесь", "Save here"),
    ("tablet.fly", "Лететь", "Fly"),
    ("tablet.delete", "Удалить", "Delete"),
    ("tablet.open_portal_to", "Открыть портал в измерение:", "Open portal to dimension:"),
    ("tablet.dim.overworld", "Мир", "Overworld"),
    ("tablet.dim.nether", "Незер", "Nether"),
    ("tablet.dim.end", "Энд", "End"),
    ("tablet.speed", "Скорость", "Speed"),
    ("tablet.altitude", "Высота", "Altitude"),
    ("tablet.orbit", "Орбита", "Orbit"),
    ("tablet.pilot", "Пилот", "Pilot"),
    ("tablet.btn.call", "Вызвать корабль", "Call ship"),
    ("tablet.btn.board", "Подняться на борт", "Board"),
    ("tablet.btn.seat", "Сесть за штурвал", "Take helm"),
    ("tablet.btn.leave", "Покинуть корабль", "Leave ship"),
    ("tablet.btn.reset_onboarding", "Сбросить обучение", "Reset tutorial"),
    ("tablet.shake", "Тряска", "Screen shake"),
    ("tablet.volume", "Громкость взрывов", "Explosion volume"),
    ("tablet.lang_label", "Язык:", "Language:"),
    ("tablet.lang.auto", "Авто", "Auto"),
    ("tablet.lang.ru", "Русский", "Русский"),
    ("tablet.lang.en", "English", "English"),
    ("tablet.error.coords", "Введите конечные координаты в пределах мира", "Enter finite coordinates within the world"),
    ("tablet.error.waypoint_dim", "Точка в другом измерении: сначала откройте портал.", "Waypoint is in another dimension: open a portal first."),
    ("tablet.save_error", "Планшет: не удалось сохранить настройки", "Tablet: failed to save settings"),
    ("tablet.waypoint_prefix", "Точка", "Waypoint"),

    // Chat Messages & Notices
    ("chat.welcome_helm", "WASD полёт, Пробел и Shift подъём и спуск, Ctrl форсаж, корабль следует за взглядом. ЛКМ огонь, 1-4 или колесо выбор оружия. N панель управления. F5 переключает вид, включая вид с корабля. V встать.", "WASD fly, Space and Shift climb and dive, Ctrl boosts, the ship follows your view. Left mouse fires, 1-4 or the wheel picks a weapon. N opens the control panel. F5 goes through the views, one of them the ship's own. V stands up."),
    ("chat.onboarding_reset", "обучение сброшено.", "tutorial reset."),
    ("chat.ship_arrived", "корабль прибыл к вам.", "the ship has arrived."),
    ("chat.ship_already_here", "корабль уже здесь.", "the ship is already here."),
    ("chat.ship_not_found", "корабль не найден.", "ship not found."),
    ("chat.ship_other_dim", "корабль в другом измерении.", "ship is in another dimension."),
    ("chat.ship_far_call", "корабль далеко — вызовите его клавишей вызова.", "ship is too far — call it with the call key."),
    ("chat.ship_nearby_board", "корабль рядом. [{}] — подняться на борт.", "ship is nearby. [{}] — board the ship."),
    ("chat.ship_removed", "корабль удалён.", "ship removed."),
    ("chat.only_helm", "только за штурвалом.", "only from the helm."),
    ("chat.not_aboard", "вы не на борту.", "you are not aboard."),
    ("chat.already_aboard", "вы уже на борту.", "you are already aboard."),
    ("chat.leave_first", "сначала покиньте корабль.", "leave the ship first."),
    ("chat.no_ship_spawn", "здесь нет корабля. Используйте /astral spawn.", "no ship here. Use /astral spawn."),
    ("chat.door_far", "слишком далеко от двери шлюза.", "too far from the airlock door."),
    ("chat.not_during_dash", "нельзя во время рывка. /astral dash cancel останавливает его.", "not during a dash. /astral dash cancel stops it."),
    ("chat.not_now_stop", "не сейчас. /astral stop отменяет это.", "not now. /astral stop calls it off."),
    ("chat.board_first", "сначала поднимитесь на борт.", "board the ship first."),
    ("chat.no_target", "нет цели под прицелом.", "no target in sight."),
    ("chat.outer_door_blocked", "внешний люк заблокирован.", "something blocks the outer door."),
    ("chat.no_safe_ground", "под люком нет безопасной земли. Прилетите в другое место.", "no safe ground under the door. Fly somewhere else."),
    ("chat.beamed_down", "перемещены вниз. /astral enter возвращает на корабль рядом с люком.", "beamed down. /astral enter brings you back from near the door."),
    ("chat.singleplayer_only", "Astral Cruiser работает пока только в одиночной игре.", "Astral Cruiser works in single player only for now."),
    ("chat.blasting_is", "взрывы {}.", "blasting is {}."),
    ("chat.blasting_on", "включены: выстрелы разрушают блоки", "on: shots tear up the world"),
    ("chat.blasting_off", "выключены", "off"),
    ("refusal.busy", "удар уже наносится.", "a strike is already under way."),
    ("refusal.no_energy", "недостаточно энергии.", "not enough energy."),
    ("refusal.out_of_range", "слишком далеко: удары действуют на 384 блока от корабля.", "too far: strikes reach 384 blocks from the ship."),
    ("refusal.cooling", "перезарядка: {:.0} с.", "still cooling: {:.0} s."),
    ("notice.lance_hand", "резак наводится вручную с орбиты", "the lance is aimed by hand, from orbit"),
    ("notice.strike_coords", "{} по координатам {:.0} {:.0}.", "{} on {:.0} {:.0}."),
    ("notice.jump_called_off", "прыжок отменён.", "jump called off."),
    ("notice.left_orbit", "выход с орбиты; корабль удерживает высоту.", "left orbit; the ship holds its height."),
    ("notice.busy_action", "корабль занят рывком, прыжком или порталом.", "the ship is busy with a dash, a jump or a portal."),
    ("notice.other_dim", "корабль в другом измерении.", "the ship is in another dimension."),
    ("notice.no_jump", "прыжок невозможен: {}.", "no jump: {}."),
    ("notice.already_there", "корабль уже там.", "the ship is already there."),
    ("notice.jump_timeout", "прыжок отменён: пункт назначения загружался слишком долго.", "jump called off: the destination took too long to load."),
    ("notice.arrived_at", "прибыли в точку {:.1} {:.1} {:.1}.", "arrived at {:.1} {:.1} {:.1}."),
    ("notice.same_dim", "корабль уже в этом измерении.", "the ship is in that dimension already."),
    ("notice.portal_no_room", "нет места перед носом корабля для портала.", "no room ahead of the nose for a portal."),
    ("notice.portal_roof", "в пещерах здесь нет места для корпуса: портал открыт над крышей.", "no room for the hull in the caves here: the portal goes out over the roof."),
    ("notice.portal_far_no_room", "на той стороне нет места для корабля.", "no room for the ship on the far side."),
    ("notice.portal_closed_behind", "портал закрылся за вами.", "the portal closed behind you."),
    ("notice.portal_closed_far", "портал закрыт: корабль ушёл слишком далеко от него.", "portal closed: the ship went too far from it."),
    ("notice.portal_closed_timeout", "портал закрыт: пункт назначения загружался слишком долго.", "portal closed: the far side took too long to load."),
    ("notice.portal_through", "прошли сквозь портал.", "through the portal."),
    ("notice.portal_closed", "портал закрыт.", "portal closed."),
    ("notice.portal_too_late", "слишком поздно: корабль проходит через портал.", "too late: the ship is going through."),
    ("notice.no_portal_open", "нет открытого портала.", "no portal open."),
    ("notice.no_portal_ahead", "перед кораблём нет открытого портала.", "no portal stands open in front of the ship."),
    ("notice.dash_busy", "рывок уже выполняется. /astral dash cancel остановит его.", "a dash is already under way. /astral dash cancel stops it."),
    ("notice.ship_busy_prep", "корабль у портала или готовится к прыжку.", "the ship is at a portal or gathering a jump."),
    ("notice.no_dash", "рывок невозможен: {}.", "no dash: {}."),
    ("notice.dash_called_off", "рывок отменён.", "dash called off."),
    ("notice.braking", "торможение.", "braking."),
    ("notice.dash_dim", "рывок отменён: корабль в другом измерении.", "dash called off: the ship is in another dimension."),
    ("notice.dash_timeout", "рывок отменён: пункт назначения загружался слишком долго.", "dash called off: the destination took too long to load."),
    ("notice.dash_obstacle", "рывок остановлен: препятствие на пути.", "dash stopped: something is in the way."),
    ("notice.no_ship", "нет корабля.", "no ship."),
    ("notice.no_orbit_nether", "под крышей Незера орбита невозможна.", "no orbit under the Nether's roof."),
    ("notice.orbit_busy", "не сейчас: выполняется рывок, прыжок или портал.", "not now: a dash, jump or portal is under way."),
    ("notice.climbing_orbit", "подъём на орбиту.", "climbing to orbit."),
    ("notice.no_way_up", "нет прохода вверх: над кораблём препятствие.", "no way up: something stands over the ship."),
    ("notice.in_orbit_help", "на орбите. O выход, 1-4 выбор удара, ЛКМ огонь, колесо зум.", "in orbit. O leaves, 1-4 pick a strike, left mouse fires, the wheel zooms."),
    ("notice.no_dash_underway", "рывок не выполняется.", "no dash under way."),
    ("notice.nothing_to_stop", "нечего останавливать.", "nothing to stop."),
    ("chat.astral_help", "/astral spawn | board | walk | seat | enter | leave | go КОМНАТА | panel | tutorial | view | jump X Y Z | dash X Y Z | dash cancel | speed ПОЛЁТ РЫВОК | stop | portal overworld | portal nether | portal nether roof | portal end | portal go | portal cancel | blast on | blast off | cruise ТЯГА ПОВОРОТ | cruise off | remove | status. Комнаты: bridge corridor reactor guns cargo airlock cabins mess sickbay", "/astral spawn | board | walk | seat | enter | leave | go ROOM | panel | tutorial | view | jump X Y Z | dash X Y Z | dash cancel | speed FLIGHT DASH | stop | portal overworld | portal nether | portal nether roof | portal end | portal go | portal cancel | blast on | blast off | cruise THRUST TURN | cruise off | remove | status. Rooms: bridge corridor reactor guns cargo airlock cabins mess sickbay"),

    // Flight HUD status telemetry
    ("hud.dash_title", "Рывок к", "Dash to"),
    ("hud.jump_title", "Прыжок к", "Jump to"),
    ("hud.portal_title", "Портал в", "Portal to"),
    ("hud.stage_prep_dest", "подготовка назначения", "preparing the destination"),
    ("hud.stage_charging", "зарядка", "charging"),
    ("hud.stage_blocks_left", "блоков осталось", "blocks to go"),
    ("hud.stage_braking", "торможение", "braking"),
    ("hud.stage_arriving", "прибытие", "arriving"),
    ("hud.looking_for_room", "поиск места", "looking for room"),
    ("hud.prep_far_side", "подготовка той стороны", "preparing the far side"),
    ("hud.portal_open_hint", "открыт. Влетайте носом вперёд, или /astral portal go; /astral portal cancel закроет его", "open. Fly in nose first, or /astral portal go; /astral portal cancel closes it"),
    ("hud.going_through", "проход сквозь портал", "going through"),

    // Dimensions
    ("dim.overworld", "Обычный мир", "the Overworld"),
    ("dim.nether", "Незер", "the Nether"),
    ("dim.end", "Энд", "the End"),

    // Direct chat strings & refusals
    ("Astral Cruiser works in single player only for now.", "Astral Cruiser работает пока только в одиночной игре.", "Astral Cruiser works in single player only for now."),
    ("Astral Cruiser: no ship here. Use /astral spawn.", "Astral Cruiser: здесь нет корабля. Используйте /astral spawn.", "Astral Cruiser: no ship here. Use /astral spawn."),
    ("Astral Cruiser: the ship is in another dimension.", "Astral Cruiser: корабль в другом измерении.", "Astral Cruiser: the ship is in another dimension."),
    ("Astral Cruiser: leave the ship first.", "Astral Cruiser: сначала покиньте корабль.", "Astral Cruiser: leave the ship first."),
    ("Astral Cruiser: you are already aboard.", "Astral Cruiser: вы уже на борту.", "Astral Cruiser: you are already aboard."),
    ("Astral Cruiser: you are not aboard.", "Astral Cruiser: вы не на борту.", "Astral Cruiser: you are not aboard."),
    ("Astral Cruiser: too far from the airlock door.", "Astral Cruiser: слишком далеко от двери шлюза.", "Astral Cruiser: too far from the airlock door."),
    ("Astral Cruiser: not during a dash. /astral dash cancel stops it.", "Astral Cruiser: нельзя во время рывка. /astral dash cancel останавливает его.", "Astral Cruiser: not during a dash. /astral dash cancel stops it."),
    ("Astral Cruiser: not now. /astral stop calls it off.", "Astral Cruiser: не сейчас. /astral stop отменяет это.", "Astral Cruiser: not now. /astral stop calls it off."),
    ("Astral Cruiser: board the ship first.", "Astral Cruiser: сначала поднимитесь на борт.", "Astral Cruiser: board the ship first."),
    ("Astral Cruiser: ship removed.", "Astral Cruiser: корабль удалён.", "Astral Cruiser: ship removed."),
    ("Astral Cruiser: only from the helm.", "Astral Cruiser: только за штурвалом.", "Astral Cruiser: only from the helm."),

    // Direct notice strings
    ("no ship.", "нет корабля.", "no ship."),
    ("left orbit; the ship holds its height.", "выход с орбиты; корабль удерживает высоту.", "left orbit; the ship holds its height."),
    ("no orbit under the Nether's roof.", "под крышей Незера орбита невозможна.", "no orbit under the Nether's roof."),
    ("not now: a dash, jump or portal is under way.", "не сейчас: выполняется рывок, прыжок или портал.", "not now: a dash, jump or portal is under way."),
    ("climbing to orbit.", "подъём на орбиту.", "climbing to orbit."),
    ("no way up: something stands over the ship.", "нет прохода вверх: над кораблём препятствие.", "no way up: something stands over the ship."),
    ("in orbit. O leaves, 1-4 pick a strike, left mouse fires, the wheel zooms.", "на орбите. O выход, 1-4 выбор удара, ЛКМ огонь, колесо зум.", "in orbit. O leaves, 1-4 pick a strike, left mouse fires, the wheel zooms."),
    ("the portal closed behind you.", "портал закрылся за вами.", "the portal closed behind you."),
    ("portal closed.", "портал закрыт.", "portal closed."),
    ("the ship is busy with a dash, a jump or a portal.", "корабль занят рывком, прыжком или порталом.", "the ship is busy with a dash, a jump or a portal."),
    ("the ship is in another dimension.", "корабль в другом измерении.", "the ship is in another dimension."),
    ("the ship is already there.", "корабль уже там.", "the ship is already there."),
    ("jump called off: the destination took too long to load.", "прыжок отменён: пункт назначения загружался слишком долго.", "jump called off: the destination took too long to load."),
    ("the ship is in that dimension already.", "корабль уже в этом измерении.", "the ship is in that dimension already."),
    ("no room ahead of the nose for a portal.", "нет места перед носом корабля для портала.", "no room ahead of the nose for a portal."),
    ("no room for the hull in the caves here: the portal goes out over the roof.", "в пещерах здесь нет места для корпуса: портал открыт над крышей.", "no room for the hull in the caves here: the portal goes out over the roof.") ,
    ("no room for the ship on the far side.", "на той стороне нет места для корабля.", "no room for the ship on the far side."),
    ("portal closed: the ship went too far from it.", "портал закрыт: корабль ушёл слишком далеко от него.", "portal closed: the ship went too far from it."),
    ("portal closed: the far side took too long to load.", "портал закрыт: пункт назначения загружался слишком долго.", "portal closed: the far side took too long to load."),
    ("through the portal.", "прошли сквозь портал.", "through the portal."),
    ("a dash is already under way. /astral dash cancel stops it.", "рывок уже выполняется. /astral dash cancel остановит его.", "a dash is already under way. /astral dash cancel stops it."),
    ("the ship is at a portal or gathering a jump.", "корабль у портала или готовится к прыжку.", "the ship is at a portal or gathering a jump."),
    ("dash called off.", "рывок отменён.", "dash called off."),
    ("braking.", "торможение.", "braking."),
    ("dash called off: the ship is in another dimension.", "рывок отменён: корабль в другом измерении.", "dash called off: the ship is in another dimension."),
    ("dash called off: the destination took too long to load.", "рывок отменён: пункт назначения загружался слишком долго.", "dash called off: the destination took too long to load."),
    ("dash stopped: something is in the way.", "рывок остановлен: препятствие на пути.", "dash stopped: something is in the way."),
    ("the lance is aimed by hand, from orbit", "резак наводится вручную с орбиты", "the lance is aimed by hand, from orbit"),
    ("jump called off.", "прыжок отменён.", "jump called off."),
    ("nothing to stop.", "нечего останавливать.", "nothing to stop."),
    ("no dash under way.", "рывок не выполняется.", "no dash under way."),
    ("too late: the ship is going through.", "слишком поздно: корабль проходит через портал.", "too late: the ship is going through."),
    ("no portal open.", "нет открытого портала.", "no portal open."),
    ("no portal stands open in front of the ship.", "перед кораблём нет открытого портала.", "no portal stands open in front of the ship."),

    // Onboarding direct titles
    ("Вызвать корабль", "Вызвать корабль", "Call the ship"),
    ("Подняться на борт", "Подняться на борт", "Board the ship"),
    ("Сесть за штурвал", "Сесть за штурвал", "Take the helm"),
    ("Полёт", "Полёт", "Flight"),
    ("Колесо оружия", "Колесо оружия", "Weapon wheel"),
    ("Выстрел", "Выстрел", "Fire"),
    ("Орбита", "Орбита", "Orbit"),
    ("Удар с орбиты", "Удар с орбиты", "Orbital strike"),
    ("Планшет", "Планшет", "Tablet"),
    ("ОБУЧЕНИЕ", "ОБУЧЕНИЕ", "TUTORIAL"),
    ("скрыть", "скрыть", "hide"),
    ("✔ Обучение пройдено", "✔ Обучение пройдено", "✔ Tutorial completed"),
    ("ЛКМ", "ЛКМ", "LMB"),
    ("ЛКМ на орбите", "ЛКМ на орбите", "LMB in orbit"),

    // Wheel sectors
    ("Лазеры", "Лазеры", "Lasers"),
    ("Классика", "Классика", "Classic"),
    ("Супер", "Супер", "Super"),
    ("Зона", "Зона", "Zone"),
    ("Особые", "Особые", "Special"),
    ("Точные", "Точные", "Precise"),

    // Weapon Names & Blurbs
    ("Ионный луч", "Ионный луч", "Ion beam"),
    ("Тяжёлый ионный луч длительного действия", "Тяжёлый ионный луч длительного действия", "Heavy sustained ion beam"),
    ("Залп", "Залп", "Barrage"),
    ("Артиллерийский залп 24 тяжёлыми снарядами", "Артиллерийский залп 24 тяжёлыми снарядами", "Artillery barrage of 24 heavy shells"),
    ("Кинетический стержень", "Кинетический стержень", "Kinetic rod"),
    ("Вольфрамовый стержень колоссальной кинетической силы", "Вольфрамовый стержень колоссальной кинетической силы", "Tungsten kinetic strike of colossal force"),
    ("Резак", "Резак", "Lance"),
    ("Высокоточный непрерывный режущий луч", "Высокоточный непрерывный режущий луч", "Precision continuous cutting beam"),
    ("Антиматерия", "Антиматерия", "Antimatter"),
    ("Взрыв колоссальной мощности с чашей и огненным грибом", "Взрыв колоссальной мощности с чашей и огненным грибом", "Colossal explosion with crater bowl and mushroom cloud"),
    ("Солнечный столп", "Солнечный столп", "Solar pillar"),
    ("Огненный столп до самого бедрока с оплавленным краем", "Огненный столп до самого бедрока с оплавленным краем", "Fiery pillar down to bedrock with fused rim"),
    ("Тектонический молот", "Тектонический молот", "Tectonic hammer"),
    ("Мощный подземный удар, раскалывающий породу разломами", "Мощный подземный удар, раскалывающий породу разломами", "Subterranean impact shattering rock with fissures"),
    ("Метеоритный дождь", "Метеоритный дождь", "Meteor shower"),
    ("Бомбардировка 40 метеорами по площади", "Бомбардировка 40 метеорами по площади", "Area bombardment of 40 meteors"),
    ("Кассетный рой", "Кассетный рой", "Cluster swarm"),
    ("Сетка из 60 кассетных суббоеприпасов", "Сетка из 60 кассетных суббоеприпасов", "Grid of 60 cluster submunitions"),
    ("Криобомба", "Криобомба", "Cryo bomb"),
    ("Глубокая заморозка площади с обращением лавы и воды", "Глубокая заморозка площади с обращением лавы и воды", "Deep freeze area turning lava and water to stone and ice"),
    ("Чёрная дыра", "Чёрная дыра", "Black hole"),
    ("Гравитационная сингулярность, затягивающая материю", "Гравитационная сингулярность, затягивающая материю", "Gravitational singularity pulling matter in"),
    ("Цепная молния", "Цепная молния", "Chain lightning"),
    ("Электрический разряд, бьющий цепочкой по целям", "Электрический разряд, бьющий цепочкой по целям", "Electric arc chaining between targets"),
    ("Плазменная стена", "Плазменная стена", "Plasma wall"),
    ("Раскалённая траншея плазмы поперёк сектора", "Раскалённая траншея плазмы поперёк сектора", "Scorching plasma trench across the sector"),
    ("Самонаводящиеся ракеты", "Самонаводящиеся ракеты", "Homing missiles"),
    ("Рой из 8 самонаводящихся тактических ракет", "Рой из 8 самонаводящихся тактических ракет", "Swarm of 8 homing tactical missiles"),
    ("Буровой луч", "Буровой луч", "Drill beam"),
    ("Узкий высокоэнергетический луч до бедрока", "Узкий высокоэнергетический луч до бедрока", "Narrow high-energy beam boring to bedrock"),
    ("Орбитальная решётка", "Орбитальная решётка", "Orbital grid"),
    ("Орбитальная сетка перекрёстных лазерных траншей", "Орбитальная сетка перекрёстных лазерных траншей", "Orbital lattice of intersecting laser trenches"),
    ("Стирание", "Стирание", "Erasure"),
    ("Полное стирание материи в заданном радиусе", "Полное стирание материи в заданном радиусе", "Complete erasure of matter in a given radius"),
    ("Сверхновая", "Сверхновая", "Supernova"),
    ("Гравитационный коллапс с образованием сверхновой", "Гравитационный коллапс с образованием сверхновой", "Gravitational collapse creating a supernova"),
];

pub fn tr<'a>(lang: Lang, key: &'a str) -> &'a str {
    for &(k, ru, en) in STRINGS {
        if k == key {
            return match lang {
                Lang::Ru => ru,
                Lang::En => en,
            };
        }
    }
    key
}
