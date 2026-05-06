use codex_session_manager::session_index::{missing_user_index_entries, SessionIndexEntry};
use codex_session_manager::state_db::ThreadRecord;

#[test]
fn appends_only_visible_user_threads() {
    let threads = vec![
        user_thread("user-1", "vscode", true, false),
        user_thread("guardian-1", "guardian", true, false),
        user_thread("archived-1", "cli", true, true),
        user_thread("no-user-event", "cli", false, false),
    ];
    let existing = vec![SessionIndexEntry {
        id: "already-indexed".to_string(),
        thread_name: Some("Old".to_string()),
        updated_at: Some("2026-05-06T00:00:00Z".to_string()),
    }];

    let missing = missing_user_index_entries(&threads, &existing);

    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].id, "user-1");
}

fn user_thread(id: &str, source: &str, has_user_event: bool, archived: bool) -> ThreadRecord {
    ThreadRecord {
        id: id.to_string(),
        rollout_path: None,
        cwd: Some("/mnt/e/code/demo".to_string()),
        source: Some(source.to_string()),
        model_provider: Some("cm".to_string()),
        model: Some("gpt-5.5".to_string()),
        reasoning_effort: Some("high".to_string()),
        has_user_event,
        archived,
        created_at: None,
        updated_at: Some("2026-05-06T00:00:00Z".to_string()),
        created_at_ms: None,
        updated_at_ms: None,
        title: Some("Demo".to_string()),
        first_user_message: Some("hello".to_string()),
    }
}
