use std::{
    io::{BufRead as _, BufReader, Write as _}, process::{ChildStdin, ChildStdout, Command, Stdio}
};

use anyhow::Error;

use crate::models::{EmbeddingRequest, EmbeddingResponse};

pub struct EmbeddingClient {
    pub stdin: ChildStdin,
    pub stdout: BufReader<ChildStdout>,
}

impl EmbeddingClient {
    pub async fn make_request(&mut self, req: EmbeddingRequest) -> Result<EmbeddingResponse, Error> {

            let req_json = serde_json::to_string(&req)?;

            // WRITE
            writeln!(self.stdin, "{}", req_json)?;
            self.stdin.flush()?;

            // READ
            let mut line = String::new();
            self.stdout.read_line(&mut line)?;

            let resp = serde_json::from_str(&line)?;
            Ok(resp)
    }
}

pub fn start_piped_server() -> Result<EmbeddingClient, Error> {

    // Drop the "crates/intime-ai/" prefix here
    let python_location = std::env::var("INTIME_PYTHON_EXE")?;
    let script_location = std::env::var("EMBEDDING_SCRIPT")?;

    let mut child = Command::new(python_location)
        .arg(script_location)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    let client = EmbeddingClient {
        stdin,
        stdout: BufReader::new(stdout),
    };

    Ok(client)
}