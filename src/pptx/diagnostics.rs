//! 表示用の警告文と、機械判定用の分類・部品・対象を同時に保持する。
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
        self.record(code, message.into(), None);
    }
    pub fn font(&mut self, resolution: FontResolution) {
        let message = format!(
            "フォント代替: {} → {} ({})",
            resolution.requested_family, resolution.post_script_name, resolution.reason
        );
        self.record(Code::FontSubstitution, message, Some(resolution));
    }
    fn record(&mut self, code: Code, message: String, font: Option<FontResolution>) {
        let record = Diagnostic {
            code,
            font,
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
    pub fn into_parts(self) -> (Vec<String>, Vec<Diagnostic>) {
        (self.messages, self.records)
    }
}
