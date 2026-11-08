// Orbital arsenal: 18 orbital strikes, groups, specs, and shockwave parameters.
// Pure module depending only on std and glam.
use super::astral_lang::Lang;

#[repr(usize)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Weapon {
    #[default]
    Ion = 0,
    Barrage,
    Rod,
    Lance,
    Antimatter,
    SolarPillar,
    Tectonic,
    Meteors,
    Cluster,
    Cryo,
    BlackHole,
    Chain,
    PlasmaWall,
    Missiles,
    Drill,
    Grid,
    Erasure,
    Supernova,
}

pub const COUNT: usize = 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Classic,
    Super,
    Zone,
    Special,
    Precise,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveSound {
    Small,
    Medium,
    Large,
    Super,
    Frost,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveSpec {
    pub delay: f64,
    pub radius: f64,
    pub strength: f64,
    pub damaging: bool,
    pub sound: WaveSound,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponSpec {
    pub name: &'static str,
    pub name_en: &'static str,
    pub blurb: &'static str,
    pub blurb_en: &'static str,
    pub group: Group,
    pub color: u32, /* 0xRRGGBB */
    pub energy: f64,
    pub energy_per_second: f64,
    pub cooldown: f64,
    pub charge: f64,
    pub duration: f64,
    pub reach: f64,
    pub waves: &'static [WaveSpec],
}

pub fn display_name(w: Weapon, lang: Lang) -> &'static str {
    match lang {
        Lang::Ru => spec(w).name,
        Lang::En => spec(w).name_en,
    }
}

pub fn display_blurb(w: Weapon, lang: Lang) -> &'static str {
    match lang {
        Lang::Ru => spec(w).blurb,
        Lang::En => spec(w).blurb_en,
    }
}

pub const SUPER_COOLDOWN: f64 = 10.0;
#[allow(dead_code)]
pub const MIN_COOLDOWN: f64 = 0.05;

impl Weapon {
    pub const ALL: [Weapon; COUNT] = [
        Weapon::Ion,
        Weapon::Barrage,
        Weapon::Rod,
        Weapon::Lance,
        Weapon::Antimatter,
        Weapon::SolarPillar,
        Weapon::Tectonic,
        Weapon::Meteors,
        Weapon::Cluster,
        Weapon::Cryo,
        Weapon::BlackHole,
        Weapon::Chain,
        Weapon::PlasmaWall,
        Weapon::Missiles,
        Weapon::Drill,
        Weapon::Grid,
        Weapon::Erasure,
        Weapon::Supernova,
    ];

    #[allow(dead_code)]
    pub fn stepped(self, by: i32) -> Self {
        let at = self as usize as i32;
        let next = (at + by).rem_euclid(COUNT as i32) as usize;
        Self::ALL[next]
    }

    #[allow(dead_code)]
    pub fn from_index(i: usize) -> Option<Self> {
        if i < COUNT {
            Some(Self::ALL[i])
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn is_super(self) -> bool {
        matches!(
            self,
            Weapon::Antimatter
                | Weapon::SolarPillar
                | Weapon::Tectonic
                | Weapon::Erasure
                | Weapon::Supernova
        )
    }
}

static ION_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 3.0,
    radius: 100.0,
    strength: 0.3,
    damaging: false,
    sound: WaveSound::Small,
}];

static BARRAGE_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 5.5,
    radius: 80.0,
    strength: 0.3,
    damaging: false,
    sound: WaveSound::Small,
}];

static ROD_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 2.5,
    radius: 150.0,
    strength: 0.5,
    damaging: false,
    sound: WaveSound::Medium,
}];

static ANTIMATTER_WAVES: [WaveSpec; 3] = [
    WaveSpec {
        delay: 1.5,
        radius: 500.0,
        strength: 1.0,
        damaging: true,
        sound: WaveSound::Super,
    },
    WaveSpec {
        delay: 2.7,
        radius: 500.0,
        strength: 0.6,
        damaging: true,
        sound: WaveSound::Super,
    },
    WaveSpec {
        delay: 4.1,
        radius: 500.0,
        strength: 0.35,
        damaging: true,
        sound: WaveSound::Super,
    },
];

static SOLAR_PILLAR_WAVES: [WaveSpec; 2] = [
    WaveSpec {
        delay: 1.0,
        radius: 500.0,
        strength: 0.9,
        damaging: true,
        sound: WaveSound::Super,
    },
    WaveSpec {
        delay: 2.8,
        radius: 500.0,
        strength: 0.5,
        damaging: true,
        sound: WaveSound::Super,
    },
];

static TECTONIC_WAVES: [WaveSpec; 5] = [
    WaveSpec {
        delay: 0.5,
        radius: 500.0,
        strength: 0.8,
        damaging: true,
        sound: WaveSound::Super,
    },
    WaveSpec {
        delay: 1.3,
        radius: 500.0,
        strength: 0.65,
        damaging: true,
        sound: WaveSound::Super,
    },
    WaveSpec {
        delay: 2.1,
        radius: 500.0,
        strength: 0.5,
        damaging: true,
        sound: WaveSound::Super,
    },
    WaveSpec {
        delay: 2.9,
        radius: 500.0,
        strength: 0.35,
        damaging: true,
        sound: WaveSound::Super,
    },
    WaveSpec {
        delay: 3.7,
        radius: 500.0,
        strength: 0.2,
        damaging: true,
        sound: WaveSound::Super,
    },
];

static METEORS_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 6.0,
    radius: 150.0,
    strength: 0.5,
    damaging: false,
    sound: WaveSound::Medium,
}];

static CLUSTER_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 1.5,
    radius: 150.0,
    strength: 0.5,
    damaging: false,
    sound: WaveSound::Medium,
}];

static CRYO_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 2.0,
    radius: 120.0,
    strength: 0.5,
    damaging: false,
    sound: WaveSound::Frost,
}];

static BLACK_HOLE_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 4.0,
    radius: 200.0,
    strength: 0.7,
    damaging: false,
    sound: WaveSound::Large,
}];

static PLASMA_WALL_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 0.0,
    radius: 80.0,
    strength: 0.3,
    damaging: false,
    sound: WaveSound::Small,
}];

static MISSILES_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 1.5,
    radius: 60.0,
    strength: 0.3,
    damaging: false,
    sound: WaveSound::Small,
}];

static GRID_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 0.5,
    radius: 80.0,
    strength: 0.3,
    damaging: false,
    sound: WaveSound::Small,
}];

static ERASURE_WAVES: [WaveSpec; 1] = [WaveSpec {
    delay: 10.0,
    radius: 200.0,
    strength: 0.6,
    damaging: false,
    sound: WaveSound::Medium,
}];

static SUPERNOVA_WAVES: [WaveSpec; 3] = [
    WaveSpec {
        delay: 10.0,
        radius: 500.0,
        strength: 1.0,
        damaging: true,
        sound: WaveSound::Super,
    },
    WaveSpec {
        delay: 11.0,
        radius: 500.0,
        strength: 0.7,
        damaging: true,
        sound: WaveSound::Super,
    },
    WaveSpec {
        delay: 12.4,
        radius: 500.0,
        strength: 0.4,
        damaging: true,
        sound: WaveSound::Super,
    },
];

pub fn spec(w: Weapon) -> WeaponSpec {
    match w {
        Weapon::Ion => WeaponSpec {
            name: "Ионный луч",
            name_en: "Ion beam",
            blurb: "Тяжёлый ионный луч длительного действия",
            blurb_en: "Heavy sustained ion beam",
            group: Group::Classic,
            color: 0x59D9FF,
            energy: 60.0,
            energy_per_second: 0.0,
            cooldown: 0.25,
            charge: 3.0,
            duration: 4.0,
            reach: 16.0,
            waves: &ION_WAVES,
        },
        Weapon::Barrage => WeaponSpec {
            name: "Залп",
            name_en: "Barrage",
            blurb: "Артиллерийский залп 24 тяжёлыми снарядами",
            blurb_en: "Artillery barrage of 24 heavy shells",
            group: Group::Classic,
            color: 0xFFB040,
            energy: 50.0,
            energy_per_second: 0.0,
            cooldown: 0.2,
            charge: 0.0,
            duration: 4.0,
            reach: 32.0,
            waves: &BARRAGE_WAVES,
        },
        Weapon::Rod => WeaponSpec {
            name: "Кинетический стержень",
            name_en: "Kinetic rod",
            blurb: "Вольфрамовый стержень колоссальной кинетической силы",
            blurb_en: "Tungsten kinetic strike of colossal force",
            group: Group::Classic,
            color: 0xFF7030,
            energy: 80.0,
            energy_per_second: 0.0,
            cooldown: 0.45,
            charge: 0.0,
            duration: 2.5,
            reach: 32.0,
            waves: &ROD_WAVES,
        },
        Weapon::Lance => WeaponSpec {
            name: "Резак",
            name_en: "Lance",
            blurb: "Высокоточный непрерывный режущий луч",
            blurb_en: "Precision continuous cutting beam",
            group: Group::Classic,
            color: 0xFF4040,
            energy: 0.0,
            energy_per_second: 12.0,
            cooldown: 0.15,
            charge: 0.0,
            duration: 8.0,
            reach: 3.0,
            waves: &[],
        },
        Weapon::Antimatter => WeaponSpec {
            name: "Антиматерия",
            name_en: "Antimatter",
            blurb: "Взрыв колоссальной мощности с чашей и огненным грибом",
            blurb_en: "Colossal explosion with crater bowl and mushroom cloud",
            group: Group::Super,
            color: 0xF4F0FF,
            energy: 3000.0,
            energy_per_second: 0.0,
            cooldown: SUPER_COOLDOWN,
            charge: 1.5,
            duration: 16.0,
            reach: 500.0,
            waves: &ANTIMATTER_WAVES,
        },
        Weapon::SolarPillar => WeaponSpec {
            name: "Солнечный столп",
            name_en: "Solar pillar",
            blurb: "Огненный столп до самого бедрока с оплавленным краем",
            blurb_en: "Fiery pillar down to bedrock with fused rim",
            group: Group::Super,
            color: 0xFFD040,
            energy: 2500.0,
            energy_per_second: 0.0,
            cooldown: SUPER_COOLDOWN,
            charge: 1.0,
            duration: 10.0,
            reach: 500.0,
            waves: &SOLAR_PILLAR_WAVES,
        },
        Weapon::Tectonic => WeaponSpec {
            name: "Тектонический молот",
            name_en: "Tectonic hammer",
            blurb: "Мощный подземный удар, раскалывающий породу разломами",
            blurb_en: "Subterranean impact shattering rock with fissures",
            group: Group::Super,
            color: 0xC08040,
            energy: 2500.0,
            energy_per_second: 0.0,
            cooldown: SUPER_COOLDOWN,
            charge: 0.5,
            duration: 8.0,
            reach: 500.0,
            waves: &TECTONIC_WAVES,
        },
        Weapon::Meteors => WeaponSpec {
            name: "Метеоритный дождь",
            name_en: "Meteor shower",
            blurb: "Бомбардировка 40 метеорами по площади",
            blurb_en: "Area bombardment of 40 meteors",
            group: Group::Zone,
            color: 0xFF5020,
            energy: 400.0,
            energy_per_second: 0.0,
            cooldown: 0.4,
            charge: 0.0,
            duration: 6.0,
            reach: 60.0,
            waves: &METEORS_WAVES,
        },
        Weapon::Cluster => WeaponSpec {
            name: "Кассетный рой",
            name_en: "Cluster swarm",
            blurb: "Сетка из 60 кассетных суббоеприпасов",
            blurb_en: "Grid of 60 cluster submunitions",
            group: Group::Zone,
            color: 0xFFE060,
            energy: 300.0,
            energy_per_second: 0.0,
            cooldown: 0.3,
            charge: 0.0,
            duration: 1.5,
            reach: 40.0,
            waves: &CLUSTER_WAVES,
        },
        Weapon::Cryo => WeaponSpec {
            name: "Криобомба",
            name_en: "Cryo bomb",
            blurb: "Глубокая заморозка площади с обращением лавы и воды",
            blurb_en: "Deep freeze area turning lava and water to stone and ice",
            group: Group::Zone,
            color: 0x90E8FF,
            energy: 300.0,
            energy_per_second: 0.0,
            cooldown: 0.3,
            charge: 0.0,
            duration: 2.0,
            reach: 50.0,
            waves: &CRYO_WAVES,
        },
        Weapon::BlackHole => WeaponSpec {
            name: "Чёрная дыра",
            name_en: "Black hole",
            blurb: "Гравитационная сингулярность, затягивающая материю",
            blurb_en: "Gravitational singularity pulling matter in",
            group: Group::Special,
            color: 0xA040FF,
            energy: 800.0,
            energy_per_second: 0.0,
            cooldown: 0.6,
            charge: 0.0,
            duration: 8.0,
            reach: 60.0,
            waves: &BLACK_HOLE_WAVES,
        },
        Weapon::Chain => WeaponSpec {
            name: "Цепная молния",
            name_en: "Chain lightning",
            blurb: "Электрический разряд, бьющий цепочкой по целям",
            blurb_en: "Electric arc chaining between targets",
            group: Group::Special,
            color: 0x80A0FF,
            energy: 200.0,
            energy_per_second: 0.0,
            cooldown: 0.2,
            charge: 0.0,
            duration: 1.5,
            reach: 80.0,
            waves: &[],
        },
        Weapon::PlasmaWall => WeaponSpec {
            name: "Плазменная стена",
            name_en: "Plasma wall",
            blurb: "Раскалённая траншея плазмы поперёк сектора",
            blurb_en: "Scorching plasma trench across the sector",
            group: Group::Special,
            color: 0xFF40A0,
            energy: 250.0,
            energy_per_second: 0.0,
            cooldown: 0.3,
            charge: 0.0,
            duration: 4.0,
            reach: 60.0,
            waves: &PLASMA_WALL_WAVES,
        },
        Weapon::Missiles => WeaponSpec {
            name: "Самонаводящиеся ракеты",
            name_en: "Homing missiles",
            blurb: "Рой из 8 самонаводящихся тактических ракет",
            blurb_en: "Swarm of 8 homing tactical missiles",
            group: Group::Precise,
            color: 0xFF8080,
            energy: 150.0,
            energy_per_second: 0.0,
            cooldown: 0.2,
            charge: 0.0,
            duration: 3.0,
            reach: 64.0,
            waves: &MISSILES_WAVES,
        },
        Weapon::Drill => WeaponSpec {
            name: "Буровой луч",
            name_en: "Drill beam",
            blurb: "Узкий высокоэнергетический луч до бедрока",
            blurb_en: "Narrow high-energy beam boring to bedrock",
            group: Group::Precise,
            color: 0x40FFB0,
            energy: 200.0,
            energy_per_second: 0.0,
            cooldown: 0.3,
            charge: 0.0,
            duration: 3.0,
            reach: 4.0,
            waves: &[],
        },
        Weapon::Grid => WeaponSpec {
            name: "Орбитальная решётка",
            name_en: "Orbital grid",
            blurb: "Орбитальная сетка перекрёстных лазерных траншей",
            blurb_en: "Orbital lattice of intersecting laser trenches",
            group: Group::Precise,
            color: 0x40FF60,
            energy: 250.0,
            energy_per_second: 0.0,
            cooldown: 0.3,
            charge: 0.0,
            duration: 3.0,
            reach: 42.0,
            waves: &GRID_WAVES,
        },
        Weapon::Erasure => WeaponSpec {
            name: "Стирание",
            name_en: "Erasure",
            blurb: "Полное стирание материи в заданном радиусе",
            blurb_en: "Complete erasure of matter in a given radius",
            group: Group::Super,
            color: 0x7CF7FF,
            energy: 4000.0,
            energy_per_second: 0.0,
            cooldown: SUPER_COOLDOWN,
            charge: 10.0,
            duration: 22.0,
            reach: 120.0,
            waves: &ERASURE_WAVES,
        },
        Weapon::Supernova => WeaponSpec {
            name: "Сверхновая",
            name_en: "Supernova",
            blurb: "Гравитационный коллапс с образованием сверхновой",
            blurb_en: "Gravitational collapse creating a supernova",
            group: Group::Super,
            color: 0xFFB347,
            energy: 5000.0,
            energy_per_second: 0.0,
            cooldown: SUPER_COOLDOWN,
            charge: 10.0,
            duration: 24.0,
            reach: 500.0,
            waves: &SUPERNOVA_WAVES,
        },
    }
}
