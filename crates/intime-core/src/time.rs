use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    pub fn now() -> Self {
        Self(Utc::now())
    }
    pub fn as_datetime(&self) -> DateTime<Utc> {
        self.0
    }
    pub fn to_filename_readable(&self) -> String {
        self.0.format("%Y-%m-%d_%H-%M-%S%.3fZ").to_string()
    }
}
