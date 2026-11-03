//! Reading the servers' JSON. The two servers, and two endpoints of one, spell a field in different cases (`Key` and `key`),
//! so a key is matched without regard to case (spec 0). A key spelled twice with different values is no key at all: a server
//! that sends both is not to be believed about either.

use serde_json::{Map, Value};

/// The value of `name` in an object, matched without regard to ASCII case
pub fn get<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    let object = value.as_object()?;
    if let Some(found) = object.get(name) {
        let twice = object.iter().any(|(key, other)| key != name && key.eq_ignore_ascii_case(name) && other != found);
        return (!twice).then_some(found);
    }
    let mut found = None;
    for (key, other) in object {
        if key.eq_ignore_ascii_case(name) {
            if found.is_some_and(|seen| seen != other) {
                return None;
            }
            found = Some(other);
        }
    }
    found
}

/// A non-null value
pub fn some<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    get(value, name).filter(|found| !found.is_null())
}

pub fn text<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    get(value, name)?.as_str()
}

/// A path of names, each matched without regard to case
pub fn path<'a>(value: &'a Value, names: &[&str]) -> Option<&'a Value> {
    names.iter().try_fold(value, |at, name| some(at, name))
}

pub fn number(value: &Value, name: &str) -> Option<u64> {
    get(value, name)?.as_u64()
}

/// The key in this object that `name` matches, as the server spelled it
pub fn key_of(object: &Map<String, Value>, name: &str) -> Option<String> {
    object.keys().find(|key| key.eq_ignore_ascii_case(name)).cloned()
}

/// Takes out every key that `name` matches
pub fn remove(object: &mut Map<String, Value>, name: &str) {
    object.retain(|key, _| !key.eq_ignore_ascii_case(name));
}

/// Sets `name`, in the spelling given here, in place of any other spelling of it
pub fn set(object: &mut Map<String, Value>, name: &str, value: Value) {
    remove(object, name);
    object.insert(name.to_string(), value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_key_is_read_whatever_its_case_and_never_when_said_twice() {
        let value = json!({"Key": "a", "kdfIterations": 5, "Both": 1, "both": 2, "Same": 3, "same": 3});
        assert_eq!(get(&value, "key"), Some(&json!("a")));
        assert_eq!(number(&value, "KdfIterations"), Some(5));
        assert_eq!(get(&value, "both"), None);
        assert_eq!(get(&value, "BOTH"), None);
        assert_eq!(get(&value, "same"), Some(&json!(3)));
        assert_eq!(path(&json!({"A": {"b": {"C": 1}}}), &["a", "B", "c"]), Some(&json!(1)));
        let mut object = json!({"Key": 1, "other": 2}).as_object().unwrap().clone();
        set(&mut object, "key", json!(3));
        assert_eq!(Value::Object(object), json!({"key": 3, "other": 2}));
    }
}
