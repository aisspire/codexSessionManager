use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::TcpStream;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::session_list::SessionSummary;

pub trait AppServerTransport {
    fn call(&self, method: &str, params: Value) -> Result<Value>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppServerProbeReport {
    pub server_thread_count: usize,
    pub read_count: usize,
    pub server_threads_missing_locally: Vec<String>,
    pub local_threads_missing_on_server: Vec<String>,
}

impl AppServerProbeReport {
    pub fn to_text(&self) -> String {
        [
            "action: app-server probe".to_string(),
            format!("server threads: {}", self.server_thread_count),
            format!("thread/read calls: {}", self.read_count),
            format!(
                "server threads missing locally: {}",
                self.server_threads_missing_locally.len()
            ),
            format!(
                "local threads missing on server: {}",
                self.local_threads_missing_on_server.len()
            ),
        ]
        .join("\n")
    }
}

pub fn probe_app_server<T: AppServerTransport>(
    transport: &T,
    local_sessions: &[SessionSummary],
) -> Result<AppServerProbeReport> {
    let response = transport.call("thread/list", json!({}))?;
    let server_ids = parse_thread_ids(&response)?;
    let mut read_count = 0;
    for id in &server_ids {
        transport.call("thread/read", json!({ "id": id }))?;
        read_count += 1;
    }

    let local_ids = local_sessions
        .iter()
        .map(|session| session.id.as_str())
        .collect::<HashSet<_>>();
    let server_id_set = server_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    Ok(AppServerProbeReport {
        server_thread_count: server_ids.len(),
        read_count,
        server_threads_missing_locally: server_ids
            .iter()
            .filter(|id| !local_ids.contains(id.as_str()))
            .cloned()
            .collect(),
        local_threads_missing_on_server: local_sessions
            .iter()
            .filter(|session| !server_id_set.contains(session.id.as_str()))
            .map(|session| session.id.clone())
            .collect(),
    })
}

fn parse_thread_ids(value: &Value) -> Result<Vec<String>> {
    let threads = value
        .get("threads")
        .and_then(Value::as_array)
        .context("thread/list response missing threads array")?;
    threads
        .iter()
        .map(|thread| {
            thread
                .get("id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .context("thread/list thread missing id")
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct HttpAppServerTransport {
    endpoint: String,
}

impl HttpAppServerTransport {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }
}

impl AppServerTransport for HttpAppServerTransport {
    fn call(&self, method: &str, params: Value) -> Result<Value> {
        let body = json!({ "method": method, "params": params }).to_string();
        post_json(&self.endpoint, &body)
    }
}

fn post_json(endpoint: &str, body: &str) -> Result<Value> {
    let Some(rest) = endpoint.strip_prefix("http://") else {
        bail!("only http:// app-server endpoints are supported");
    };
    let (host_port, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path = format!("/{}", path);
    let (host, port) = host_port.split_once(':').unwrap_or((host_port, "80"));
    let mut stream = TcpStream::connect(format!("{host}:{port}"))
        .with_context(|| format!("failed to connect to {endpoint}"))?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes())?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let Some((_, body)) = response.split_once("\r\n\r\n") else {
        bail!("invalid HTTP response from app-server");
    };
    serde_json::from_str(body).context("failed to parse app-server JSON response")
}
