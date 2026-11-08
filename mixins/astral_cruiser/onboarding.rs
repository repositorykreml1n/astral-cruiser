// Onboarding checklist progress and step state.
// Pure module depending only on std.

pub const STEP_COUNT: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    CallShip,
    Board,
    Seat,
    Fly,
    Wheel,
    Fire,
    Orbit,
    Strike,
    Tablet,
}

impl Step {
    pub const ALL: [Self; STEP_COUNT] = [
        Self::CallShip,
        Self::Board,
        Self::Seat,
        Self::Fly,
        Self::Wheel,
        Self::Fire,
        Self::Orbit,
        Self::Strike,
        Self::Tablet,
    ];

    pub fn index(self) -> usize {
        match self {
            Self::CallShip => 0,
            Self::Board => 1,
            Self::Seat => 2,
            Self::Fly => 3,
            Self::Wheel => 4,
            Self::Fire => 5,
            Self::Orbit => 6,
            Self::Strike => 7,
            Self::Tablet => 8,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::CallShip => "Вызвать корабль",
            Self::Board => "Подняться на борт",
            Self::Seat => "Сесть за штурвал",
            Self::Fly => "Полёт",
            Self::Wheel => "Колесо оружия",
            Self::Fire => "Выстрел",
            Self::Orbit => "Орбита",
            Self::Strike => "Удар с орбиты",
            Self::Tablet => "Планшет",
        }
    }

    #[allow(dead_code)]
    pub fn key_hint(self) -> &'static str {
        match self {
            Self::CallShip => "K",
            Self::Board => "F",
            Self::Seat => "V",
            Self::Fly => "WASD",
            Self::Wheel => "R",
            Self::Fire => "ЛКМ",
            Self::Orbit => "O",
            Self::Strike => "ЛКМ на орбите",
            Self::Tablet => "N",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    CalledShip,
    Boarded,
    Seated,
    Flew,
    OpenedWheel,
    Fired,
    Orbit,
    Struck,
    OpenedTablet,
}

impl Event {
    pub fn step(self) -> Step {
        match self {
            Self::CalledShip => Step::CallShip,
            Self::Boarded => Step::Board,
            Self::Seated => Step::Seat,
            Self::Flew => Step::Fly,
            Self::OpenedWheel => Step::Wheel,
            Self::Fired => Step::Fire,
            Self::Orbit => Step::Orbit,
            Self::Struck => Step::Strike,
            Self::OpenedTablet => Step::Tablet,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    pub done: [bool; STEP_COUNT],
    pub hidden: bool,
    pub finished_at: Option<f64>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            done: [false; STEP_COUNT],
            hidden: false,
            finished_at: None,
        }
    }
}

impl std::fmt::Display for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl Progress {
    /// Marks the step corresponding to event `e` as done. Out-of-order execution is allowed.
    pub fn observe(&mut self, e: Event) {
        self.done[e.step().index()] = true;
    }

    /// Returns the first step in order that has not been marked done yet, or None if all are done.
    pub fn current(&self) -> Option<Step> {
        Step::ALL.into_iter().find(|s| !self.done[s.index()])
    }

    /// Returns true if all 9 onboarding steps have been marked done.
    pub fn all_done(&self) -> bool {
        self.done.iter().all(|&b| b)
    }

    /// Resets all progress to default uncompleted state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Compact serialization format, e.g. "111000000h" or "111111111h:12345.0".
    pub fn to_string(&self) -> String {
        let mut s = String::with_capacity(32);
        for &d in &self.done {
            s.push(if d { '1' } else { '0' });
        }
        if self.hidden {
            s.push('h');
        }
        if let Some(ts) = self.finished_at {
            s.push(':');
            s.push_str(&ts.to_string());
        }
        s
    }

    /// Parses progress from a string. Tolerant of any garbage or invalid format, returning default.
    pub fn parse(s: &str) -> Self {
        let bytes = s.as_bytes();
        if bytes.len() < STEP_COUNT {
            return Self::default();
        }

        let mut done = [false; STEP_COUNT];
        for i in 0..STEP_COUNT {
            match bytes[i] {
                b'1' => done[i] = true,
                b'0' => done[i] = false,
                _ => return Self::default(),
            }
        }

        let mut rest = &s[STEP_COUNT..];
        let mut hidden = false;
        if rest.starts_with('h') {
            hidden = true;
            rest = &rest[1..];
        }

        let mut finished_at = None;
        if rest.starts_with(':') {
            let ts_str = &rest[1..];
            if let Ok(ts) = ts_str.parse::<f64>() {
                if ts.is_finite() {
                    finished_at = Some(ts);
                }
            }
        }

        Self {
            done,
            hidden,
            finished_at,
        }
    }
}
