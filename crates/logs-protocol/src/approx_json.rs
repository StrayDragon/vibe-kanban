use std::fmt::Write as _;

use json_patch::{Patch, PatchOperation};
use serde_json::{Number, Value};

/// Estimate the length of `serde_json::to_string(value)` without allocating.
///
/// The returned length is intended to be exact for serde_json's default compact
/// JSON formatting (no whitespace).
pub fn approx_json_value_len(value: &Value) -> usize {
    match value {
        Value::Null => 4,        // null
        Value::Bool(true) => 4,  // true
        Value::Bool(false) => 5, // false
        Value::Number(n) => approx_json_number_len(n),
        Value::String(s) => approx_json_string_len(s),
        Value::Array(items) => {
            let mut len = 2usize; // [ ]
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    len = len.saturating_add(1); // ,
                }
                len = len.saturating_add(approx_json_value_len(item));
            }
            len
        }
        Value::Object(map) => {
            let mut len = 2usize; // { }
            for (idx, (k, v)) in map.iter().enumerate() {
                if idx > 0 {
                    len = len.saturating_add(1); // ,
                }
                len = len.saturating_add(approx_json_string_len(k)); // key as JSON string
                len = len.saturating_add(1); // :
                len = len.saturating_add(approx_json_value_len(v));
            }
            len
        }
    }
}

/// Estimate the length of `serde_json::to_string(patch)` without allocating.
///
/// Intended to be exact for serde_json's default compact JSON formatting.
pub fn approx_json_patch_len(patch: &Patch) -> usize {
    let mut len = 2usize; // [ ]
    for (idx, op) in patch.iter().enumerate() {
        if idx > 0 {
            len = len.saturating_add(1); // ,
        }
        len = len.saturating_add(approx_json_patch_op_len(op));
    }
    len
}

fn approx_json_patch_op_len(op: &PatchOperation) -> usize {
    match op {
        PatchOperation::Add(add) => approx_json_op_object_len("add", |fields| {
            fields.path(add.path.as_str());
            fields.value(&add.value);
        }),
        PatchOperation::Remove(remove) => approx_json_op_object_len("remove", |fields| {
            fields.path(remove.path.as_str());
        }),
        PatchOperation::Replace(replace) => approx_json_op_object_len("replace", |fields| {
            fields.path(replace.path.as_str());
            fields.value(&replace.value);
        }),
        PatchOperation::Move(mv) => approx_json_op_object_len("move", |fields| {
            fields.from(mv.from.as_str());
            fields.path(mv.path.as_str());
        }),
        PatchOperation::Copy(copy) => approx_json_op_object_len("copy", |fields| {
            fields.from(copy.from.as_str());
            fields.path(copy.path.as_str());
        }),
        PatchOperation::Test(test) => approx_json_op_object_len("test", |fields| {
            fields.path(test.path.as_str());
            fields.value(&test.value);
        }),
    }
}

struct OpFieldsLen {
    fields_len: usize,
    field_count: usize,
}

impl OpFieldsLen {
    fn new() -> Self {
        Self {
            fields_len: 0,
            field_count: 0,
        }
    }

    fn push_field(&mut self, key: &str, value_len: usize) {
        // Each field is `"key":<value>`
        self.fields_len = self
            .fields_len
            .saturating_add(approx_json_string_len(key))
            .saturating_add(1) // :
            .saturating_add(value_len);
        self.field_count = self.field_count.saturating_add(1);
    }

    fn op(&mut self, op_name: &str) {
        self.push_field("op", approx_json_string_len(op_name));
    }

    fn path(&mut self, path: &str) {
        self.push_field("path", approx_json_string_len(path));
    }

    fn from(&mut self, from: &str) {
        self.push_field("from", approx_json_string_len(from));
    }

    fn value(&mut self, value: &Value) {
        self.push_field("value", approx_json_value_len(value));
    }

    fn into_object_len(self) -> usize {
        // {<fields separated by commas>}
        let commas = self.field_count.saturating_sub(1);
        2usize
            .saturating_add(self.fields_len)
            .saturating_add(commas)
    }
}

fn approx_json_op_object_len(op_name: &str, add_fields: impl FnOnce(&mut OpFieldsLen)) -> usize {
    let mut fields = OpFieldsLen::new();
    fields.op(op_name);
    add_fields(&mut fields);
    fields.into_object_len()
}

fn approx_json_number_len(number: &Number) -> usize {
    struct Counter(usize);

    impl std::fmt::Write for Counter {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.0 = self.0.saturating_add(s.len());
            Ok(())
        }
    }

    let mut counter = Counter(0);
    let _ = write!(&mut counter, "{number}");
    counter.0.max(1)
}

fn approx_json_string_len(s: &str) -> usize {
    // serde_json escapes:
    // - '"' as \" (2 bytes)
    // - '\\' as \\ (2 bytes)
    // - control bytes 0x00..=0x1F:
    //   - \b, \t, \n, \f, \r (2 bytes)
    //   - otherwise \u00XX (6 bytes)
    let mut len = 2usize; // surrounding quotes

    for &b in s.as_bytes() {
        len = len.saturating_add(match b {
            b'"' | b'\\' => 2,
            0x08 | 0x09 | 0x0A | 0x0C | 0x0D => 2,
            0x00..=0x1F => 6,
            _ => 1,
        });
    }

    len
}

#[cfg(test)]
mod tests {
    use proptest::{collection, prelude::*, string::string_regex};
    use serde_json::json;

    use super::*;

    fn json_string_strategy() -> BoxedStrategy<String> {
        collection::vec(any::<char>(), 0..64)
            .prop_map(|chars| chars.into_iter().collect())
            .boxed()
    }

    fn json_number_strategy() -> BoxedStrategy<Number> {
        prop_oneof![
            any::<i64>().prop_map(Number::from),
            any::<u64>().prop_map(Number::from),
            any::<f64>().prop_filter_map("finite f64", Number::from_f64),
        ]
        .boxed()
    }

    fn json_value_strategy() -> BoxedStrategy<Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            json_number_strategy().prop_map(Value::Number),
            json_string_strategy().prop_map(Value::String),
        ]
        .boxed();

        leaf.prop_recursive(4, 256, 8, |inner| {
            prop_oneof![
                collection::vec(inner.clone(), 0..8).prop_map(Value::Array),
                collection::vec((json_string_strategy(), inner), 0..8).prop_map(|pairs| {
                    let mut map = serde_json::Map::new();
                    for (k, v) in pairs {
                        map.insert(k, v);
                    }
                    Value::Object(map)
                }),
            ]
            .boxed()
        })
        .boxed()
    }

    fn json_pointer_strategy() -> BoxedStrategy<String> {
        let token = string_regex("[A-Za-z0-9_.-]{1,16}").expect("valid regex");
        collection::vec(token, 0..4)
            .prop_map(|tokens| {
                if tokens.is_empty() {
                    String::new()
                } else {
                    format!("/{}", tokens.join("/"))
                }
            })
            .boxed()
    }

    fn json_patch_strategy() -> BoxedStrategy<Patch> {
        let value = json_value_strategy();
        let path = json_pointer_strategy();
        let from = json_pointer_strategy();

        let op = prop_oneof![
            (path.clone(), value.clone())
                .prop_map(|(path, value)| { json!({ "op": "add", "path": path, "value": value }) }),
            path.clone()
                .prop_map(|path| json!({ "op": "remove", "path": path })),
            (path.clone(), value.clone()).prop_map(|(path, value)| {
                json!({ "op": "replace", "path": path, "value": value })
            }),
            (from.clone(), path.clone())
                .prop_map(|(from, path)| { json!({ "op": "move", "from": from, "path": path }) }),
            (from, path.clone())
                .prop_map(|(from, path)| { json!({ "op": "copy", "from": from, "path": path }) }),
            (path, value).prop_map(|(path, value)| {
                json!({ "op": "test", "path": path, "value": value })
            }),
        ];

        collection::vec(op, 0..8)
            .prop_filter_map("valid JSON patch", |ops| {
                serde_json::from_value::<Patch>(Value::Array(ops)).ok()
            })
            .boxed()
    }

    #[test]
    fn approx_json_value_len_matches_serde_json() {
        let cases = vec![
            json!(null),
            json!(true),
            json!(false),
            json!(123),
            json!(-456),
            json!(123.5),
            json!("simple"),
            json!("needs\"escape\\and\ncontrol"),
            json!([1, 2, 3, "x"]),
            json!({"a": 1, "b": "two", "nested": {"c": [true, false, null]}}),
        ];

        for value in cases {
            let expected = serde_json::to_string(&value).unwrap().len();
            let got = approx_json_value_len(&value);
            assert_eq!(
                got, expected,
                "value length mismatch for {value:?}: got {got} expected {expected}"
            );
        }
    }

    #[test]
    fn approx_json_patch_len_matches_serde_json() {
        let patch: Patch = serde_json::from_value(json!([
            { "op": "add", "path": "/tasks/1", "value": { "type": "NORMALIZED_ENTRY", "content": "hi" } },
            { "op": "replace", "path": "/workspaces/abc", "value": { "task_id": "t1", "x": [1,2,3] } },
            { "op": "move", "from": "/a", "path": "/b" },
            { "op": "copy", "from": "/c", "path": "/d" },
            { "op": "remove", "path": "/e" },
            { "op": "test", "path": "/f", "value": "needs\"escape\\and\ncontrol" }
        ]))
        .unwrap();

        let expected = serde_json::to_string(&patch).unwrap().len();
        let got = approx_json_patch_len(&patch);
        assert_eq!(
            got, expected,
            "patch length mismatch: got {got} expected {expected}"
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            max_shrink_iters: 0,
            .. ProptestConfig::default()
        })]

        #[test]
        fn approx_json_value_len_matches_serde_json_for_random_values(value in json_value_strategy()) {
            let expected = serde_json::to_string(&value).unwrap().len();
            let got = approx_json_value_len(&value);
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn approx_json_patch_len_matches_serde_json_for_random_patches(patch in json_patch_strategy()) {
            let expected = serde_json::to_string(&patch).unwrap().len();
            let got = approx_json_patch_len(&patch);
            prop_assert_eq!(got, expected);
        }
    }
}
