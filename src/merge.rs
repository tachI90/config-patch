use serde_json::Value;
use tracing;

pub fn merge_all(values: &[Value], array_key: &str) -> Value {
    values
        .iter()
        .cloned()
        .enumerate()
        .reduce(|(i, acc), (_, v)| {
            tracing::debug!(source = i + 1, "Merging source into accumulated result");
            (i + 1, deep_merge(acc, v, array_key))
        })
        .map(|(_, v)| v)
        .unwrap_or(Value::Object(serde_json::Map::new()))
}

fn deep_merge(base: Value, overlay: Value, array_key: &str) -> Value {
    match (base, overlay) {
        (Value::Object(mut base_map), Value::Object(overlay_map)) => {
            for (key, overlay_value) in overlay_map {
                match base_map.remove(&key) {
                    Some(base_value) => {
                        let merged = deep_merge(base_value, overlay_value.clone(), array_key);
                        if is_null_removal(&merged) {
                            tracing::debug!(key = %key, action = "REMOVE", "Null override removed key");
                        } else {
                            tracing::debug!(key = %key, action = "MERGE", "Key merged");
                            base_map.insert(key, merged);
                        }
                    }
                    None => {
                        if is_null_removal(&overlay_value) {
                            tracing::debug!(key = %key, action = "SKIP", "Null value for non-existent key skipped");
                        } else {
                            tracing::debug!(key = %key, action = "ADD", "New key added");
                            base_map.insert(key, overlay_value);
                        }
                    }
                }
            }
            Value::Object(base_map)
        }
        (Value::Array(base_arr), Value::Array(overlay_arr)) => {
            Value::Array(merge_arrays(base_arr, overlay_arr, array_key))
        }
        (_, overlay) => overlay,
    }
}

fn merge_arrays(base: Vec<Value>, overlay: Vec<Value>, array_key: &str) -> Vec<Value> {
    let has_indexable = base
        .iter()
        .chain(overlay.iter())
        .any(|v| array_key_value(v, array_key).is_some());

    if !has_indexable {
        tracing::debug!(array_key = %array_key, action = "REPLACE", "Array replaced - no indexable field found");
        return overlay;
    }

    tracing::debug!(array_key = %array_key, action = "SMART_MERGE", "Array smart merge by key");

    let base_indexed: std::collections::HashMap<String, (usize, Value)> = base
        .iter()
        .enumerate()
        .filter_map(|(i, v)| array_key_value(v, array_key).map(|key| (key, (i, v.clone()))))
        .collect();

    let mut result = Vec::new();
    let mut matched_base_indices = std::collections::HashSet::new();

    for overlay_item in overlay {
        if let Some(key) = array_key_value(&overlay_item, array_key) {
            if let Some((base_idx, base_item)) = base_indexed.get(&key) {
                matched_base_indices.insert(*base_idx);
                let merged = deep_merge(base_item.clone(), overlay_item.clone(), array_key);
                tracing::debug!(key = %key, action = "MATCH", "Array item matched and merged");
                result.push(merged);
            } else {
                tracing::debug!(key = %key, action = "NEW", "Array item added from overlay");
                result.push(overlay_item);
            }
        } else {
            result.push(overlay_item);
        }
    }

    for (i, base_item) in base.into_iter().enumerate() {
        if !matched_base_indices.contains(&i) {
            if let Some(key) = array_key_value(&base_item, array_key) {
                tracing::debug!(key = %key, action = "KEEP", "Array item preserved from base");
            }
            result.push(base_item);
        }
    }

    result
}

fn array_key_value(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => map.get(key).and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }),
        _ => None,
    }
}

fn is_null_removal(value: &Value) -> bool {
    matches!(value, Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_merge_nested_objects() {
        let base = json!({"a": {"b": 1, "c": 2}, "d": 3});
        let overlay = json!({"a": {"b": 10, "e": 5}});
        let result = deep_merge(base, overlay, "name");
        assert_eq!(result, json!({"a": {"b": 10, "c": 2, "e": 5}, "d": 3}));
    }

    #[test]
    fn test_scalar_override() {
        let base = json!({"key": "old"});
        let overlay = json!({"key": "new"});
        let result = deep_merge(base, overlay, "name");
        assert_eq!(result, json!({"key": "new"}));
    }

    #[test]
    fn test_type_conflict_overlay_wins() {
        let base = json!({"key": "string"});
        let overlay = json!({"key": {"nested": "object"}});
        let result = deep_merge(base, overlay, "name");
        assert_eq!(result, json!({"key": {"nested": "object"}}));
    }

    #[test]
    fn test_smart_array_merge() {
        let base = json!([
            {"name": "a", "val": 1},
            {"name": "b", "val": 2}
        ]);
        let overlay = json!([
            {"name": "a", "val": 10},
            {"name": "c", "val": 3}
        ]);
        let result = deep_merge(json!({"items": base}), json!({"items": overlay}), "name");
        assert_eq!(
            result,
            json!({"items": [
                {"name": "a", "val": 10},
                {"name": "c", "val": 3},
                {"name": "b", "val": 2}
            ]})
        );
    }

    #[test]
    fn test_primitive_array_replace() {
        let base = json!({"arr": [1, 2, 3]});
        let overlay = json!({"arr": [4, 5]});
        let result = deep_merge(base, overlay, "name");
        assert_eq!(result, json!({"arr": [4, 5]}));
    }

    #[test]
    fn test_null_removal() {
        let base = json!({"a": 1, "b": 2, "c": 3});
        let overlay = json!({"b": null});
        let result = deep_merge(base, overlay, "name");
        assert_eq!(result, json!({"a": 1, "c": 3}));
    }

    #[test]
    fn test_three_way_merge() {
        let base = json!({"a": 1, "b": 2, "c": 3});
        let new = json!({"a": 10, "d": 4});
        let local = json!({"b": 20});
        let result = merge_all(&[base, new, local], "name");
        assert_eq!(result, json!({"a": 10, "b": 20, "c": 3, "d": 4}));
    }

    #[test]
    fn test_empty_inputs() {
        let result = merge_all(&[], "name");
        assert_eq!(result, json!({}));
    }

    #[test]
    fn test_missing_key_field_fallback() {
        let base = json!([
            {"name": "a", "val": 1},
            {"val": 2}
        ]);
        let overlay = json!([
            {"name": "a", "val": 10},
            {"val": 20}
        ]);
        let result = deep_merge(json!({"items": base}), json!({"items": overlay}), "name");
        assert_eq!(
            result,
            json!({"items": [
                {"name": "a", "val": 10},
                {"val": 20},
                {"val": 2}
            ]})
        );
    }

    #[test]
    fn test_array_key_numeric() {
        let base = json!([
            {"id": 1, "val": "a"},
            {"id": 2, "val": "b"}
        ]);
        let overlay = json!([
            {"id": 1, "val": "updated"}
        ]);
        let result = deep_merge(json!({"items": base}), json!({"items": overlay}), "id");
        assert_eq!(
            result,
            json!({"items": [
                {"id": 1, "val": "updated"},
                {"id": 2, "val": "b"}
            ]})
        );
    }
}
