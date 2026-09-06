use serde::Serialize;

#[derive(thiserror::Error, Debug, Serialize)]
pub enum AppError {
    #[error("Internal error: {0}")]
    // This allows the ? operator to convert anyhow::Error automatically
    Anyhow(
        #[from]
        #[serde(serialize_with = "serialize_anyhow")]
        anyhow::Error,
    ),
}

fn serialize_anyhow<S>(err: &anyhow::Error, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{:#}", err))
}
