//! Story-owned deterministic controls rendered by the gallery.

#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    dead_code,
    reason = "Bool and Number are designed for the full story rollout"
)]
pub(crate) enum KnobValue {
    Bool(bool),
    Choice(usize),
    Text(String),
    Number(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Knob {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) value: KnobValue,
    pub(crate) choices: &'static [&'static str],
}

impl Knob {
    pub(crate) fn display_value(&self) -> String {
        match &self.value {
            KnobValue::Bool(value) => if *value { "on" } else { "off" }.to_owned(),
            KnobValue::Choice(index) => self.choices.get(*index).copied().unwrap_or("").to_owned(),
            KnobValue::Text(value) => value.clone(),
            KnobValue::Number(value) => value.to_string(),
        }
    }
}
