use super::*;

fn snapshot_json(updated_at_ms: u64, activities: &str) -> Vec<u8> {
    format!(
        r#"{{"schema":"{SNAPSHOT_SCHEMA}","updated_at_ms":{updated_at_ms},"degraded":false,"activities":{activities}}}"#
    )
    .into_bytes()
}

#[test]
fn rendered_metrics_and_categories_use_exact_overlapping_lifecycle_groups() {
    let bytes = snapshot_json(
        10_000,
        r#"[
            {"id":"approval","agent":"a3s-code","state":"waiting_approval","confidence":"exact","actions":[{"action":"approve_once","token":"0123456789abcdef0123456789abcdef","target_instance_id":"approval","expires_at_ms":20000}]},
            {"id":"input","agent":"researcher","task":"Choose a source","state":"waiting_input","confidence":"exact"},
            {"id":"failed","agent":"worker","state":"failed","confidence":"exact"},
            {"id":"planning","agent":"planner","state":"planning","confidence":"exact"},
            {"id":"process","agent":"codex","state":"working","confidence":"process"},
            {"id":"completed","agent":"worker","state":"completed","confidence":"exact"},
            {"id":"cancelled","agent":"worker","state":"cancelled","confidence":"exact"}
        ]"#,
    );
    let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();
    let rendered: serde_json::Value =
        serde_json::from_str(&snapshot.render_json().unwrap()).unwrap();

    assert_eq!(rendered["metrics"]["total"], 7);
    assert_eq!(rendered["metrics"]["needs_attention"], 3);
    assert_eq!(rendered["metrics"]["running"], 1);
    assert_eq!(rendered["metrics"]["recent"], 3);
    assert_eq!(rendered["metrics"]["inferred"], 1);
    assert_eq!(
        rendered["activities"][2]["categories"],
        serde_json::json!(["needs_attention", "recent"])
    );
    assert_eq!(
        rendered["activities"][4]["categories"],
        serde_json::json!([])
    );
    assert_eq!(rendered["attention_keys"].as_array().unwrap().len(), 2);
}

#[test]
fn attention_keys_are_stable_per_request_and_change_with_a_new_token() {
    let activities = |token: &str, task: &str| {
        format!(
            r#"[{{"id":"approval","agent":"a3s-code","task":"{task}","state":"waiting_approval","confidence":"exact","actions":[{{"action":"approve_once","token":"{token}","target_instance_id":"approval","expires_at_ms":20000}},{{"action":"deny","token":"{token}","target_instance_id":"approval","expires_at_ms":20000}}]}}]"#
        )
    };
    let first = Snapshot::parse(
        &snapshot_json(
            10_000,
            &activities("0123456789abcdef0123456789abcdef", "First label"),
        ),
        10_000,
    )
    .unwrap();
    let relabeled = Snapshot::parse(
        &snapshot_json(
            10_001,
            &activities("0123456789abcdef0123456789abcdef", "Updated label"),
        ),
        10_001,
    )
    .unwrap();
    let next = Snapshot::parse(
        &snapshot_json(
            10_002,
            &activities("abcdef0123456789abcdef0123456789", "Updated label"),
        ),
        10_002,
    )
    .unwrap();

    let keys = |snapshot: &Snapshot| {
        let rendered: serde_json::Value =
            serde_json::from_str(&snapshot.render_json().unwrap()).unwrap();
        rendered["attention_keys"].clone()
    };
    assert_eq!(keys(&first), keys(&relabeled));
    assert_ne!(keys(&first), keys(&next));
}

#[test]
fn non_approval_controls_do_not_create_attention_expansion_keys() {
    let bytes = snapshot_json(
        10_000,
        r#"[{"id":"invalid-grant","agent":"a3s-code","state":"waiting_approval","confidence":"exact","actions":[{"action":"stop","token":"0123456789abcdef0123456789abcdef","target_instance_id":"invalid-grant","expires_at_ms":20000}]}]"#,
    );
    let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();
    let rendered: serde_json::Value =
        serde_json::from_str(&snapshot.render_json().unwrap()).unwrap();

    assert!(rendered["attention_keys"].as_array().unwrap().is_empty());
    assert_eq!(rendered["metrics"]["needs_attention"], 1);
}

#[test]
fn rendered_parent_progress_counts_direct_exact_settled_children() {
    let bytes = snapshot_json(
        10_000,
        r#"[
            {"id":"parent","agent":"a3s-code","state":"working","confidence":"exact"},
            {"id":"done","parent_id":"parent","agent":"worker","state":"completed","confidence":"exact"},
            {"id":"failed","parent_id":"parent","agent":"worker","state":"failed","confidence":"exact"},
            {"id":"live","parent_id":"parent","agent":"worker","state":"working","confidence":"exact"},
            {"id":"detected","parent_id":"parent","agent":"codex","state":"unknown","confidence":"process"},
            {"id":"grandchild","parent_id":"live","agent":"worker","state":"cancelled","confidence":"exact"}
        ]"#,
    );
    let snapshot = Snapshot::parse(&bytes, 10_000).unwrap();
    let rendered: serde_json::Value =
        serde_json::from_str(&snapshot.render_json().unwrap()).unwrap();

    assert_eq!(
        rendered["activities"][0]["child_progress"],
        serde_json::json!({"settled": 2, "total": 3})
    );
    assert_eq!(
        rendered["activities"][3]["child_progress"],
        serde_json::json!({"settled": 1, "total": 1})
    );
    assert!(rendered["activities"][1]["child_progress"].is_null());
}
