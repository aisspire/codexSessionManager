use std::io::{self, Read};

use codex_session_manager::profile_operation::{
    execute_profile_operation, BridgeIdentity, BridgeRequest, BridgeResponse,
    WSL_BRIDGE_PROTOCOL_VERSION, WSL_BRIDGE_RESPONSE_MARKER,
};

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--protocol-version") => {
            println!("{WSL_BRIDGE_PROTOCOL_VERSION}");
            return;
        }
        Some("--identity") => {
            match BridgeIdentity::current()
                .and_then(|identity| serde_json::to_string(&identity).map_err(Into::into))
            {
                Ok(identity) => println!("{identity}"),
                Err(error) => {
                    eprintln!("failed to report WSL bridge identity: {error:?}");
                    std::process::exit(2);
                }
            }
            return;
        }
        _ => {}
    }

    let response = run().unwrap_or_else(|error| BridgeResponse::failure(format!("{error:?}")));
    let ok = response.ok;
    let encoded = serde_json::to_string(&response).unwrap_or_else(|error| {
        format!(
            r#"{{"protocol_version":{WSL_BRIDGE_PROTOCOL_VERSION},"ok":false,"error":"failed to encode bridge response: {error}"}}"#
        )
    });
    println!("{WSL_BRIDGE_RESPONSE_MARKER}{encoded}");
    if !ok {
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<BridgeResponse> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: BridgeRequest = serde_json::from_str(&input)?;
    if request.operation.requires_codex_stopped() {
        codex_session_manager::safety::ensure_codex_not_running()?;
    }
    execute_profile_operation(&request).map(BridgeResponse::success)
}
