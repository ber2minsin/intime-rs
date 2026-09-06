use anyhow::{Context, Error};
use reqwest::Url;

use crate::models::{EmbeddingRequest, EmbeddingResponse};

#[async_trait::async_trait]
pub trait EmbeddingServer {
    async fn make_request(&mut self, req: EmbeddingRequest) -> Result<EmbeddingResponse, Error>;
}

pub struct EmbeddingService {
    pub base_url: Url,
    client: reqwest::Client,
}

impl EmbeddingService {
    const TEXT_ENDPOINT: &'static str = "/embed/text";
    const IMAGE_ENDPOINT: &'static str = "/embed/image";

    pub fn new(base_url: &str) -> Result<Self, Error> {
        Ok(Self {
            base_url: Url::parse(base_url).context("invalid embedding server URL")?,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait::async_trait]
impl EmbeddingServer for EmbeddingService {
    async fn make_request(&mut self, req: EmbeddingRequest) -> Result<EmbeddingResponse, Error> {
        match &req {
            EmbeddingRequest::Text { .. } => {
                let url = self.base_url.join(Self::TEXT_ENDPOINT)?;
                let resp = self
                    .client
                    .post(url)
                    .json(&req)
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<EmbeddingResponse>()
                    .await?;

                Ok(resp)
            }
            EmbeddingRequest::Image { image_path } => {
                let url = self.base_url.join(Self::IMAGE_ENDPOINT)?;

                let form = reqwest::multipart::Form::new()
                    .file("image_file", image_path)
                    .await?;
                let resp = self
                    .client
                    .post(url)
                    .multipart(form)
                    .send()
                    .await?
                    .json::<EmbeddingResponse>()
                    .await?;

                Ok(resp)
            }
        }
    }
}
