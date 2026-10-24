//! Bounded cache with explicit invalidation and stale detection (Issue #292).
//!
//! Provides a ledger-aware, capacity-bounded key-value cache stored in Soroban
//! temporary storage. Entries are stamped with the ledger sequence at write
//! time, enabling deterministic staleness detection without wall-clock access.
//!
//! ## Design
//!
//! - **Bounded**: a configurable maximum capacity. When the cache is full, the
//!   oldest entry (by ledger sequence) is evicted before inserting a new one.
//! - **Stale detection**: each entry carries the ledger sequence at which it
//!   was written. A configurable `max_age_ledgers` threshold marks entries as
//!   stale. Stale entries are never returned by [`get`].
//! - **Explicit invalidation**: [`invalidate`] removes a single key;
//!   [`invalidate_all`] clears the entire cache.
//! - **No_std compatible**: uses only Soroban SDK types.
//!
//! Time complexity: O(n) for eviction scan where n = current cache size
//! (bounded by `max_capacity`, typically ≤ 64).
//! Space complexity: O(n) entries in temporary storage + one metadata entry.

use soroban_sdk::{contracttype, symbol_short, Env, Symbol, Vec};

use crate::storage::{temporary_get, temporary_remove, temporary_set};

pub const DEFAULT_MAX_CAPACITY: u32 = 64;
pub const DEFAULT_MAX_AGE_LEDGERS: u32 = 1_000;
pub const ABSOLUTE_MAX_CAPACITY: u32 = 256;

pub const TOPIC_CACHE_EVICT: Symbol = symbol_short!("cch_ev");

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheConfig {
    pub max_capacity: u32,
    pub max_age_ledgers: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_capacity: DEFAULT_MAX_CAPACITY,
            max_age_ledgers: DEFAULT_MAX_AGE_LEDGERS,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheEntry {
    pub key: Symbol,
    pub value: i128,
    pub written_at_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum CacheStorageKey {
    Meta(Symbol),
    Entry(Symbol, Symbol),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheMeta {
    pub config: CacheConfig,
    pub keys: Vec<Symbol>,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CacheError {
    InvalidConfig = 1,
    KeyNotFound = 2,
    StaleEntry = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheStats {
    pub size: u32,
    pub capacity: u32,
    pub stale_count: u32,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn meta_key(namespace: &Symbol) -> CacheStorageKey {
    CacheStorageKey::Meta(namespace.clone())
}

fn entry_key(namespace: &Symbol, key: &Symbol) -> CacheStorageKey {
    CacheStorageKey::Entry(namespace.clone(), key.clone())
}

fn load_meta(env: &Env, namespace: &Symbol) -> CacheMeta {
    temporary_get(env, &meta_key(namespace)).unwrap_or_else(|| CacheMeta {
        config: CacheConfig::default(),
        keys: Vec::new(env),
    })
}

fn save_meta(env: &Env, namespace: &Symbol, meta: &CacheMeta) {
    temporary_set(env, &meta_key(namespace), meta);
}

fn is_stale(entry: &CacheEntry, current_ledger: u32, max_age: u32) -> bool {
    current_ledger.saturating_sub(entry.written_at_ledger) > max_age
}

fn find_oldest_index(env: &Env, namespace: &Symbol, keys: &Vec<Symbol>) -> Option<u32> {
    let mut oldest_ledger = u32::MAX;
    let mut oldest_idx = None;

    for i in 0..keys.len() {
        let k = keys.get_unchecked(i);
        if let Some(entry) = temporary_get::<_, CacheEntry>(env, &entry_key(namespace, &k)) {
            if entry.written_at_ledger < oldest_ledger {
                oldest_ledger = entry.written_at_ledger;
                oldest_idx = Some(i);
            }
        } else {
            return Some(i);
        }
    }
    oldest_idx
}

fn remove_key_at(keys: &Vec<Symbol>, idx: u32, env: &Env) -> Vec<Symbol> {
    let mut new_keys = Vec::new(env);
    for i in 0..keys.len() {
        if i != idx {
            new_keys.push_back(keys.get_unchecked(i));
        }
    }
    new_keys
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn validate_config(config: &CacheConfig) -> Result<(), CacheError> {
    if config.max_capacity == 0 || config.max_capacity > ABSOLUTE_MAX_CAPACITY {
        return Err(CacheError::InvalidConfig);
    }
    if config.max_age_ledgers == 0 {
        return Err(CacheError::InvalidConfig);
    }
    Ok(())
}

pub fn initialize(env: &Env, namespace: &Symbol, config: &CacheConfig) -> Result<(), CacheError> {
    validate_config(config)?;
    let meta = CacheMeta {
        config: config.clone(),
        keys: Vec::new(env),
    };
    save_meta(env, namespace, &meta);
    Ok(())
}

pub fn put(
    env: &Env,
    namespace: &Symbol,
    key: &Symbol,
    value: i128,
) -> Result<(), CacheError> {
    let mut meta = load_meta(env, namespace);
    validate_config(&meta.config)?;

    let ekey = entry_key(namespace, key);
    let current_ledger = env.ledger().sequence();

    let mut key_exists = false;
    for i in 0..meta.keys.len() {
        if meta.keys.get_unchecked(i) == *key {
            key_exists = true;
            break;
        }
    }

    if !key_exists && meta.keys.len() >= meta.config.max_capacity {
        if let Some(oldest_idx) = find_oldest_index(env, namespace, &meta.keys) {
            let evicted_key = meta.keys.get_unchecked(oldest_idx);
            temporary_remove(env, &entry_key(namespace, &evicted_key));
            meta.keys = remove_key_at(&meta.keys, oldest_idx, env);
        }
    }

    if !key_exists {
        meta.keys.push_back(key.clone());
    }

    let entry = CacheEntry {
        key: key.clone(),
        value,
        written_at_ledger: current_ledger,
    };
    temporary_set(env, &ekey, &entry);
    save_meta(env, namespace, &meta);
    Ok(())
}

pub fn get(
    env: &Env,
    namespace: &Symbol,
    key: &Symbol,
) -> Result<CacheEntry, CacheError> {
    let meta = load_meta(env, namespace);
    let ekey = entry_key(namespace, key);

    let entry: CacheEntry = temporary_get(env, &ekey).ok_or(CacheError::KeyNotFound)?;
    let current_ledger = env.ledger().sequence();

    if is_stale(&entry, current_ledger, meta.config.max_age_ledgers) {
        return Err(CacheError::StaleEntry);
    }
    Ok(entry)
}

pub fn invalidate(env: &Env, namespace: &Symbol, key: &Symbol) -> Result<(), CacheError> {
    let mut meta = load_meta(env, namespace);

    temporary_remove(env, &entry_key(namespace, key));

    let mut new_keys = Vec::new(env);
    for i in 0..meta.keys.len() {
        let k = meta.keys.get_unchecked(i);
        if k != *key {
            new_keys.push_back(k);
        }
    }
    meta.keys = new_keys;
    save_meta(env, namespace, &meta);
    Ok(())
}

pub fn invalidate_all(env: &Env, namespace: &Symbol) -> Result<u32, CacheError> {
    let meta = load_meta(env, namespace);
    let count = meta.keys.len();

    for i in 0..meta.keys.len() {
        let k = meta.keys.get_unchecked(i);
        temporary_remove(env, &entry_key(namespace, &k));
    }

    let cleared_meta = CacheMeta {
        config: meta.config,
        keys: Vec::new(env),
    };
    save_meta(env, namespace, &cleared_meta);
    Ok(count)
}

pub fn stats(env: &Env, namespace: &Symbol) -> CacheStats {
    let meta = load_meta(env, namespace);
    let current_ledger = env.ledger().sequence();
    let mut stale_count = 0u32;

    for i in 0..meta.keys.len() {
        let k = meta.keys.get_unchecked(i);
        if let Some(entry) = temporary_get::<_, CacheEntry>(env, &entry_key(namespace, &k)) {
            if is_stale(&entry, current_ledger, meta.config.max_age_ledgers) {
                stale_count += 1;
            }
        }
    }

    CacheStats {
        size: meta.keys.len(),
        capacity: meta.config.max_capacity,
        stale_count,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Ledger as _, Env};

    fn ns() -> Symbol {
        symbol_short!("test_ns")
    }

    fn default_init(env: &Env) {
        initialize(env, &ns(), &CacheConfig::default()).unwrap();
    }

    #[test]
    fn normal_path_put_and_get() {
        let env = Env::default();
        default_init(&env);

        let key = symbol_short!("price");
        put(&env, &ns(), &key, 42_000).unwrap();

        let entry = get(&env, &ns(), &key).unwrap();
        assert_eq!(entry.value, 42_000);
        assert_eq!(entry.key, key);
    }

    #[test]
    fn normal_path_overwrite_updates_value_and_ledger() {
        let env = Env::default();
        default_init(&env);

        let key = symbol_short!("rate");
        put(&env, &ns(), &key, 100).unwrap();
        env.ledger().set_sequence_number(50);
        put(&env, &ns(), &key, 200).unwrap();

        let entry = get(&env, &ns(), &key).unwrap();
        assert_eq!(entry.value, 200);
        assert_eq!(entry.written_at_ledger, 50);
    }

    #[test]
    fn stale_detection_rejects_old_entries() {
        let env = Env::default();
        let config = CacheConfig {
            max_capacity: 10,
            max_age_ledgers: 100,
        };
        initialize(&env, &ns(), &config).unwrap();

        let key = symbol_short!("stale");
        put(&env, &ns(), &key, 999).unwrap();

        env.ledger().set_sequence_number(101);
        let err = get(&env, &ns(), &key).unwrap_err();
        assert_eq!(err, CacheError::StaleEntry);
    }

    #[test]
    fn stale_detection_allows_fresh_entries() {
        let env = Env::default();
        let config = CacheConfig {
            max_capacity: 10,
            max_age_ledgers: 100,
        };
        initialize(&env, &ns(), &config).unwrap();

        let key = symbol_short!("fresh");
        put(&env, &ns(), &key, 42).unwrap();

        env.ledger().set_sequence_number(100);
        let entry = get(&env, &ns(), &key).unwrap();
        assert_eq!(entry.value, 42);
    }

    #[test]
    fn eviction_removes_oldest_when_full() {
        let env = Env::default();
        let config = CacheConfig {
            max_capacity: 2,
            max_age_ledgers: 10_000,
        };
        initialize(&env, &ns(), &config).unwrap();

        let k1 = symbol_short!("k1");
        let k2 = symbol_short!("k2");
        let k3 = symbol_short!("k3");

        put(&env, &ns(), &k1, 1).unwrap();
        env.ledger().set_sequence_number(1);
        put(&env, &ns(), &k2, 2).unwrap();

        env.ledger().set_sequence_number(2);
        put(&env, &ns(), &k3, 3).unwrap();

        assert_eq!(get(&env, &ns(), &k3).unwrap().value, 3);
        assert_eq!(get(&env, &ns(), &k2).unwrap().value, 2);
        assert_eq!(get(&env, &ns(), &k1), Err(CacheError::KeyNotFound));
    }

    #[test]
    fn invalidate_removes_single_key() {
        let env = Env::default();
        default_init(&env);

        let key = symbol_short!("rmme");
        put(&env, &ns(), &key, 100).unwrap();
        invalidate(&env, &ns(), &key).unwrap();

        assert_eq!(get(&env, &ns(), &key), Err(CacheError::KeyNotFound));
        assert_eq!(stats(&env, &ns()).size, 0);
    }

    #[test]
    fn invalidate_all_clears_entire_cache() {
        let env = Env::default();
        default_init(&env);

        put(&env, &ns(), &symbol_short!("a"), 1).unwrap();
        put(&env, &ns(), &symbol_short!("b"), 2).unwrap();
        put(&env, &ns(), &symbol_short!("c"), 3).unwrap();

        let removed = invalidate_all(&env, &ns()).unwrap();
        assert_eq!(removed, 3);
        assert_eq!(stats(&env, &ns()).size, 0);
    }

    #[test]
    fn stats_reports_stale_count() {
        let env = Env::default();
        let config = CacheConfig {
            max_capacity: 10,
            max_age_ledgers: 50,
        };
        initialize(&env, &ns(), &config).unwrap();

        put(&env, &ns(), &symbol_short!("old"), 1).unwrap();
        env.ledger().set_sequence_number(100);
        put(&env, &ns(), &symbol_short!("new"), 2).unwrap();

        let s = stats(&env, &ns());
        assert_eq!(s.size, 2);
        assert_eq!(s.stale_count, 1);
    }

    #[test]
    fn failure_path_invalid_config_rejected() {
        let env = Env::default();

        let zero_cap = CacheConfig {
            max_capacity: 0,
            max_age_ledgers: 100,
        };
        assert_eq!(initialize(&env, &ns(), &zero_cap), Err(CacheError::InvalidConfig));

        let over_cap = CacheConfig {
            max_capacity: ABSOLUTE_MAX_CAPACITY + 1,
            max_age_ledgers: 100,
        };
        assert_eq!(initialize(&env, &ns(), &over_cap), Err(CacheError::InvalidConfig));

        let zero_age = CacheConfig {
            max_capacity: 10,
            max_age_ledgers: 0,
        };
        assert_eq!(initialize(&env, &ns(), &zero_age), Err(CacheError::InvalidConfig));
    }

    #[test]
    fn get_missing_key_returns_not_found() {
        let env = Env::default();
        default_init(&env);

        assert_eq!(
            get(&env, &ns(), &symbol_short!("nope")),
            Err(CacheError::KeyNotFound)
        );
    }

    #[test]
    fn retry_path_put_after_invalidate_succeeds() {
        let env = Env::default();
        default_init(&env);

        let key = symbol_short!("retry");
        put(&env, &ns(), &key, 1).unwrap();
        invalidate(&env, &ns(), &key).unwrap();
        put(&env, &ns(), &key, 2).unwrap();

        assert_eq!(get(&env, &ns(), &key).unwrap().value, 2);
    }
}
