use std::cell::RefCell;

use codex_session_manager::app_server::{probe_app_server, AppServerTransport};
use codex_session_manager::session_list::SessionSummary;
use serde_json::{json, Value};

#[test]
fn probe_calls_thread_list_and_thread_read_and_reports_missing_local_threads() {
    let transport = FakeTransport {
        calls: RefCell::new(Vec::new()),
    };
    let local = vec![SessionSummary {
        id: "local-1".to_string(),
        title: None,
        first_user_message: None,
        project: None,
        provider: None,
        model: None,
        source: None,
        archived: false,
        updated_at: None,
        rollout_path: None,
        in_session_index: false,
    }];

    let report = probe_app_server(&transport, &local).unwrap();

    assert_eq!(
        transport.calls.into_inner(),
        vec![
            "thread/list".to_string(),
            "thread/read".to_string(),
            "thread/read".to_string()
        ]
    );
    assert_eq!(report.server_thread_count, 2);
    assert_eq!(report.server_threads_missing_locally, vec!["server-only"]);
}

struct FakeTransport {
    calls: RefCell<Vec<String>>,
}

impl AppServerTransport for FakeTransport {
    fn call(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        self.calls.borrow_mut().push(method.to_string());
        match method {
            "thread/list" => Ok(json!({
                "threads": [
                    {"id": "local-1"},
                    {"id": "server-only"}
                ]
            })),
            "thread/read" => Ok(json!({
                "id": params["id"].as_str().unwrap(),
                "ok": true
            })),
            other => anyhow::bail!("unexpected method {other}"),
        }
    }
}
