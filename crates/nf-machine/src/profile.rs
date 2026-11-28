//! M1: the machine profile. Named factory rules, every value sourced; nothing is defaulted.
//! kerf_mm and lead_in_mm are placeholders: null until a person records a source and a test.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DxfVersion {
    /// AutoCAD R12, AC1009.
    #[serde(rename = "R12")]
    R12,
    /// AutoCAD R2000, AC1015.
    #[serde(rename = "R2000")]
    R2000,
}

impl DxfVersion {
    pub fn acad_ver(self) -> &'static str {
        match self {
            DxfVersion::R12 => "AC1009",
            DxfVersion::R2000 => "AC1015",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextEncoding {
    #[serde(rename = "utf-8")]
    Utf8,
    /// Shift_JIS (CP932 as in encoding_rs); text that does not map is refused, never replaced.
    #[serde(rename = "shift_jis")]
    ShiftJis,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineProfile {
    /// Short id, letters digits '-' '_' only, e.g. "PLASMA-A".
    pub id: String,
    pub version: u32,
    pub name: String,
    pub dxf_version: DxfVersion,
    pub text_encoding: TextEncoding,
    /// Only millimetres are supported; a source file in other units is refused.
    pub units: String,
    /// NF layer name -> layer name the machine's CAM expects. Every layer in the file must be mapped.
    pub layer_map: BTreeMap<String, String>,
    /// File name pattern; must contain {part}. Placeholders: {job} {part} {profile}.
    pub file_name: String,
    /// UNVERIFIED placeholder. Set only with a source citation.
    pub kerf_mm: Option<f64>,
    /// UNVERIFIED placeholder. Set only with a source citation.
    pub lead_in_mm: Option<f64>,
    /// One citation per sourced value (manual page, test record).
    pub sources: Vec<String>,
    /// Person and date who tested this profile on the real machine. None = untested.
    pub tested_by: Option<String>,
}

/// Longest output file name, in bytes.
pub const MAX_FILE_NAME: usize = 120;

/// True when the string has no control character (newlines included). Profile strings that are written
/// into a DXF value line must pass: a newline would start a new group code / value pair.
pub fn is_clean(s: &str) -> bool {
    !s.chars()
        .any(|c| c.is_control() || c == '\u{2028}' || c == '\u{2029}')
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProfileError {
    Parse(String),
    BadId,
    BadVersion,
    NoLayers,
    EmptyLayer(String),
    /// A layer name (key or machine value) holds a control character or line break.
    ControlInLayer(String),
    BadFileName,
    BadUnits(String),
    NoSources,
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileError::Parse(e) => write!(f, "profile parse error: {e}"),
            ProfileError::BadId => write!(f, "profile id must be letters, digits, '-' or '_'"),
            ProfileError::BadVersion => write!(f, "profile version must be at least 1"),
            ProfileError::NoLayers => write!(f, "layer map is empty"),
            ProfileError::EmptyLayer(l) => {
                write!(f, "layer map entry '{l}' has an empty machine name")
            }
            ProfileError::ControlInLayer(l) => {
                write!(f, "layer map entry '{l}' has a control character or line break")
            }
            ProfileError::BadFileName => write!(
                f,
                "file_name must contain {{part}}, no path separators, ':' or '..', and be at most {MAX_FILE_NAME} bytes"
            ),
            ProfileError::BadUnits(u) => write!(f, "unsupported units '{u}' (only mm)"),
            ProfileError::NoSources => write!(f, "a profile needs at least one source citation"),
        }
    }
}

impl std::error::Error for ProfileError {}

impl MachineProfile {
    pub fn from_json(s: &str) -> Result<Self, ProfileError> {
        let p: MachineProfile =
            serde_json::from_str(s).map_err(|e| ProfileError::Parse(e.to_string()))?;
        p.validate()?;
        Ok(p)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("profile serialises")
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ProfileError::BadId);
        }
        if self.version == 0 {
            return Err(ProfileError::BadVersion);
        }
        if self.units != "mm" {
            return Err(ProfileError::BadUnits(self.units.clone()));
        }
        if self.layer_map.is_empty() {
            return Err(ProfileError::NoLayers);
        }
        for (k, v) in &self.layer_map {
            if k.is_empty() || v.trim().is_empty() {
                return Err(ProfileError::EmptyLayer(k.clone()));
            }
            if !is_clean(k) || !is_clean(v) {
                return Err(ProfileError::ControlInLayer(k.clone()));
            }
        }
        if !self.file_name.contains("{part}")
            || self.file_name.contains(['/', '\\', ':'])
            || self.file_name.contains("..")
            || self.file_name.len() > MAX_FILE_NAME
            || !is_clean(&self.file_name)
        {
            return Err(ProfileError::BadFileName);
        }
        if self.sources.is_empty() || self.sources.iter().any(|s| s.trim().is_empty()) {
            return Err(ProfileError::NoSources);
        }
        Ok(())
    }

    /// Output file name for one part of one job.
    pub fn file_name_for(&self, job: &str, part: &str) -> String {
        self.file_name
            .replace("{job}", job)
            .replace("{part}", part)
            .replace("{profile}", &self.id)
    }
}
