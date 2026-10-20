//! Keep readable warnings alongside machine-readable categories, parts, and targets.
use serde::Serialize;
use std::ops::Deref;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    Unsupported,
    Omitted,
    Approximation,
    FontSubstitution,
    MissingGlyph,
    TextOverflow,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FontResolution {
    pub requested_family: String,
    pub actual_family: String,
    pub post_script_name: String,
    pub face_index: u32,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: Code,
    pub part: String,
    pub object_id: Option<String>,
    pub object_name: Option<String>,
    pub location: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<FontResolution>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_character: Option<char>,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Target {
    pub part: String,
    pub object_id: Option<String>,
    pub object_name: Option<String>,
    pub location: String,
}
#[derive(Debug, Default)]
pub(super) struct Warnings {
    messages: Vec<String>,
    pub records: Vec<Diagnostic>,
    pub target: Target,
}
impl Deref for Warnings {
    type Target = Vec<String>;
    fn deref(&self) -> &Self::Target {
        &self.messages
    }
}
impl Warnings {
    pub fn push(&mut self, code: Code, message: impl Into<String>) {
        self.record(code, message.into(), None, None, None);
    }
    pub fn missing_font(&mut self, family: &str) {
        self.record(
            Code::MissingGlyph,
            format!("Required font is unavailable: {family}"),
            None,
            Some(family.into()),
            None,
        );
    }
    pub fn missing_glyph(&mut self, family: &str, ch: char) {
        self.record(
            Code::MissingGlyph,
            format!(
                "Font {family} lacks a glyph; width is approximate: U+{:04X}",
                ch as u32
            ),
            None,
            Some(family.into()),
            Some(ch),
        );
    }
    pub fn font(&mut self, resolution: FontResolution) {
        self.font_character(resolution, None);
    }
    pub fn font_character(&mut self, resolution: FontResolution, character: Option<char>) {
        let message = format!(
            "Font substitution: {} -> {} ({})",
            resolution.requested_family, resolution.post_script_name, resolution.reason
        );
        self.record(
            Code::FontSubstitution,
            message,
            Some(resolution),
            None,
            character,
        );
    }
    pub fn measured(&mut self, diagnostic: Diagnostic) {
        self.record(
            diagnostic.code,
            diagnostic.message,
            diagnostic.font,
            diagnostic.required_font,
            diagnostic.missing_character,
        );
    }
    fn record(
        &mut self,
        code: Code,
        message: String,
        font: Option<FontResolution>,
        required_font: Option<String>,
        missing_character: Option<char>,
    ) {
        let record = Diagnostic {
            code,
            font,
            required_font,
            missing_character,
            message: message.clone(),
            part: self.target.part.clone(),
            object_id: self.target.object_id.clone(),
            object_name: self.target.object_name.clone(),
            location: self.target.location.clone(),
        };
        if !self.records.contains(&record) {
            self.records.push(record);
        }
        if !self.messages.contains(&message) {
            self.messages.push(message);
        }
    }
    /// Keeps only the first record of each distinct message that starts with `prefix`.
    pub fn keep_first_per_message(&mut self, prefix: &str) {
        let mut seen = Vec::new();
        self.records.retain(|record| {
            if !record.message.starts_with(prefix) {
                return true;
            }
            if seen.contains(&record.message) {
                return false;
            }
            seen.push(record.message.clone());
            true
        });
    }
    pub fn into_parts(self) -> (Vec<String>, Vec<Diagnostic>) {
        (self.messages, self.records)
    }
}
