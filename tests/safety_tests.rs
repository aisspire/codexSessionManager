use codex_session_manager::safety::{
    detect_codex_processes_from_lines, ensure_codex_not_running_with,
};

#[test]
fn detects_codex_desktop_cli_and_app_server_processes() {
    let processes = detect_codex_processes_from_lines(&[
        "/usr/bin/bash".to_string(),
        "/opt/Codex/Codex Desktop --app-server".to_string(),
        "node /home/me/.npm/bin/codex".to_string(),
        "codex app-server --port 12345".to_string(),
    ]);

    assert_eq!(processes.len(), 3);
    assert!(processes.iter().any(|process| process.kind == "desktop"));
    assert!(processes.iter().any(|process| process.kind == "cli"));
    assert!(processes.iter().any(|process| process.kind == "app-server"));
}

#[test]
fn blocks_writes_when_codex_is_running() {
    let result = ensure_codex_not_running_with(|| {
        Ok(detect_codex_processes_from_lines(&[
            "codex app-server --port 12345".to_string(),
        ]))
    });

    assert!(result.is_err());
    let message = format!("{:?}", result.unwrap_err());
    assert!(message.contains("Codex appears to be running"));
}
