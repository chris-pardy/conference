//! A lexicon validator for our own lexicons, mirroring `@atproto/lexicon`: it
//! checks in the same order and stops at the same first error, so the backend
//! and the PWA reject a card at the same JSON pointer for the same reason.
//!
//! It covers what `lexicons/` uses: objects, arrays, refs, open and closed
//! unions, `unknown`, booleans, integers and strings with their length,
//! enum, const and format constraints. Blobs, bytes and cid-links aren't used
//! and are rejected as unsupported.

use std::collections::{HashMap, HashSet};
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

// What `value[key]` finds on a JavaScript array or plain object without that
// own key: inherited methods. A function fails every lexicon type check, as
// JSON null does, so null stands in for one.
const OBJECT_PROTO: &[&str] = &[
    "constructor",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toLocaleString",
    "toString",
    "valueOf",
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
    "__proto__",
];
const ARRAY_PROTO: &[&str] = &[
    "at",
    "concat",
    "copyWithin",
    "entries",
    "every",
    "fill",
    "filter",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "flat",
    "flatMap",
    "forEach",
    "includes",
    "indexOf",
    "join",
    "keys",
    "lastIndexOf",
    "map",
    "pop",
    "push",
    "reduce",
    "reduceRight",
    "reverse",
    "shift",
    "slice",
    "some",
    "sort",
    "splice",
    "toReversed",
    "toSorted",
    "toSpliced",
    "unshift",
    "values",
    "with",
];

/// `value[key]` as JavaScript reads it from a parsed JSON object or array.
fn property(value: &Value, key: &str) -> Option<Value> {
    let inherited = |extra: &[&str]| {
        (OBJECT_PROTO.contains(&key) || extra.contains(&key)).then_some(Value::Null)
    };
    match value {
        Value::Object(obj) => obj.get(key).cloned().or_else(|| inherited(&[])),
        Value::Array(items) => {
            let index = key.parse::<usize>().ok().filter(|i| i.to_string() == key);
            match index {
                Some(i) => items.get(i).cloned(),
                None if key == "length" => Some(Value::from(items.len())),
                None => inherited(ARRAY_PROTO),
            }
        }
        _ => None,
    }
}

fn object(path: &Path, def: &Value, value: &Value) -> Result {
    // As in @atproto/lexicon, an array is an object too: its properties are
    // read the way JavaScript would.
    if !value.is_object() && !value.is_array() {
        return Err(path.fail("type", "must be an object"));
    }
    let required =
        |key: &str| def["required"].as_array().is_some_and(|r| r.iter().any(|k| k == key));
    let empty = Map::new();
    for (key, prop) in def["properties"].as_object().unwrap_or(&empty) {
        match property(value, key).as_ref() {
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
                    crate::datetime::is_datetime_lenient(s),
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

// The patterns @atproto/syntax uses. JavaScript's `\w` is ASCII and its `\s`
// is its own set, unlike Rust's Unicode classes, so both are spelled out.
static URI: LazyLock<Regex> = LazyLock::new(|| {
    let space = r"\t\n\x0B\x0C\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";
    Regex::new(&format!(r"^[A-Za-z0-9_]+:(?://)?[^{space}/][^{space}]*$")).unwrap()
});
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
/// A CID string as multiformats' `CID.parse` takes it: a bare base58 CIDv0
/// (`Q…`), or a CIDv1 in base58btc (`z`), base32 (`b`) or base36 (`k`), with
/// no bytes left over.
fn is_cid(s: &str) -> bool {
    use cid::multibase::Base;
    let base = match s.chars().next() {
        Some('Q') => None,
        Some('z') => Some(Base::Base58Btc),
        Some('b') => Some(Base::Base32Lower),
        Some('k') => Some(Base::Base36Lower),
        _ => return false,
    };
    let Ok(parsed) = cid::Cid::try_from(s) else {
        return false;
    };
    // Re-encoding must give the string back: that rules out trailing bytes,
    // and a CIDv0 behind a multibase prefix, which multiformats rejects.
    match (base, parsed.version()) {
        (None, cid::Version::V0) => parsed.to_string() == s,
        (Some(base), cid::Version::V1) => parsed.to_string_of_base(base).is_ok_and(|e| e == s),
        _ => false,
    }
}

const DEFS_PREFIX: &str = "app.gather.block.defs#";

/// How deeply blocks may nest (mirrors `MAX_DEPTH` in web/src/blocks/limits.ts).
pub(crate) const MAX_DEPTH: usize = 10;

/// The first block nested deeper than `MAX_DEPTH`, in document order. It
/// walks with its own stack, not recursion, and runs before the lexicon check
/// (which recurses), so no card can overflow the call stack. Children are a
/// block's `blocks`, its columns' `blocks`, its `template`, then its sheet's
/// `blocks`, as in `checkDepth` in web/src/blocks/validate.ts.
pub(crate) fn check_depth(card: &Value) -> Result {
    let mut stack: Vec<(&Value, String, usize)> = Vec::new();
    fn push<'a>(
        stack: &mut Vec<(&'a Value, String, usize)>,
        list: &'a Value,
        path: &str,
        depth: usize,
    ) {
        if let Some(items) = list.as_array() {
            for (i, item) in items.iter().enumerate().rev() {
                stack.push((item, format!("{path}/{i}"), depth));
            }
        }
    }
    push(&mut stack, &card["blocks"], "/blocks", 1);
    while let Some((block, at, depth)) = stack.pop() {
        if !block.is_object() {
            continue;
        }
        if depth > MAX_DEPTH {
            return Err(CardError {
                message: format!("{at} is nested more than {MAX_DEPTH} blocks deep"),
                path: at,
                reason: "max-depth".into(),
            });
        }
        let mut children: Vec<(&Value, String)> = vec![(&block["blocks"], format!("{at}/blocks"))];
        for (j, column) in block["columns"].as_array().into_iter().flatten().enumerate() {
            children.push((&column["blocks"], format!("{at}/columns/{j}/blocks")));
        }
        children.push((&block["template"], format!("{at}/template")));
        children.push((&block["opens"]["blocks"], format!("{at}/opens/blocks")));
        for (list, path) in children.into_iter().rev() {
            push(&mut stack, list, &path, depth + 1);
        }
    }
    Ok(())
}

/// The first lexicon `cid` that isn't ASCII. CIDs are ASCII, but the base58
/// decoder inside @atproto/lexicon accepts characters above U+00FF, which
/// `is_cid` rightly doesn't; checking this first, the same way on both sides,
/// keeps the validators agreeing. It looks only where the lexicon declares a
/// CID (a record source's `record.cid`, a module's `wasm.cid` and
/// `script.cid`), through blocks this version knows, in document order:
/// sources, then middleware, then blocks. As `checkCids` in
/// web/src/blocks/validate.ts. (Runs after the depth check, so recursion is
/// bounded.)
pub(crate) fn check_cids(card: &Value) -> Result {
    fn bad(value: &Value, at: String) -> Result {
        match value.as_str() {
            Some(s) if !s.is_ascii() => Err(CardError {
                message: format!("{at} must be a cid string (ASCII)"),
                path: at,
                reason: "format".into(),
            }),
            _ => Ok(()),
        }
    }
    fn module(r: &Value, at: &str) -> Result {
        bad(&r["wasm"]["cid"], format!("{at}/wasm/cid"))?;
        bad(&r["script"]["cid"], format!("{at}/script/cid"))
    }
    fn visit(blocks: &Value, path: &str) -> Result {
        for (i, block) in blocks.as_array().into_iter().flatten().enumerate() {
            let at = format!("{path}/{i}");
            let Some(kind) = block["$type"].as_str().and_then(|t| t.strip_prefix(DEFS_PREFIX))
            else {
                continue;
            };
            match kind {
                "custom" | "canvas" => module(&block["module"], &format!("{at}/module"))?,
                "section" | "stack" => visit(&block["blocks"], &format!("{at}/blocks"))?,
                "columns" => {
                    for (j, column) in block["columns"].as_array().into_iter().flatten().enumerate()
                    {
                        visit(&column["blocks"], &format!("{at}/columns/{j}/blocks"))?;
                    }
                }
                "list" => visit(&block["template"], &format!("{at}/template"))?,
                "button" => visit(&block["opens"]["blocks"], &format!("{at}/opens/blocks"))?,
                _ => {}
            }
        }
        Ok(())
    }
    for (i, source) in card["sources"].as_array().into_iter().flatten().enumerate() {
        let r = &source["ref"];
        if r["$type"].as_str() == Some("app.gather.block.defs#recordSource") {
            bad(&r["record"]["cid"], format!("/sources/{i}/ref/record/cid"))?;
        }
    }
    for (i, r) in card["middleware"].as_array().into_iter().flatten().enumerate() {
        module(r, &format!("/middleware/{i}"))?;
    }
    visit(&card["blocks"], "/blocks")
}

/// Rules the lexicon can't express, checked after it passes (the same rules,
/// in the same order, as `checkCard` in web/src/blocks/validate.ts):
/// - block ids are unique across the whole card, sheets and list templates
///   included, so an intent's blockId (plus `item` in a list) names one block
/// - a select's option values are unique, and so are a button group's
/// - source names are unique, and `$item` (a list's element) is reserved
/// - a button has exactly one of `action` and `opens`
/// - a binding names a declared source, or `$item` inside a list template
///   (or a sheet opened from one), and its `path` is a JSON pointer
/// - a list's `key` is a JSON pointer
///
/// Sources are checked first, then blocks, depth first in document order.
/// Within a block: its id, its options, its button rule, its bindings (in
/// `bind` order, then a list's `items`), a list's `key`, then the blocks
/// inside it. A binding's source is checked before its path.
pub(crate) fn check_card(card: &Value) -> Result {
    let mut names: HashSet<&str> = HashSet::new();
    for (i, source) in card["sources"].as_array().into_iter().flatten().enumerate() {
        let name = source["name"].as_str().unwrap_or_default();
        let at = format!("/sources/{i}/name");
        if name == "$item" {
            return Err(CardError {
                message: format!("{at} \"$item\" is reserved for list elements"),
                path: at,
                reason: "reserved".into(),
            });
        }
        if names.contains(&name) {
            return Err(CardError {
                message: format!("{at} \"{name}\" is already a source"),
                path: at,
                reason: "duplicate".into(),
            });
        }
        names.insert(name);
    }
    let declared = names;
    visit(&card["blocks"], "/blocks", &mut HashSet::new(), &declared, false)
}

/// An object's entries in the order JavaScript's `Object.entries` gives them:
/// array-index keys (`0` to `2^32 - 2`, no leading zeros) first, ascending,
/// then the other keys in the order they were written.
fn js_entries(map: &Map<String, Value>) -> Vec<(String, &Value)> {
    let index = |key: &str| {
        key.parse::<u64>().ok().filter(|n| *n < u64::from(u32::MAX) && n.to_string() == key)
    };
    let mut indexed: Vec<(u64, String, &Value)> =
        map.iter().filter_map(|(k, v)| index(k).map(|n| (n, k.clone(), v))).collect();
    indexed.sort_by_key(|(n, _, _)| *n);
    let named = map.iter().filter(|(k, _)| index(k).is_none()).map(|(k, v)| (k.clone(), v));
    indexed.into_iter().map(|(_, k, v)| (k, v)).chain(named).collect()
}

/// A JSON pointer (RFC 6901): empty, or "/" segments where `~` only appears
/// as `~0` or `~1`.
fn check_pointer(value: &Value, at: &str) -> Result {
    match value.as_str() {
        Some(s) if !(s.is_empty() || (s.starts_with('/') && tildes_escaped(s))) => Err(CardError {
            path: at.to_owned(),
            reason: "format".into(),
            message: format!(
                "{at} \"{s}\" must be a JSON pointer (empty, or starting with \"/\", with ~ only as ~0 or ~1)"
            ),
        }),
        _ => Ok(()),
    }
}

/// Every `~` is followed by `0` or `1`.
fn tildes_escaped(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes
        .iter()
        .enumerate()
        .all(|(i, b)| *b != b'~' || matches!(bytes.get(i + 1), Some(b'0' | b'1')))
}

/// A key as one JSON pointer segment.
fn segment(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// A binding must name a declared source, or `$item` inside a list template.
fn check_binding(binding: &Value, at: &str, declared: &HashSet<&str>, in_list: bool) -> Result {
    let Some(source) = binding.get("source").and_then(Value::as_str) else {
        return Ok(());
    };
    let known = if source == "$item" { in_list } else { declared.contains(&source) };
    if known {
        return check_pointer(&binding["path"], &format!("{at}/path"));
    }
    let why = if source == "$item" {
        "is only defined inside a list template"
    } else {
        "is not a source of this card"
    };
    Err(CardError {
        path: format!("{at}/source"),
        reason: "unknown-source".into(),
        message: format!("{at}/source \"{source}\" {why}"),
    })
}

fn visit(
    blocks: &Value,
    path: &str,
    ids: &mut HashSet<String>,
    declared: &HashSet<&str>,
    in_list: bool,
) -> Result {
    let duplicate =
        |at: String, message: String| CardError { path: at, reason: "duplicate".into(), message };
    for (i, block) in blocks.as_array().into_iter().flatten().enumerate() {
        let at = format!("{path}/{i}");
        let Some(kind) = block["$type"].as_str().and_then(|t| t.strip_prefix(DEFS_PREFIX)) else {
            continue;
        };
        if let Some(id) = block["id"].as_str() {
            if ids.contains(id) {
                return Err(duplicate(
                    format!("{at}/id"),
                    format!("{at}/id \"{id}\" is already used on this card"),
                ));
            }
            ids.insert(id.to_owned());
        }
        let choices = match kind {
            "select" => Some("options"),
            "buttonGroup" => Some("buttons"),
            _ => None,
        };
        if let Some(choices) = choices {
            let mut values: HashSet<&str> = HashSet::new();
            for (j, option) in block[choices].as_array().into_iter().flatten().enumerate() {
                let value = option["value"].as_str().unwrap_or_default();
                if values.contains(&value) {
                    return Err(duplicate(
                        format!("{at}/{choices}/{j}/value"),
                        format!("{at}/{choices}/{j}/value \"{value}\" is already a choice"),
                    ));
                }
                values.insert(value);
            }
        }
        if kind == "button" {
            let (has_action, has_sheet) =
                (block.get("action").is_some(), block.get("opens").is_some());
            if !has_action && !has_sheet {
                return Err(CardError {
                    path: format!("{at}/action"),
                    reason: "required".into(),
                    message: format!("{at} needs an action or a sheet to open"),
                });
            }
            if has_action && has_sheet {
                return Err(CardError {
                    path: format!("{at}/opens"),
                    reason: "exclusive".into(),
                    message: format!("{at} has both an action and a sheet"),
                });
            }
        }
        // As JavaScript's Object.entries reads it: an object's keys, or an
        // array's indices (a def without `bind` leaves it unchecked by the lexicon).
        let bind: Vec<(String, &Value)> = match &block["bind"] {
            Value::Object(map) => js_entries(map),
            Value::Array(items) => {
                items.iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect()
            }
            _ => Vec::new(),
        };
        for (prop, binding) in bind {
            check_binding(binding, &format!("{at}/bind/{}", segment(&prop)), declared, in_list)?;
        }
        if kind == "list" {
            check_binding(&block["items"], &format!("{at}/items"), declared, in_list)?;
            check_pointer(&block["key"], &format!("{at}/key"))?;
        }
        match kind {
            "section" | "stack" => {
                visit(&block["blocks"], &format!("{at}/blocks"), ids, declared, in_list)?
            }
            "columns" => {
                for (j, column) in block["columns"].as_array().into_iter().flatten().enumerate() {
                    visit(
                        &column["blocks"],
                        &format!("{at}/columns/{j}/blocks"),
                        ids,
                        declared,
                        in_list,
                    )?;
                }
            }
            "list" => visit(&block["template"], &format!("{at}/template"), ids, declared, true)?,
            "button" if !block["opens"].is_null() => visit(
                &block["opens"]["blocks"],
                &format!("{at}/opens/blocks"),
                ids,
                declared,
                in_list,
            )?,
            _ => {}
        }
    }
    Ok(())
}
