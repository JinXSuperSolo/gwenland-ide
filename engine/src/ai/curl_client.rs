//! Minimal curl-backed HTTP transport for AI provider calls.
//!
//! This keeps TLS and HTTP implementation out of the engine binary by delegating
//! HTTPS to the OS-provided curl command. POST bodies are written through stdin,
//! and stdout is parsed as `-i` headers followed by the response body.

use std::collections::HashMap;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};

use crate::ai::error::AiError;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct CurlResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub struct CurlStreamResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: CurlStream,
}

pub struct CurlStream {
    child: Child,
    stdout: BufReader<ChildStdout>,
    finished: bool,
}

impl CurlStream {
    pub async fn next_bytes(&mut self) -> Result<Option<Vec<u8>>, AiError> {
        if self.finished {
            return Ok(None);
        }

        let mut buf = vec![0; 8192];
        let read = self
            .stdout
            .read(&mut buf)
            .await
            .map_err(|e| AiError::Network(e.to_string()))?;
        if read == 0 {
            self.finished = true;
            let status = self
                .child
                .wait()
                .await
                .map_err(|e| AiError::Network(e.to_string()))?;
            if !status.success() {
                return Err(AiError::Network(format!(
                    "curl exited with status {status}"
                )));
            }
            return Ok(None);
        }

        buf.truncate(read);
        Ok(Some(buf))
    }

    pub async fn read_to_string(mut self) -> Result<String, AiError> {
        let mut body = Vec::new();
        while let Some(bytes) = self.next_bytes().await? {
            body.extend_from_slice(&bytes);
        }
        Ok(String::from_utf8_lossy(&body).into_owned())
    }
}

pub async fn curl_get(
    url: &str,
    headers: &HashMap<String, String>,
) -> Result<CurlResponse, AiError> {
    let response = spawn_curl("GET", url, headers, None).await?;
    let body = response.body.read_to_string().await?;
    Ok(CurlResponse {
        status: response.status,
        headers: response.headers,
        body,
    })
}

pub async fn curl_post(
    url: &str,
    headers: &HashMap<String, String>,
    body: String,
) -> Result<CurlResponse, AiError> {
    let response = curl_post_stream(url, headers, body).await?;
    let body = response.body.read_to_string().await?;
    Ok(CurlResponse {
        status: response.status,
        headers: response.headers,
        body,
    })
}

pub async fn curl_post_stream(
    url: &str,
    headers: &HashMap<String, String>,
    body: String,
) -> Result<CurlStreamResponse, AiError> {
    spawn_curl("POST", url, headers, Some(body)).await
}

async fn spawn_curl(
    method: &str,
    url: &str,
    headers: &HashMap<String, String>,
    body: Option<String>,
) -> Result<CurlStreamResponse, AiError> {
    let mut command = Command::new(curl_bin());
    command.arg("-sS").arg("-N").arg("-i").arg("-X").arg(method);

    for (name, value) in headers {
        validate_header(name, value)?;
        command.arg("-H").arg(format!("{name}: {value}"));
    }

    if body.is_some() {
        command.arg("--data-binary").arg("@-");
    }

    command
        .arg(url)
        .stdin(if body.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command
        .spawn()
        .map_err(|e| AiError::Network(format!("failed to spawn curl: {e}")))?;

    if let Some(body) = body {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AiError::Network("failed to open curl stdin".into()))?;
        stdin
            .write_all(body.as_bytes())
            .await
            .map_err(|e| AiError::Network(e.to_string()))?;
        stdin
            .shutdown()
            .await
            .map_err(|e| AiError::Network(e.to_string()))?;
    }

    drop(child.stdin.take());

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AiError::Network("failed to open curl stdout".into()))?;
    let mut stream = CurlStream {
        child,
        stdout: BufReader::new(stdout),
        finished: false,
    };
    let (status, headers) = read_headers(&mut stream).await?;

    Ok(CurlStreamResponse {
        status,
        headers,
        body: stream,
    })
}

async fn read_headers(stream: &mut CurlStream) -> Result<(u16, HashMap<String, String>), AiError> {
    let mut headers = HeaderAccumulator::default();

    loop {
        let mut line = Vec::new();
        let read = stream
            .stdout
            .read_until(b'\n', &mut line)
            .await
            .map_err(|e| AiError::Network(e.to_string()))?;

        if read == 0 {
            stream.finished = true;
            let exit = stream
                .child
                .wait()
                .await
                .map_err(|e| AiError::Network(e.to_string()))?;
            return Err(AiError::Network(format!(
                "curl exited before response headers with status {exit}"
            )));
        }

        trim_line_end(&mut line);
        let text = String::from_utf8_lossy(&line);
        if let Some(done) = headers.accept_line(&text) {
            return Ok(done);
        }
    }
}

fn parse_status_line_parts(line: &str) -> Option<(u16, String)> {
    let mut parts = line.split_whitespace();
    let protocol = parts.next()?;
    if !protocol.starts_with("HTTP/") {
        return None;
    }
    let code = parts.next()?.parse().ok()?;
    Some((code, parts.collect::<Vec<_>>().join(" ")))
}

#[derive(Default)]
struct HeaderAccumulator {
    status: Option<u16>,
    reason: String,
    headers: HashMap<String, String>,
}

impl HeaderAccumulator {
    fn accept_line(&mut self, line: &str) -> Option<(u16, HashMap<String, String>)> {
        if line.is_empty() {
            let code = self.status?;
            if code == 100 || self.is_proxy_connect_tunnel() {
                self.status = None;
                self.reason.clear();
                self.headers.clear();
                return None;
            }
            return Some((code, std::mem::take(&mut self.headers)));
        }

        if let Some((code, reason)) = parse_status_line_parts(line) {
            self.status = Some(code);
            self.reason = reason;
            self.headers.clear();
            return None;
        }

        if let Some((name, value)) = line.split_once(':') {
            self.headers
                .insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
        None
    }

    fn is_proxy_connect_tunnel(&self) -> bool {
        self.status == Some(200)
            && self.headers.is_empty()
            && self.reason.to_ascii_lowercase().contains("connection")
            && self.reason.to_ascii_lowercase().contains("established")
    }
}

fn trim_line_end(line: &mut Vec<u8>) {
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
}

fn validate_header(name: &str, value: &str) -> Result<(), AiError> {
    if name.is_empty()
        || name.bytes().any(|b| b == b':' || b == b'\r' || b == b'\n')
        || value.bytes().any(|b| b == b'\r' || b == b'\n')
    {
        return Err(AiError::ProviderError("invalid HTTP header".into()));
    }
    Ok(())
}

fn curl_bin() -> &'static str {
    #[cfg(windows)]
    {
        "curl.exe"
    }
    #[cfg(not(windows))]
    {
        "curl"
    }
}

#[cfg(test)]
mod tests {
    use super::{HeaderAccumulator, parse_status_line_parts, trim_line_end};

    #[test]
    fn parses_http_status_lines() {
        assert_eq!(
            parse_status_line_parts("HTTP/1.1 200 OK"),
            Some((200, "OK".to_string()))
        );
        assert_eq!(
            parse_status_line_parts("HTTP/2 429"),
            Some((429, String::new()))
        );
        assert_eq!(parse_status_line_parts("nope"), None);
    }

    #[test]
    fn trims_crlf() {
        let mut line = b"header: value\r\n".to_vec();
        trim_line_end(&mut line);
        assert_eq!(line, b"header: value");
    }

    #[test]
    fn parses_response_status_and_headers() {
        let mut h = HeaderAccumulator::default();
        assert_eq!(h.accept_line("HTTP/2 429"), None);
        assert_eq!(h.accept_line("Retry-After: 42"), None);
        assert_eq!(h.accept_line("Content-Type: application/json"), None);

        let (status, headers) = h.accept_line("").expect("blank line completes headers");
        assert_eq!(status, 429);
        assert_eq!(headers["retry-after"], "42");
        assert_eq!(headers["content-type"], "application/json");
    }

    #[test]
    fn skips_interim_and_proxy_connect_header_blocks() {
        let mut h = HeaderAccumulator::default();
        h.accept_line("HTTP/1.1 200 Connection established");
        assert_eq!(h.accept_line(""), None);
        h.accept_line("HTTP/1.1 100 Continue");
        assert_eq!(h.accept_line(""), None);
        h.accept_line("HTTP/2 200");

        let (status, headers) = h
            .accept_line("")
            .expect("final empty header block is valid");
        assert_eq!(status, 200);
        assert!(headers.is_empty());
    }
}
