//! Story-owned deterministic controls rendered by the gallery.
#[derive(Debug, Clone, PartialEq, Eq)]
/// Host-editable value for one deterministic demo control.
pub enum KnobValue {
    /// On/off value.
    Bool(bool),
    /// Selected index into [`Knob::choices`].
    Choice(usize),
    /// Editable text value.
    Text(String),
    /// Signed numeric value.
    Number(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One deterministic control exposed beside a live demo.
pub struct Knob {
    /// Stable control identifier.
    pub id: &'static str,
    /// Human-readable label.
    pub label: &'static str,
    /// Current value.
    pub value: KnobValue,
    /// Valid labels for a choice value.
    pub choices: &'static [&'static str],
}

impl Knob {
    /// Format the current value for native Lookbook chrome.
    pub fn display_value(&self) -> String {
        match &self.value {
            KnobValue::Bool(value) => if *value { "on" } else { "off" }.to_owned(),
            KnobValue::Choice(index) => self.choices.get(*index).copied().unwrap_or("").to_owned(),
            KnobValue::Text(value) => value.clone(),
            KnobValue::Number(value) => value.to_string(),
        }
    }
}
