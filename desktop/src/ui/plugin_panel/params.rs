//! A plugin parameter as the window shows it, read from `strip.parameters`: its range, scale,
//! choices and the text the plugin displays. Knob positions follow the parameter's log scale;
//! typed values commit once, in range.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub id: u32,
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub value: f64,
    pub default: f64,
    pub unit: String,
    pub steps: u32,
    pub labels: Vec<String>,
    pub logarithmic: bool,
    /// The value as the plugin writes it ("-6.0 dB", "Hall").
    pub display: String,
    pub automatable: bool,
    /// The automation lane that drives this parameter, if any.
    pub lane: Option<String>,
}

impl Parameter {
    /// One row of `strip.parameters`.
    pub fn from_json(v: &Value) -> Option<Self> {
        let num = |key: &str| v[key].as_f64();
        Some(Self {
            id: u32::try_from(v["id"].as_u64()?).ok()?,
            name: v["name"].as_str()?.to_string(),
            min: num("min")?,
            max: num("max")?,
            value: num("value")?,
            default: num("default").unwrap_or(0.0),
            unit: v["unit"].as_str().unwrap_or_default().to_string(),
            steps: v["steps"].as_u64().unwrap_or(0) as u32,
            labels: v["labels"]
                .as_array()
                .map(|l| {
                    l.iter()
                        .filter_map(|s| s.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            logarithmic: v["logarithmic"].as_bool().unwrap_or(false),
            display: v["display"].as_str().unwrap_or_default().to_string(),
            automatable: v["automatable"].as_bool().unwrap_or(true),
            lane: v["automationLane"].as_str().map(str::to_string),
        })
    }

    /// A choice among labels (a switch, tabs or a select) rather than a dial.
    pub fn is_choice(&self) -> bool {
        !self.labels.is_empty()
    }

    /// Value to knob position 0..1, logarithmic where the plugin says so.
    pub fn position(&self, value: f64) -> f64 {
        if self.max == self.min {
            return 0.0;
        }
        let v = value.clamp(self.min.min(self.max), self.max.max(self.min));
        if self.logarithmic && self.min > 0.0 {
            (v / self.min).ln() / (self.max / self.min).ln()
        } else {
            (v - self.min) / (self.max - self.min)
        }
    }

    /// Knob position 0..1 to a value; stepped parameters land on their steps.
    pub fn from_position(&self, position: f64) -> f64 {
        let mut t = position.clamp(0.0, 1.0);
        if self.steps > 1 {
            let n = (self.steps - 1) as f64;
            t = (t * n).round() / n;
        }
        if self.logarithmic && self.min > 0.0 {
            self.min * (self.max / self.min).powf(t)
        } else {
            self.min + t * (self.max - self.min)
        }
    }

    /// A range that straddles zero fills its arc from the middle, like a pan or a gain.
    pub fn bipolar(&self) -> bool {
        self.min < 0.0 && self.max > 0.0
    }

    /// The value as a stock face writes it: labels, kHz above 1000 Hz, fewer decimals as
    /// numbers grow, and the unit.
    pub fn format(&self, value: f64) -> String {
        if !self.labels.is_empty() {
            return self
                .labels
                .get(value.round().max(0.0) as usize)
                .cloned()
                .unwrap_or_default();
        }
        if self.unit == "Hz" && value >= 1000.0 {
            let digits = if value >= 10_000.0 { 1 } else { 2 };
            return format!("{:.*} kHz", digits, value / 1000.0);
        }
        let abs = value.abs();
        let digits = if abs >= 100.0 {
            0
        } else if abs >= 10.0 {
            1
        } else {
            2
        };
        let text = format!("{value:.digits$}");
        let space = if matches!(self.unit.as_str(), ":1" | "°" | "x") {
            ""
        } else {
            " "
        };
        if self.unit.is_empty() {
            text
        } else {
            format!("{text}{space}{}", self.unit)
        }
    }

    /// The value as the plugin shows it; the face's own format when the plugin says nothing.
    pub fn shown(&self) -> String {
        if self.display.trim().is_empty() {
            self.format(self.value)
        } else {
            self.display.clone()
        }
    }
}

/// What a typed value commits.
#[derive(Clone, Debug, PartialEq)]
pub enum Typed {
    /// A number, clamped to the range.
    Value(f64),
    /// Text for the plugin to read ("-6 dB", "2.5k", "Hall").
    Text(String),
}

/// A value typed into a parameter's field, committed on Enter or when the field loses focus.
/// An emptied field commits nothing, a number is clamped to the range (typing "5" on the way
/// to "500" never reaches the plugin), the same value is no edit, and anything else is text
/// the plugin parses.
pub fn typed(p: &Parameter, text: &str) -> Option<Typed> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    match text.parse::<f64>() {
        Ok(v) if v.is_finite() => {
            let clamped = v.clamp(p.min.min(p.max), p.max.max(p.min));
            (clamped != p.value).then_some(Typed::Value(clamped))
        }
        Ok(_) => None,
        Err(_) => (text != p.display.trim()).then(|| Typed::Text(text.to_string())),
    }
}

/// Parameters whose name holds every word of the filter, in their order.
pub fn matching<'a>(params: &'a [Parameter], filter: &str) -> Vec<&'a Parameter> {
    let words: Vec<String> = filter
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();
    params
        .iter()
        .filter(|p| {
            let name = p.name.to_lowercase();
            words.iter().all(|w| name.contains(w.as_str()))
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn cutoff() -> Parameter {
    Parameter {
        id: 3,
        name: "Cutoff".into(),
        min: 20.0,
        max: 20_000.0,
        value: 1000.0,
        default: 1000.0,
        unit: "Hz".into(),
        steps: 0,
        labels: vec![],
        logarithmic: true,
        display: "1.00 kHz".into(),
        automatable: true,
        lane: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn log_knobs_round_trip_and_values_read_like_the_face() {
        let p = cutoff();
        assert!((p.position(632.5) - 0.5).abs() < 0.005);
        assert!((p.from_position(p.position(4321.0)) - 4321.0).abs() < 1e-6);
        assert_eq!(p.format(1000.0), "1.00 kHz");
        assert_eq!(p.format(12_500.0), "12.5 kHz");
        let ratio = Parameter {
            unit: ":1".into(),
            logarithmic: false,
            ..cutoff()
        };
        assert_eq!(ratio.format(4.0), "4.00:1");
        let gain = Parameter {
            unit: "dB".into(),
            min: -24.0,
            max: 24.0,
            logarithmic: false,
            ..cutoff()
        };
        assert_eq!(gain.format(-6.0), "-6.00 dB");
        assert_eq!(gain.format(-12.34), "-12.3 dB");
        assert!(gain.bipolar() && !p.bipolar());
        let choice = Parameter {
            labels: vec!["Low".into(), "High".into(), "Band".into()],
            min: 0.0,
            max: 2.0,
            steps: 3,
            logarithmic: false,
            ..cutoff()
        };
        assert_eq!(choice.format(1.0), "High");
        assert_eq!(choice.from_position(0.4), 1.0, "steps snap");
    }

    // Every keystroke used to be sent: clearing the field set the cutoff to 0 at once, and
    // "5" on the way to "500" was out of range.
    #[test]
    fn a_typed_value_commits_once_in_range_and_never_from_an_empty_field() {
        let p = cutoff();
        assert_eq!(typed(&p, ""), None);
        assert_eq!(typed(&p, "   "), None);
        assert_eq!(typed(&p, "500"), Some(Typed::Value(500.0)));
        assert_eq!(typed(&p, "5"), Some(Typed::Value(20.0)));
        assert_eq!(typed(&p, "90000"), Some(Typed::Value(20_000.0)));
        assert_eq!(typed(&p, "1000"), None, "the same value is no edit");
        assert_eq!(typed(&p, "2.5k"), Some(Typed::Text("2.5k".into())));
        assert_eq!(typed(&p, "1.00 kHz"), None);
    }

    #[test]
    fn rows_of_strip_parameters_parse_and_filter_by_words() {
        let row = json!({"id": 7, "name": "Filter Cutoff", "value": 440.0, "min": 20.0,
            "max": 20000.0, "default": 1000.0, "unit": "Hz", "steps": 0, "labels": [],
            "logarithmic": true, "display": "440 Hz", "automatable": true,
            "automationLane": "automation-1"});
        let p = Parameter::from_json(&row).unwrap();
        assert_eq!(p.id, 7);
        assert_eq!(p.lane.as_deref(), Some("automation-1"));
        assert_eq!(p.shown(), "440 Hz");
        let list = vec![p, cutoff()];
        assert_eq!(matching(&list, "cut").len(), 2);
        assert_eq!(matching(&list, "filter cut").len(), 1);
        assert_eq!(matching(&list, "").len(), 2);
        assert!(Parameter::from_json(&json!({"name": "x"})).is_none());
    }
}
