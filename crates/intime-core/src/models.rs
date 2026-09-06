use std::path::Path;

use blake3::Hash;
use serde::{Deserialize, Serialize};

use crate::time::Timestamp;

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: Timestamp,
    pub data: EventData,
    #[serde(default)]
    pub metadata: EventMetadata,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventMetadata {
    pub window_title: Option<String>,
    pub process_id: Option<u32>,
    pub executable_path: Option<String>,
    pub focused_element: Option<String>,
    pub focused_element_class: Option<String>,
    pub focused_control_type: Option<String>,
    pub automation_id: Option<String>,
    /// Indicates that the UI reported text activity without storing typed text.
    pub text_changed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EventData {
    WindowFocus {
        fingerprint: Hash,
        window_handle: u64,
    },
    TitleChange {
        fingerprint: Hash,
        new_title: String,
        window_handle: u64,
    },
    TextChanged {
        fingerprint: Hash,
        window_handle: u64,
    },
    IdleStart,
    IdleEnd,
    Gap,
    /// Fires when an app is seen first time
    AppSeen {
        fingerprint: Hash,
        details: AppDetails,
        window_handle: u64,
    },
    Background {
        fingerprint: Hash,
    },
}

impl EventData {
    pub fn fingerprint(&self) -> Option<Hash> {
        match self {
            EventData::WindowFocus { fingerprint, .. }
            | EventData::TitleChange { fingerprint, .. }
            | EventData::TextChanged { fingerprint, .. }
            | EventData::AppSeen { fingerprint, .. } => Some(*fingerprint),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::WindowFocus { .. } => "window_focus",
            Self::TitleChange { .. } => "title_change",
            Self::TextChanged { .. } => "text_changed",
            Self::AppSeen { .. } => "app_seen",
            Self::IdleStart => "idle_start",
            Self::IdleEnd => "idle_end",
            Self::Gap => "gap",
            Self::Background { .. } => "background",
        }
    }
    pub fn window_handle(&self) -> Option<u64> {
        match self {
            EventData::WindowFocus { window_handle, .. }
            | EventData::TitleChange { window_handle, .. }
            | EventData::TextChanged { window_handle, .. }
            | EventData::AppSeen { window_handle, .. } => Some(*window_handle),
            // Background has a fingerprint but no active window handle
            _ => None,
        }
    }
}
/// Hacky way of getting the enum as string

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDetails {
    pub title: String,
    pub file_path: String,
    pub aumid: Option<String>,
    pub company_name: Option<String>,
    pub product_name: Option<String>,
    pub version_info: Option<VersionInfo>,
    pub signature_info: Option<SignatureInfo>,
}

fn norm(s: &str) -> Option<String> {
    let v = s.trim().to_string();
    if v.is_empty() { None } else { Some(v) }
}

impl AppDetails {
    /// Stable per app fingerprint to deduplicate all apps and persist through
    /// updates, relocation/reinstallation or portable apps
    pub fn fingerprint(&self) -> Hash {
        let h = {
            let mut h = blake3::Hasher::new();
            if let Some(aumid) = self.aumid.as_deref().and_then(norm) {
                h.update(b"aumid\x1f");
                h.update(aumid.as_bytes());
                h
            } else if let Some(company) = self.company() {
                h.update(b"company\x1f");
                h.update(company.as_bytes());
                h.update(b"\x1f");
                h.update(self.display_name().as_bytes());
                h
            } else {
                h.update(b"name\x1f");
                h.update(self.display_name().as_bytes());
                h
            }
        };
        let fingerprint = h.finalize();
        fingerprint
    }

    pub fn company(&self) -> Option<String> {
        if let Some(company) = self.company_name.as_deref().and_then(norm) {
            return Some(company);
        }

        if let Some(publisher) = self
            .signature_info
            .as_ref()
            .and_then(|s| s.publisher.as_deref())
            .and_then(norm)
        {
            return Some(publisher);
        }
        None
    }

    pub fn display_name(&self) -> String {
        if let Some(product) = self.product_name.as_deref().and_then(norm) {
            return product;
        }

        if let Some(original_name) = self
            .version_info
            .as_ref()
            .and_then(|v| v.original_filename.clone())
            .and_then(|s| norm(&s))
        {
            return original_name;
        }

        Path::new(&self.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(norm)
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionInfo {
    pub file_description: Option<String>,
    pub product_version: Option<String>,
    pub file_version: Option<String>,
    pub original_filename: Option<String>,
    pub internal_name: Option<String>,
    pub legal_copyright: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignatureInfo {
    pub publisher: Option<String>,
    pub subject_full: Option<String>,
    pub issuer: Option<String>,
    pub serial_number: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_metadata_round_trips_through_json() {
        let event = Event {
            timestamp: Timestamp::now(),
            data: EventData::WindowFocus {
                fingerprint: blake3::hash(b"editor"),
                window_handle: 42,
            },
            metadata: EventMetadata {
                window_title: Some("mail".into()),
                process_id: Some(123),
                executable_path: Some("/bin/editor".into()),
                focused_element: Some("Body".into()),
                focused_element_class: Some("TextBox".into()),
                focused_control_type: Some("Edit".into()),
                automation_id: Some("body".into()),
                text_changed: true,
            },
        };

        let encoded = serde_json::to_string(&event).expect("event should serialize");
        let decoded: Event = serde_json::from_str(&encoded).expect("event should deserialize");
        assert_eq!(decoded.metadata, event.metadata);
        assert_eq!(decoded.data.name(), "window_focus");
    }

    #[test]
    fn display_name_handles_missing_or_non_utf8_paths() {
        let details = AppDetails {
            title: String::new(),
            file_path: String::new(),
            aumid: None,
            company_name: None,
            product_name: None,
            version_info: None,
            signature_info: None,
        };
        assert_eq!(details.display_name(), "unknown");
    }
}
