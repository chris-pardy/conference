//! A lexicon validator for our own lexicons, mirroring `@atproto/lexicon`: it
//! checks in the same order and stops at the same first error, so the backend
//! and the PWA reject a card at the same JSON pointer for the same reason.
//!
//! It covers what `lexicons/` uses: objects, arrays, refs, open and closed
//! unions, `unknown`, booleans, integers and strings with their length,
//! enum, const and format constraints. Blobs, bytes and cid-links aren't used
//! and are rejected as unsupported.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use crate::CardError;

const LEXICONS: &[&str] = &[
    include_str!("../../../lexicons/app/gather/block/card.json"),
    include_str!("../../../lexicons/app/gather/block/defs.json"),
];

/// Every def by its full URI, `nsid#name` (`nsid#main` for the main def).
static DEFS: LazyLock<HashMap<String, Value>> = LazyLock::new(|| {
    let mut defs = HashMap::new();
    for doc in LEXICONS {
        let doc: Value = serde_json::from_str(doc).expect("the lexicons are valid JSON");
        let id = doc["id"].as_str().expect("a lexicon has an id").to_owned();
        for (name, def) in doc["defs"].as_object().expect("a lexicon has defs") {
            defs.insert(format!("{id}#{name}"), resolve_local_refs(&id, def.clone()));
        }
    }
    defs
});

/// Rewrites `#name` refs to `nsid#name`, so every ref is a key into DEFS.
fn resolve_local_refs(id: &str, value: Value) -> Value {
    let full = |r: &str| match r.strip_prefix('#') {
        Some(name) => format!("{id}#{name}"),
        None if r.contains('#') => r.to_owned(),
        None => format!("{r}#main"),
    };
    match value {
        Value::Object(mut map) => {
            if let Some(Value::String(r)) = map.get("ref") {
                let r = full(r);
                map.insert("ref".into(), Value::String(r));
            }
            if let Some(Value::Array(refs)) = map.get("refs") {
                let refs =
                    refs.iter().filter_map(Value::as_str).map(|r| Value::String(full(r))).collect();
                map.insert("refs".into(), Value::Array(refs));
            }
            Value::Object(map.into_iter().map(|(k, v)| (k, resolve_local_refs(id, v))).collect())
        }
        Value::Array(items) => {
            Value::Array(items.into_iter().map(|v| resolve_local_refs(id, v)).collect())
        }
        other => other,
    }
}

fn def(uri: &str) -> &'static Value {
    DEFS.get(uri).unwrap_or_else(|| panic!("the lexicons define {uri}"))
}

/// Where a value sits: its path segments, for the pointer and the message.
#[derive(Clone)]
struct Path(Vec<String>);

impl Path {
    fn child(&self, segment: impl Into<String>) -> Path {
        let mut segments = self.0.clone();
        segments.push(segment.into());
        Path(segments)
    }

    fn pointer(&self) -> String {
        self.0.iter().map(|s| format!("/{}", s.replace('~', "~0").replace('/', "~1"))).collect()
    }

    fn fail(&self, reason: &str, message: impl AsRef<str>) -> CardError {
        let pointer = self.pointer();
        let shown = if pointer.is_empty() { "/" } else { &pointer };
        CardError {
            path: pointer.clone(),
            reason: reason.to_owned(),
            message: format!("{shown} {}", message.as_ref()),
        }
    }
}

type Result = std::result::Result<(), CardError>;

/// Validates a record against the record def at `uri` (`nsid#main`).
pub(crate) fn validate_record(uri: &str, value: &Value) -> Result {
    object(&Path(Vec::new()), &def(uri)["record"], value)
}

fn validate(path: &Path, def: &Value, value: &Value) -> Result {
    match def["type"].as_str() {
        Some("object") => object(path, def, value),
        Some("array") => array(path, def, value),
        Some("boolean") => boolean(path, def, value),
        Some("integer") => integer(path, def, value),
        Some("string") => string(path, def, value),
        Some("unknown") => unknown(path, value),
        other => Err(path.fail("invalid", format!("has an unsupported lexicon type {other:?}"))),
    }
}

fn one_of(path: &Path, def: &Value, value: &Value) -> Result {
    match def["type"].as_str() {
        Some("union") => {
            let Some(type_) =
                value.as_object().and_then(|o| o.get("$type")).and_then(Value::as_str)
            else {
                return Err(path
                    .child("$type")
                    .fail("required", "must be an object which includes the \"$type\" property"));
            };
            let type_ =
                if type_.contains('#') { type_.to_owned() } else { format!("{type_}#main") };
            let refs: Vec<&str> =
                def["refs"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            if refs.contains(&type_.as_str()) {
                validate(path, self::def(&type_), value)
            } else if def["closed"].as_bool() == Some(true) {
                Err(path.fail("union", format!("$type must be one of {}", refs.join(", "))))
            } else {
                // Open unions let newer types through; older readers skip them.
                Ok(())
            }
        }
        Some("ref") => validate(path, self::def(def["ref"].as_str().unwrap_or_default()), value),
        _ => validate(path, def, value),
    }
}

fn object(path: &Path, def: &Value, value: &Value) -> Result {
    let Some(obj) = value.as_object() else {
        return Err(path.fail("type", "must be an object"));
    };
    let required =
        |key: &str| def["required"].as_array().is_some_and(|r| r.iter().any(|k| k == key));
    let empty = Map::new();
    for (key, prop) in def["properties"].as_object().unwrap_or(&empty) {
        match obj.get(key) {
            None if required(key) => {
                return Err(path
                    .child(key.as_str())
                    .fail("required", format!("must have the property \"{key}\"")));
            }
            None => {}
            Some(Value::Null)
                if def["nullable"].as_array().is_some_and(|n| n.iter().any(|k| k == key)) => {}
            Some(v) => one_of(&path.child(key.as_str()), prop, v)?,
        }
    }
    Ok(())
}

fn array(path: &Path, def: &Value, value: &Value) -> Result {
    let Some(items) = value.as_array() else {
        return Err(path.fail("type", "must be an array"));
    };
    if let Some(max) = def["maxLength"].as_u64()
        && items.len() as u64 > max
    {
        return Err(path.fail("max-length", format!("must not have more than {max} elements")));
    }
    if let Some(min) = def["minLength"].as_u64()
        && (items.len() as u64) < min
    {
        return Err(path.fail("min-length", format!("must not have fewer than {min} elements")));
    }
    for (i, item) in items.iter().enumerate() {
        one_of(&path.child(i.to_string()), &def["items"], item)?;
    }
    Ok(())
}

fn boolean(path: &Path, def: &Value, value: &Value) -> Result {
    let Some(b) = value.as_bool() else {
        return Err(path.fail("type", "must be a boolean"));
    };
    match def["const"].as_bool() {
        Some(c) if c != b => Err(path.fail("const", format!("must be {c}"))),
        _ => Ok(()),
    }
}

fn integer(path: &Path, def: &Value, value: &Value) -> Result {
    // JSON numbers like 3.0 are integers to JavaScript too.
    let n = match value {
        Value::Number(n) => {
            n.as_i64().or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))
        }
        _ => None,
    };
    let Some(n) = n else {
        return Err(path.fail("type", "must be an integer"));
    };
    if let Some(c) = def["const"].as_i64()
        && c != n
    {
        return Err(path.fail("const", format!("must be {c}")));
    }
    if let Some(values) = def["enum"].as_array()
        && !values.iter().any(|v| v.as_i64() == Some(n))
    {
        return Err(path.fail("enum", format!("must be one of ({})", join(values))));
    }
    if let Some(max) = def["maximum"].as_i64()
        && n > max
    {
        return Err(path.fail("maximum", format!("can not be greater than {max}")));
    }
    if let Some(min) = def["minimum"].as_i64()
        && n < min
    {
        return Err(path.fail("minimum", format!("can not be less than {min}")));
    }
    Ok(())
}

fn string(path: &Path, def: &Value, value: &Value) -> Result {
    let Some(s) = value.as_str() else {
        return Err(path.fail("type", "must be a string"));
    };
    if let Some(c) = def["const"].as_str()
        && c != s
    {
        return Err(path.fail("const", format!("must be {c}")));
    }
    if let Some(values) = def["enum"].as_array()
        && !values.iter().any(|v| v.as_str() == Some(s))
    {
        return Err(path.fail("enum", format!("must be one of ({})", join(values))));
    }
    // Lengths are UTF-8 bytes, as in the atproto data model.
    if let Some(max) = def["maxLength"].as_u64()
        && s.len() as u64 > max
    {
        return Err(path.fail("max-length", format!("must not be longer than {max} characters")));
    }
    if let Some(min) = def["minLength"].as_u64()
        && (s.len() as u64) < min
    {
        return Err(path.fail("min-length", format!("must not be shorter than {min} characters")));
    }
    if def.get("maxGraphemes").is_some() || def.get("minGraphemes").is_some() {
        return Err(path.fail("invalid", "uses graphemes, which this validator doesn't support"));
    }
    match def["format"].as_str() {
        None => Ok(()),
        Some(format) => {
            let (ok, message) = match format {
                "datetime" => (
                    is_datetime(s),
                    "must be an valid atproto datetime (both RFC-3339 and ISO-8601)",
                ),
                "uri" => (URI.is_match(s), "must be a uri"),
                "did" => (s.len() <= 2048 && DID.is_match(s), "must be a valid did"),
                "nsid" => {
                    (s.len() <= 317 && s.len() >= 5 && NSID.is_match(s), "must be a valid nsid")
                }
                "cid" => (is_cid(s), "must be a cid string"),
                "record-key" => {
                    (s != "." && s != ".." && RECORD_KEY.is_match(s), "must be a valid Record Key")
                }
                other => {
                    return Err(
                        path.fail("invalid", format!("uses the unsupported format {other}"))
                    );
                }
            };
            if ok { Ok(()) } else { Err(path.fail("format", message)) }
        }
    }
}

fn unknown(path: &Path, value: &Value) -> Result {
    // As in @atproto/lexicon: any object, arrays included.
    if value.is_object() || value.is_array() {
        Ok(())
    } else {
        Err(path.fail("type", "must be an object"))
    }
}

fn join(values: &[Value]) -> String {
    values
        .iter()
        .map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()))
        .collect::<Vec<_>>()
        .join("|")
}

// The patterns @atproto/syntax uses.
static URI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\w+:(?://)?[^\s/][^\s]*$").unwrap());
static DID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^did:[a-z]+:[a-zA-Z0-9._:%-]*[a-zA-Z0-9._-]$").unwrap());
static NSID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[a-zA-Z](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)+(?:\.[a-zA-Z](?:[a-zA-Z0-9]{0,62})?)$",
    )
    .unwrap()
});
static RECORD_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_~.:-]{1,512}$").unwrap());
static DATETIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}(?:[Tt ]\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:[Zz]|[+-]\d{2}(?::?\d{2})?)?)?$").unwrap()
});
static CID_V1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^b[a-z2-7]{8,}$").unwrap());
static CID_V0: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Qm[1-9A-HJ-NP-Za-km-z]{44}$").unwrap());

/// An ISO-8601 date or date-time, with its fields in range.
fn is_datetime(s: &str) -> bool {
    if !DATETIME.is_match(s) {
        return false;
    }
    let num = |range: std::ops::Range<usize>| {
        s.get(range).and_then(|p| p.parse::<u32>().ok()).unwrap_or(99)
    };
    let (month, day) = (num(5..7), num(8..10));
    let time_ok = s.len() < 16
        || (num(11..13) < 24 && num(14..16) < 60 && (s.len() < 19 || num(17..19) < 61));
    (1..=12).contains(&month) && (1..=31).contains(&day) && time_ok
}

/// A CIDv1 in base32 or a CIDv0 in base58: the string forms records use.
fn is_cid(s: &str) -> bool {
    CID_V1.is_match(s) || CID_V0.is_match(s)
}
