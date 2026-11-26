//! Drawing units from `$INSUNITS`. A missing or zero value is `Unknown`; nothing is ever guessed.

/// Units named by `$INSUNITS` (AutoCAD codes 1 to 24). Anything else is `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Units {
    Unknown,
    Inch,
    Foot,
    Mile,
    Millimeter,
    Centimeter,
    Meter,
    Kilometer,
    Microinch,
    Mil,
    Yard,
    Angstrom,
    Nanometer,
    Micron,
    Decimeter,
    Decameter,
    Hectometer,
    Gigameter,
    AstronomicalUnit,
    LightYear,
    Parsec,
    UsSurveyFoot,
    UsSurveyInch,
    UsSurveyYard,
    UsSurveyMile,
}

impl Units {
    /// The units for an `$INSUNITS` value. 0 (unitless) and unknown codes give `Unknown`.
    pub fn from_insunits(code: i64) -> Units {
        match code {
            1 => Units::Inch,
            2 => Units::Foot,
            3 => Units::Mile,
            4 => Units::Millimeter,
            5 => Units::Centimeter,
            6 => Units::Meter,
            7 => Units::Kilometer,
            8 => Units::Microinch,
            9 => Units::Mil,
            10 => Units::Yard,
            11 => Units::Angstrom,
            12 => Units::Nanometer,
            13 => Units::Micron,
            14 => Units::Decimeter,
            15 => Units::Decameter,
            16 => Units::Hectometer,
            17 => Units::Gigameter,
            18 => Units::AstronomicalUnit,
            19 => Units::LightYear,
            20 => Units::Parsec,
            21 => Units::UsSurveyFoot,
            22 => Units::UsSurveyInch,
            23 => Units::UsSurveyYard,
            24 => Units::UsSurveyMile,
            _ => Units::Unknown,
        }
    }

    /// Stable lower-case name, `"unknown"` for `Unknown`.
    pub fn as_str(self) -> &'static str {
        match self {
            Units::Unknown => "unknown",
            Units::Inch => "inch",
            Units::Foot => "foot",
            Units::Mile => "mile",
            Units::Millimeter => "millimeter",
            Units::Centimeter => "centimeter",
            Units::Meter => "meter",
            Units::Kilometer => "kilometer",
            Units::Microinch => "microinch",
            Units::Mil => "mil",
            Units::Yard => "yard",
            Units::Angstrom => "angstrom",
            Units::Nanometer => "nanometer",
            Units::Micron => "micron",
            Units::Decimeter => "decimeter",
            Units::Decameter => "decameter",
            Units::Hectometer => "hectometer",
            Units::Gigameter => "gigameter",
            Units::AstronomicalUnit => "astronomical_unit",
            Units::LightYear => "light_year",
            Units::Parsec => "parsec",
            Units::UsSurveyFoot => "us_survey_foot",
            Units::UsSurveyInch => "us_survey_inch",
            Units::UsSurveyYard => "us_survey_yard",
            Units::UsSurveyMile => "us_survey_mile",
        }
    }
}
