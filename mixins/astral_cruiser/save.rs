// The ship's save file. It is written beside the world's data and replaced in
// one step, so a crash leaves the old ship or the new one, never half of either.
use super::astral_core::{Dimension, ShipPose};
use super::astral_lang::LangSetting;
use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};
use std::{fmt, fs, io::Write, path::Path};

/// Tablet preferences live beside ship.json, so dismissing a ship does not erase them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    pub name: String,
    #[serde(with = "waypoint_position")]
    pub pos: DVec3,
    pub dim: String,
}

mod waypoint_position {
    use super::*;
    pub fn serialize<S: serde::Serializer>(pos: &DVec3, serializer: S) -> Result<S::Ok, S::Error> {
        pos.to_array().serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<DVec3, D::Error> {
        <[f64; 3]>::deserialize(deserializer).map(DVec3::from_array)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TabletSave {
    pub waypoints: Vec<Waypoint>,
    pub shake: f32,
    pub volume: f32,
    pub onboarding: String,
    pub lang: LangSetting,
}

impl Default for TabletSave {
    fn default() -> Self {
        Self { waypoints: Vec::new(), shake: 1.0, volume: 1.0, onboarding: String::new(), lang: LangSetting::Auto }
    }
}

impl TabletSave {
    pub fn normalize(&mut self) {
        self.waypoints.retain(|p| p.pos.is_finite() && dimension_named(&p.dim).is_some());
        self.waypoints.truncate(8);
        for point in &mut self.waypoints {
            point.name = point.name.chars().take(24).collect();
        }
        for value in [&mut self.shake, &mut self.volume] {
            *value = if value.is_finite() { value.clamp(0.0, 2.0) } else { 1.0 };
        }
    }

    pub fn load(path: &Path) -> Result<Self, SaveError> {
        match fs::read_to_string(path) {
            Ok(text) => {
                let mut saved: Self = serde_json::from_str(&text)
                    .map_err(|e| SaveError::Malformed(e.to_string()))?;
                saved.normalize();
                Ok(saved)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(SaveError::Io(e.to_string())),
        }
    }

    pub fn save_atomic(&self, path: &Path) -> Result<(), SaveError> {
        let io = |e: std::io::Error| SaveError::Io(e.to_string());
        let mut saved = self.clone();
        saved.normalize();
        let text = serde_json::to_vec_pretty(&saved)
            .map_err(|e| SaveError::Malformed(e.to_string()))?;
        if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(io)?; }
        let temporary = path.with_extension("json.tmp");
        let mut file = fs::File::create(&temporary).map_err(io)?;
        file.write_all(&text).map_err(io)?;
        file.sync_all().map_err(io)?;
        drop(file);
        fs::rename(temporary, path).map_err(io)
    }
}

pub const CAPACITY: f64 = 10_000.0;
const VERSION: u32 = 1;
const USUAL_CRUISE: f64 = 64.0;
const USUAL_DASH: f64 = 800.0;
const CRUISE_RANGE: (f64, f64) = (16.0, 256.0);
const DASH_RANGE: (f64, f64) = (200.0, 4000.0);

/// The speeds the pilot has chosen, in blocks a second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Speeds {
    pub cruise: f64,
    pub dash: f64,
}

impl Default for Speeds {
    fn default() -> Self {
        Self { cruise: USUAL_CRUISE, dash: USUAL_DASH }
    }
}

impl Speeds {
    /// As near these as the ship can do.
    pub fn limited(cruise: f64, dash: f64) -> Self {
        let within = |value: f64, usual: f64, (low, high): (f64, f64)| {
            if value.is_nan() { usual } else { value.clamp(low, high) }
        };
        Self {
            cruise: within(cruise, USUAL_CRUISE, CRUISE_RANGE),
            dash: within(dash, USUAL_DASH, DASH_RANGE),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SaveState {
    pub dimension: Dimension,
    pub pose: ShipPose,
    /// The player was on board when this was written.
    pub aboard: bool,
    /// The guns' battery and how hot they run.
    pub energy: f64,
    pub heat: f64,
    pub speeds: Speeds,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SaveError {
    UnsupportedVersion(u32),
    Malformed(String),
    Io(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(f, "ship save format {v} is newer than this mod"),
            Self::Malformed(e) => write!(f, "ship save is damaged: {e}"),
            Self::Io(e) => write!(f, "ship save could not be read or written: {e}"),
        }
    }
}

impl std::error::Error for SaveError {}

#[derive(Deserialize)]
struct Header {
    version: u32,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    dimension: String,
    position: [f64; 3],
    rotation: [f64; 4],
    #[serde(default)]
    aboard: bool,
    #[serde(default = "full_battery")]
    energy: f64,
    #[serde(default)]
    heat: f64,
    #[serde(default = "usual_cruise")]
    cruise: f64,
    #[serde(default = "usual_dash")]
    dash: f64,
    #[serde(default, alias = "energy_capacity")]
    capacity: Option<f64>,
}

fn full_battery() -> f64 {
    CAPACITY
}

fn usual_cruise() -> f64 {
    USUAL_CRUISE
}

fn usual_dash() -> f64 {
    USUAL_DASH
}

fn dimension_name(dimension: Dimension) -> &'static str {
    match dimension {
        Dimension::Overworld => "overworld",
        Dimension::Nether => "nether",
        Dimension::End => "end",
    }
}

fn dimension_named(name: &str) -> Option<Dimension> {
    match name {
        "overworld" => Some(Dimension::Overworld),
        "nether" => Some(Dimension::Nether),
        "end" => Some(Dimension::End),
        _ => None,
    }
}

impl SaveState {
    pub fn to_json(&self) -> String {
        let stored = Stored {
            version: VERSION,
            dimension: dimension_name(self.dimension).to_string(),
            position: self.pose.position.to_array(),
            rotation: self.pose.rotation.to_array(),
            aboard: self.aboard,
            energy: self.energy,
            heat: self.heat,
            cruise: self.speeds.cruise,
            dash: self.speeds.dash,
            capacity: Some(CAPACITY),
        };
        serde_json::to_string_pretty(&stored).expect("numbers and text always serialize")
    }

    pub fn from_json(text: &str) -> Result<Self, SaveError> {
        let malformed = |e: serde_json::Error| SaveError::Malformed(e.to_string());
        let header: Header = serde_json::from_str(text).map_err(malformed)?;
        if header.version != VERSION {
            return Err(SaveError::UnsupportedVersion(header.version));
        }
        let stored: Stored = serde_json::from_str(text).map_err(malformed)?;
        let dimension = dimension_named(&stored.dimension)
            .ok_or_else(|| SaveError::Malformed(format!("dimension {}", stored.dimension)))?;
        let energy = if stored.energy.is_nan() {
            CAPACITY
        } else {
            match stored.capacity {
                Some(_) => stored.energy.clamp(0.0, CAPACITY),
                None => (stored.energy * 100.0).clamp(0.0, CAPACITY),
            }
        };
        Ok(Self {
            dimension,
            pose: ShipPose::new(
                DVec3::from_array(stored.position),
                DQuat::from_array(stored.rotation),
            ),
            aboard: stored.aboard,
            energy,
            heat: stored.heat,
            speeds: Speeds::limited(stored.cruise, stored.dash),
        })
    }

    pub fn load(path: &Path) -> Result<Option<Self>, SaveError> {
        match fs::read_to_string(path) {
            Ok(text) => Self::from_json(&text).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SaveError::Io(e.to_string())),
        }
    }

    pub fn save_atomic(&self, path: &Path) -> Result<(), SaveError> {
        let io = |e: std::io::Error| SaveError::Io(e.to_string());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        let temporary = path.with_extension("json.tmp");
        let mut file = fs::File::create(&temporary).map_err(io)?;
        file.write_all(self.to_json().as_bytes()).map_err(io)?;
        file.sync_all().map_err(io)?;
        drop(file);
        fs::rename(&temporary, path).map_err(io)
    }
}
