use std::fs;

use codex_session_manager::rollout::{read_rollout_meta, rewrite_session_meta};
use tempfile::tempdir;

#[test]
fn rewrites_only_first_session_meta_line() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("thread.jsonl");
    fs::write(
        &path,
        concat!(
            r#"{"type":"session_meta","payload":{"id":"abc","cwd":"E:\\code\\demo","source":"vscode","model_provider":"openai"}}"#,
            "\n",
            r#"{"type":"event_msg","payload":{"message":"keep E:\\code\\demo in history"}}"#,
            "\n"
        ),
    )
    .unwrap();

    let changed = rewrite_session_meta(&path, |payload| {
        payload.cwd = Some("/mnt/e/code/demo".to_string());
        payload.model_provider = Some("cm".to_string());
        true
    })
    .unwrap();

    assert!(changed);

    let text = fs::read_to_string(&path).unwrap();
    let mut lines = text.lines();
    let first = lines.next().unwrap();
    let second = lines.next().unwrap();

    assert!(first.contains(r#""cwd":"/mnt/e/code/demo""#));
    assert!(first.contains(r#""model_provider":"cm""#));
    assert!(second.contains(r#"keep E:\\code\\demo in history"#));

    let meta = read_rollout_meta(&path).unwrap().unwrap();
    assert_eq!(meta.id.as_deref(), Some("abc"));
    assert_eq!(meta.cwd.as_deref(), Some("/mnt/e/code/demo"));
}

#[test]
fn reads_object_source_session_meta() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("guardian.jsonl");
    fs::write(
        &path,
        r#"{"type":"session_meta","payload":{"id":"guardian-1","cwd":"/mnt/e/code/demo","source":{"subagent":{"other":"guardian"}},"model_provider":"cm"}}"#,
    )
    .unwrap();

    let meta = read_rollout_meta(&path).unwrap().unwrap();

    assert_eq!(meta.id.as_deref(), Some("guardian-1"));
    assert_eq!(meta.cwd.as_deref(), Some("/mnt/e/code/demo"));
    assert_eq!(meta.source, None);
}

#[test]
fn reads_session_meta_without_loading_later_jsonl_bytes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("large-thread.jsonl");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
        br#"{"type":"session_meta","payload":{"id":"large-1","cwd":"/mnt/e/code/demo","source":"cli","model_provider":"cm"}}"#,
    );
    bytes.push(b'\n');
    bytes.extend_from_slice(&[0xff, 0xfe, 0xfd]);
    fs::write(&path, bytes).unwrap();

    let meta = read_rollout_meta(&path).unwrap().unwrap();

    assert_eq!(meta.id.as_deref(), Some("large-1"));
    assert_eq!(meta.cwd.as_deref(), Some("/mnt/e/code/demo"));
}

#[test]
fn preserves_object_source_when_rewriting_session_meta() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("guardian.jsonl");
    fs::write(
        &path,
        r#"{"type":"session_meta","payload":{"id":"guardian-1","cwd":"E:\\code\\demo","source":{"subagent":{"other":"guardian"}},"model_provider":"cm"}}"#,
    )
    .unwrap();

    let changed = rewrite_session_meta(&path, |payload| {
        payload.cwd = Some("/mnt/e/code/demo".to_string());
        true
    })
    .unwrap();

    assert!(changed);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains(r#""cwd":"/mnt/e/code/demo""#));
    assert!(text.contains(r#""source":{"subagent":{"other":"guardian"}}"#));
}
