//! Runs card modules on the server: the same wasm the browser runs, with
//! `env: "server"`. See DEMO.md for the ABI.
//!
//! Each call gets a fresh instance with a fuel and memory budget, so a
//! module can't loop forever or eat the server's memory, and keeps nothing
//! between calls but what it returns.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;
use sha2::{Digest, Sha256};
use wasmi::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

use crate::db::{Db, now_ms};

/// Instructions a call may run.
const FUEL: u64 = 200_000_000;
/// Linear memory a call may use.
const MEMORY: usize = 64 << 20;

const STD: &[u8] = include_bytes!("../../demo/std.wasm");
const DICE: &[u8] = include_bytes!("../../demo/dice.wasm");
const QUIZ: &[u8] = include_bytes!("../../demo/quiz.wasm");
const EVENT: &[u8] = include_bytes!("../../demo/event.wasm");

/// The modules that ship with the server, by name.
pub fn builtin(name: &str) -> Option<&'static [u8]> {
    match name {
        "std" => Some(STD),
        "dice" => Some(DICE),
        "quiz" => Some(QUIZ),
        "event" => Some(EVENT),
        _ => None,
    }
}

pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The hash of a built-in module.
pub fn builtin_hash(name: &str) -> String {
    static HASHES: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    let hashes = HASHES.get_or_init(|| {
        ["std", "dice", "quiz", "event"]
            .into_iter()
            .map(|n| (n, hash(builtin(n).expect("a built-in module"))))
            .collect()
    });
    hashes.get(name).cloned().unwrap_or_default()
}

/// Stores a module, returning its hash.
pub async fn store(db: &Db, bytes: &[u8]) -> Result<String, sqlx::Error> {
    let hash = hash(bytes);
    sqlx::query(
        "INSERT INTO modules (hash, bytes, created_at) VALUES ($1, $2, $3) ON CONFLICT (hash) DO NOTHING",
    )
    .bind(&hash)
    .bind(STANDARD.encode(bytes))
    .bind(now_ms())
    .execute(db)
    .await?;
    Ok(hash)
}

/// A stored module's bytes.
pub async fn bytes(db: &Db, hash: &str) -> Result<Option<Vec<u8>>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as("SELECT bytes FROM modules WHERE hash = $1")
        .bind(hash)
        .fetch_optional(db)
        .await?;
    Ok(row.and_then(|(b64,)| STANDARD.decode(b64).ok()))
}

/// Checks that bytes are a module with the card ABI's exports.
pub fn check(runtime: &Runtime, bytes: &[u8]) -> Result<(), String> {
    let module =
        Module::new(&runtime.engine, bytes).map_err(|e| format!("not a valid wasm module: {e}"))?;
    if module.imports().len() > 0 {
        return Err("a card module can't import anything".into());
    }
    for name in ["memory", "alloc", "reduce", "view"] {
        if module.get_export(name).is_none() {
            return Err(format!("a card module must export `{name}`"));
        }
    }
    Ok(())
}

pub struct Runtime {
    engine: Engine,
    compiled: Mutex<HashMap<String, Module>>,
}

impl Default for Runtime {
    fn default() -> Self {
        let mut config = Config::default();
        config.consume_fuel(true);
        Self { engine: Engine::new(&config), compiled: Mutex::default() }
    }
}

impl Runtime {
    /// Calls `export` (`reduce` or `view`) of the module with this hash,
    /// compiling it from `bytes` the first time (see [`Runtime::has`]).
    /// Blocking: run it off the async threads.
    pub fn call(
        &self,
        hash: &str,
        bytes: Option<&[u8]>,
        export: &str,
        input: &Value,
    ) -> Result<Value, String> {
        let module = {
            let mut compiled = self.compiled.lock().expect("the module cache");
            match compiled.get(hash) {
                Some(m) => m.clone(),
                None => {
                    let bytes = bytes.ok_or("the module isn't stored")?;
                    let m = Module::new(&self.engine, bytes).map_err(|e| e.to_string())?;
                    compiled.insert(hash.to_owned(), m.clone());
                    m
                }
            }
        };
        self.run(&module, export, input)
    }

    /// Whether the module with this hash is compiled already.
    pub fn has(&self, hash: &str) -> bool {
        self.compiled.lock().expect("the module cache").contains_key(hash)
    }

    fn run(&self, module: &Module, export: &str, input: &Value) -> Result<Value, String> {
        let limits: StoreLimits = StoreLimitsBuilder::new().memory_size(MEMORY).build();
        let mut store = Store::new(&self.engine, limits);
        store.limiter(|l| l);
        store.set_fuel(FUEL).map_err(|e| e.to_string())?;
        let instance = Linker::<StoreLimits>::new(&self.engine)
            .instantiate_and_start(&mut store, module)
            .map_err(|e| e.to_string())?;
        let memory = instance.get_memory(&store, "memory").ok_or("no memory export")?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&store, "alloc")
            .map_err(|e| format!("alloc: {e}"))?;
        let entry = instance
            .get_typed_func::<(i32, i32), i64>(&store, export)
            .map_err(|e| format!("{export}: {e}"))?;
        let bytes = serde_json::to_vec(input).map_err(|e| e.to_string())?;
        let len = i32::try_from(bytes.len()).map_err(|_| "input too large")?;
        let ptr = alloc.call(&mut store, len).map_err(|e| format!("alloc: {e}"))?;
        memory.write(&mut store, ptr as u32 as usize, &bytes).map_err(|e| e.to_string())?;
        let packed = entry.call(&mut store, (ptr, len)).map_err(|e| format!("{export}: {e}"))?;
        let (out_ptr, out_len) = ((packed >> 32) as u32 as usize, (packed & 0xffff_ffff) as usize);
        let mut out = vec![0u8; out_len];
        memory.read(&store, out_ptr, &mut out).map_err(|e| e.to_string())?;
        serde_json::from_slice(&out).map_err(|e| format!("{export} returned invalid JSON: {e}"))
    }
}
