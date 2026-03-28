//! Deterministic structural hashing for JSON values.
//!
//! ```
//! use genome_rs::{Genome, GenomeConfig};
//! use serde_json::json;
//!
//! let mut g = Genome::new(GenomeConfig::default());
//! let id  = g.hash(&json!({ "id": 1, "name": "alice" }));
//! let id2 = g.hash(&json!({ "id": 2, "name": "bob" }));
//! assert_eq!(id, id2); // same structure, different values
//! ```

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde_json::Value;
use std::collections::{HashMap, HashSet};

// ────────────────────────────────────────────────────────────────────────────
// xxHash32
// Faithful port of the TS implementation. The JS version splits every u32
// multiply into two u16 halves to avoid float precision loss — Rust's u32
// wrapping_mul handles this natively so the split is gone entirely.
// ────────────────────────────────────────────────────────────────────────────

const PRIME32_1: u32 = 2_654_435_761;
const PRIME32_2: u32 = 2_246_822_519;
const PRIME32_3: u32 = 3_266_489_917;
const PRIME32_4: u32 = 668_265_263;
const PRIME32_5: u32 = 374_761_393;

#[inline(always)]
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

#[inline(always)]
fn xx_round(acc: u32, lane: u32) -> u32 {
    acc.wrapping_add(lane.wrapping_mul(PRIME32_2))
        .rotate_left(13)
        .wrapping_mul(PRIME32_1)
}

/// Raw xxHash32 over a byte slice with an optional seed.
pub fn xx_hash32(input: &[u8], seed: u32) -> u32 {
    let len = input.len();
    let mut rest = input;
    let mut acc: u32;

    if len >= 16 {
        let mut v1 = seed.wrapping_add(PRIME32_1).wrapping_add(PRIME32_2);
        let mut v2 = seed.wrapping_add(PRIME32_2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(PRIME32_1);

        while rest.len() >= 16 {
            v1 = xx_round(v1, read_u32_le(rest, 0));
            v2 = xx_round(v2, read_u32_le(rest, 4));
            v3 = xx_round(v3, read_u32_le(rest, 8));
            v4 = xx_round(v4, read_u32_le(rest, 12));
            rest = &rest[16..];
        }

        acc = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
    } else {
        acc = seed.wrapping_add(PRIME32_5);
    }

    acc = acc.wrapping_add(len as u32);

    while rest.len() >= 4 {
        let lane = read_u32_le(rest, 0);
        acc = acc
            .wrapping_add(lane.wrapping_mul(PRIME32_3))
            .rotate_left(17)
            .wrapping_mul(PRIME32_4);
        rest = &rest[4..];
    }

    for &byte in rest {
        acc = acc
            .wrapping_add((byte as u32).wrapping_mul(PRIME32_5))
            .rotate_left(11)
            .wrapping_mul(PRIME32_1);
    }

    // avalanche
    acc ^= acc >> 15;
    acc = acc.wrapping_mul(PRIME32_2);
    acc ^= acc >> 13;
    acc = acc.wrapping_mul(PRIME32_3);
    acc ^= acc >> 16;

    acc
}

/// Hashes a string with xxHash32 and returns a hex string.
pub fn hash_str(input: &str, seed: u32) -> String {
    format!("{:x}", xx_hash32(input.as_bytes(), seed))
}

// ────────────────────────────────────────────────────────────────────────────
// Type bits — mirrors TYPE_BITS in the TS implementation
// ────────────────────────────────────────────────────────────────────────────

fn type_bit(value: &Value) -> u64 {
    match value {
        Value::Null => 16,
        Value::Bool(_) => 4,
        Value::Number(_) => 1,
        Value::String(_) => 2,
        Value::Array(_) => 256,
        Value::Object(_) => 128,
    }
}

/// When ignoring value types, all scalars collapse to the string sentinel (2)
/// so that structural shape is compared without caring about primitive types.
fn effective_type_bit(value: &Value, ignore_value_types: bool) -> u64 {
    if ignore_value_types {
        match value {
            Value::Object(_) | Value::Array(_) => type_bit(value),
            _ => 2,
        }
    } else {
        type_bit(value)
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Config
// ────────────────────────────────────────────────────────────────────────────

/// Configuration for the Genome hasher.
#[derive(Debug, Clone, Default)]
pub struct GenomeConfig {
    /// When true, L0 is replaced with an incrementing collision counter so
    /// that structurally identical values receive distinct IDs.
    pub new_id_on_collision: bool,
    /// When true, arrays with different lengths but the same element shapes
    /// produce the same ID.
    pub ignore_array_length: bool,
    /// When true, all scalar types are treated as equivalent — only key names
    /// and nesting depth affect the ID.
    pub ignore_value_types: bool,
}

// ────────────────────────────────────────────────────────────────────────────
// Similarity
// ────────────────────────────────────────────────────────────────────────────

/// Result of comparing two structure IDs.
#[derive(Debug, Clone)]
pub struct Similarity {
    /// 0.0 (completely different) to 1.0 (identical)
    pub score: f64,
    /// Number of levels that matched exactly
    pub matched_levels: usize,
    /// Max levels across both IDs
    pub total_levels: usize,
    /// Per-level match score (1.0 = exact, 0.0 = missing/completely different)
    pub level_scores: Vec<f64>,
}

// ────────────────────────────────────────────────────────────────────────────
// Genome
// ────────────────────────────────────────────────────────────────────────────

/// The main hasher. Holds internal key cache and collision counters.
///
/// # Example
/// ```
/// use genome_rs::{Genome, GenomeConfig};
/// use serde_json::json;
///
/// let mut g = Genome::new(GenomeConfig::default());
/// let id = g.hash(&json!({ "id": 1, "name": "alice" }));
/// ```
pub struct Genome {
    config: GenomeConfig,
    /// Cache of string keys to deterministic u64 bit values (mirrors GLOBAL_KEY_MAP)
    key_cache: HashMap<String, u64>,
    /// Collision counters keyed by L1+ structural signature.
    /// Only meaningful when new_id_on_collision is true.
    collision_counters: HashMap<String, u64>,
}

impl Genome {
    pub fn new(config: GenomeConfig) -> Self {
        Self {
            config,
            key_cache: HashMap::new(),
            collision_counters: HashMap::new(),
        }
    }

    /// Returns the cached or freshly computed bit for a key string.
    fn get_bit(&mut self, key: &str) -> u64 {
        if let Some(&v) = self.key_cache.get(key) {
            return v;
        }
        let h = xx_hash32(key.as_bytes(), 0) as u64;
        self.key_cache.insert(key.to_string(), h);
        h
    }

    /// Produces a shape fingerprint string for an object or array node,
    /// used for cycle detection.
    fn node_signature(value: &Value, path: &[&str]) -> String {
        let path_str = path.join(".");
        match value {
            Value::Object(map) => {
                let mut keys: Vec<&str> = map.keys().map(|s| s.as_str()).collect();
                keys.sort_unstable();
                format!("{}.{{{}}}", path_str, keys.join(","))
            }
            Value::Array(arr) => format!("{}.[{}]", path_str, arr.len()),
            _ => path_str,
        }
    }

    /// Core recursive structure processor. Mirrors processStructure() in TS.
    ///
    /// Uses wrapping_shl instead of << to avoid a panic at level >= 64.
    fn process<'a>(
        &mut self,
        value: &'a Value,
        level: usize,
        path: &mut Vec<&'a str>,
        levels: &mut HashMap<usize, u64>,
        seen: &mut HashMap<usize, String>,
    ) {
        let entry = levels
            .entry(level)
            .or_insert(1u64.wrapping_shl(level as u32));
        *entry = entry.wrapping_add(effective_type_bit(value, self.config.ignore_value_types));

        match value {
            Value::Object(map) => {
                let ptr = value as *const Value as usize;
                let sig = Self::node_signature(value, path);

                if let Some(circular) = seen.get(&ptr).cloned() {
                    let bit = self.get_bit(&format!("circular:{}", circular));
                    *levels.entry(level).or_insert(0) = levels[&level].wrapping_add(bit);
                    return;
                }
                seen.insert(ptr, sig);

                let obj_bit = self.get_bit("type:object");
                *levels.entry(level).or_insert(0) = levels[&level].wrapping_add(obj_bit);

                let mut keys: Vec<&str> = map.keys().map(|s| s.as_str()).collect();
                keys.sort_unstable();

                for (i, key) in keys.iter().enumerate() {
                    let multiplier = (i + 1) as u64;
                    let val = &map[*key];
                    let prop_bit = effective_type_bit(val, self.config.ignore_value_types);
                    let key_bit = self.get_bit(key);

                    let h = levels.entry(level).or_insert(0);
                    *h = h.wrapping_add(key_bit.wrapping_mul(multiplier));
                    *h = h.wrapping_add(prop_bit.wrapping_mul(multiplier));

                    path.push(key);
                    self.process(val, level + 1, path, levels, seen);
                    path.pop();
                }
            }

            Value::Array(arr) => {
                let ptr = value as *const Value as usize;
                let sig = Self::node_signature(value, path);

                if let Some(circular) = seen.get(&ptr).cloned() {
                    let bit = self.get_bit(&format!("circular:{}", circular));
                    *levels.entry(level).or_insert(0) = levels[&level].wrapping_add(bit);
                    return;
                }
                seen.insert(ptr, sig);

                let arr_bit = self.get_bit("type:array");
                *levels.entry(level).or_insert(0) = levels[&level].wrapping_add(arr_bit);

                if !self.config.ignore_array_length {
                    let len_bit = self.get_bit(&format!("length:{}", arr.len()));
                    *levels.entry(level).or_insert(0) = levels[&level].wrapping_add(len_bit);

                    for (i, item) in arr.iter().enumerate() {
                        let multiplier = (i + 1) as u64;
                        let item_bit = effective_type_bit(item, self.config.ignore_value_types);
                        let index_bit = self.get_bit(&format!("[{}]", i));

                        let h = levels.entry(level).or_insert(0);
                        *h = h.wrapping_add(index_bit.wrapping_mul(multiplier));
                        *h = h.wrapping_add(item_bit.wrapping_mul(multiplier));

                        path.push("[*]");
                        self.process(item, level + 1, path, levels, seen);
                        path.pop();
                    }
                } else {
                    // Shape-only: hash unique element shapes, ignore cardinality.
                    let mut seen_shapes: HashSet<String> = HashSet::new();

                    for item in arr.iter() {
                        let shape_key = match item {
                            Value::Object(_) | Value::Array(_) => {
                                let mut p = path.clone();
                                p.push("[*]");
                                Self::node_signature(item, &p)
                            }
                            _ => format!("scalar:{}", value_type_name(item)),
                        };

                        if seen_shapes.insert(shape_key.clone()) {
                            let bit = self.get_bit(&shape_key);
                            *levels.entry(level).or_insert(0) = levels[&level].wrapping_add(bit);

                            path.push("[*]");
                            self.process(item, level + 1, path, levels, seen);
                            path.pop();
                        }
                    }

                    // Sentinel so variadic arrays differ from object containers
                    let sentinel = self.get_bit("array:variadic");
                    *levels.entry(level).or_insert(0) = levels[&level].wrapping_add(sentinel);
                }
            }

            // Primitives — type bit already added at the top
            _ => {}
        }
    }

    /// Generates a deterministic hierarchical structure ID for any JSON value.
    ///
    /// Format: `"L0:hash-L1:hash-L2:hash..."`
    ///
    /// When `new_id_on_collision` is false (default): L0 contains real
    /// structural data. The full ID is the structural fingerprint.
    ///
    /// When `new_id_on_collision` is true: L0 is replaced with an incrementing
    /// counter so structurally identical values receive distinct IDs.
    pub fn hash(&mut self, value: &Value) -> String {
        match value {
            Value::Object(map) if map.is_empty() => return "{}".to_string(),
            Value::Array(arr) if arr.is_empty() => return "[]".to_string(),
            v if !v.is_object() && !v.is_array() => {
                let bit = effective_type_bit(v, self.config.ignore_value_types);
                return format!("L0:{bit}-L1:{bit}");
            }
            _ => {}
        }

        let mut level_hashes: HashMap<usize, u64> = HashMap::new();
        let mut seen: HashMap<usize, String> = HashMap::new();
        let mut path: Vec<&str> = Vec::new();

        self.process(value, 0, &mut path, &mut level_hashes, &mut seen);

        let mut all_levels: Vec<(usize, u64)> =
            level_hashes.iter().map(|(&l, &h)| (l, h)).collect();
        all_levels.sort_unstable_by_key(|&(l, _)| l);

        if self.config.new_id_on_collision {
            let sig: String = all_levels
                .iter()
                .filter(|(l, _)| *l > 0)
                .map(|(l, h)| format!("L{l}:{h}"))
                .collect::<Vec<_>>()
                .join("-");

            let count = self.collision_counters.get(&sig).copied().unwrap_or(0);

            if let Some(l0) = all_levels.iter_mut().find(|(l, _)| *l == 0) {
                l0.1 = count;
            }

            self.collision_counters.insert(sig, count + 1);
        }

        all_levels
            .iter()
            .map(|(l, h)| format!("L{l}:{h}"))
            .collect::<Vec<_>>()
            .join("-")
    }

    /// Returns the structural signature for a value.
    ///
    /// When `new_id_on_collision` is false: the full ID is returned since L0
    /// is structural.
    ///
    /// When `new_id_on_collision` is true: L0 has been replaced with a counter
    /// so it is stripped and L1+ is returned — the stable shape fingerprint
    /// shared by structurally identical values.
    pub fn signature(&mut self, value: &Value) -> String {
        let id = self.hash(value);
        if self.config.new_id_on_collision {
            id.splitn(2, '-').nth(1).unwrap_or(&id).to_string()
        } else {
            id
        }
    }

    /// Compares two structure ID strings and returns a similarity score.
    /// Score ranges from 0.0 (completely different) to 1.0 (identical).
    pub fn compare(&self, id_a: &str, id_b: &str) -> Similarity {
        if id_a == id_b {
            let total = id_a.split('-').count();
            return Similarity {
                score: 1.0,
                matched_levels: total,
                total_levels: total,
                level_scores: vec![1.0; total],
            };
        }

        let parse = |id: &str| -> HashMap<usize, u64> {
            id.split('-')
                .filter_map(|part| {
                    let part = part.strip_prefix('L').unwrap_or(part);
                    let mut iter = part.splitn(2, ':');
                    let level: usize = iter.next()?.parse().ok()?;
                    let hash: u64 = iter.next()?.parse().ok()?;
                    Some((level, hash))
                })
                .collect()
        };

        let levels_a = parse(id_a);
        let levels_b = parse(id_b);

        let mut all_levels: Vec<usize> = levels_a
            .keys()
            .chain(levels_b.keys())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        all_levels.sort_unstable();

        let total_levels = all_levels.len();
        let weight_sum: f64 = all_levels.iter().map(|&l| 1.0 / (l + 1) as f64).sum();

        let mut score = 0.0_f64;
        let mut matched_levels = 0;
        let mut level_scores = Vec::with_capacity(total_levels);

        for &level in &all_levels {
            let weight = (1.0 / (level + 1) as f64) / weight_sum;

            match (levels_a.get(&level), levels_b.get(&level)) {
                (Some(&a), Some(&b)) if a == b => {
                    matched_levels += 1;
                    level_scores.push(1.0);
                    score += weight;
                }
                (Some(&a), Some(&b)) => {
                    let diff = a.abs_diff(b);
                    let max = a.max(b);
                    let proximity = if max == 0 {
                        1.0
                    } else {
                        (1.0 - (diff as f64 / max as f64)).max(0.0)
                    };
                    level_scores.push(proximity);
                    score += proximity * weight;
                }
                _ => {
                    level_scores.push(0.0);
                }
            }
        }

        Similarity {
            score,
            matched_levels,
            total_levels,
            level_scores,
        }
    }

    /// Convenience wrapper — compare two JSON values directly without
    /// needing to call `hash` first.
    pub fn compare_values(&mut self, a: &Value, b: &Value) -> Similarity {
        let id_a = self.hash(a);
        let id_b = self.hash(b);
        self.compare(&id_a, &id_b)
    }

    /// Seeds the collision counter for a known signature.
    /// Useful for restoring persisted state.
    pub fn seed(&mut self, signature: &str, count: u64) {
        self.collision_counters.insert(signature.to_string(), count);
    }

    /// Exports collision counter state for persistence.
    pub fn export_counters(&self) -> HashMap<String, u64> {
        self.collision_counters.clone()
    }

    /// Exports the key hash cache for persistence / cross-process consistency.
    pub fn export_cache(&self) -> HashMap<String, u64> {
        self.key_cache.clone()
    }

    /// Resets all internal state — equivalent to resetState() in TS.
    pub fn reset(&mut self) {
        self.key_cache.clear();
        self.collision_counters.clear();
    }
}

// ────────────────────────────────────────────────────────────────────────────
// WASM bindings
//
// Only compiled when the "wasm" feature is enabled:
//   wasm-pack build --target bundler --features wasm --release
//
// Mirrors the original TS library's functional API — no instantiation needed,
// just import and call. A single module-level Genome instance is held in a
// thread_local so state (key cache, collision counters) persists across calls
// exactly as it does with the TS module-level globals (GLOBAL_KEY_MAP,
// STRUCTURE_HASH_COUNTER etc).
//
// Config is applied via setConfig() rather than constructor arguments since
// there is no constructor — matching the TS setStructureIdConfig() pattern.
//
// WASM runs single-threaded in the browser so thread_local! is safe here.
// ────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "wasm")]
thread_local! {
    static GENOME: std::cell::RefCell<Genome> =
        std::cell::RefCell::new(Genome::new(GenomeConfig::default()));
}

/// Generates a deterministic hierarchical structure ID for a JSON string.
///
/// ```js
/// import { hash } from 'genome'
/// const id = hash(JSON.stringify({ id: 1, name: "alice" }))
/// ```
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "hash")]
pub fn wasm_hash(json: &str) -> Result<String, JsValue> {
    let value: Value = serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(GENOME.with(|g| g.borrow_mut().hash(&value)))
}

/// Returns the structural signature for a JSON string.
///
/// In default mode returns the full ID. When `newIdOnCollision` is true
/// (set via `setConfig`), strips L0 and returns L1+ only.
///
/// ```js
/// import { signature } from 'genome'
/// const sig = signature(JSON.stringify({ id: 1, name: "alice" }))
/// ```
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "signature")]
pub fn wasm_signature(json: &str) -> Result<String, JsValue> {
    let value: Value = serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(GENOME.with(|g| g.borrow_mut().signature(&value)))
}

/// Compares two structure ID strings and returns a similarity score
/// between 0.0 (completely different) and 1.0 (identical).
///
/// ```js
/// import { compare } from 'genome'
/// const score = compare("L0:100-L1:200", "L0:100-L1:200") // 1.0
/// ```
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "compare")]
pub fn wasm_compare(id_a: &str, id_b: &str) -> f64 {
    GENOME.with(|g| g.borrow().compare(id_a, id_b).score)
}

/// Compares two JSON strings structurally and returns a similarity score.
///
/// ```js
/// import { compareValues } from 'genome'
/// const score = compareValues(
///   JSON.stringify({ id: 1, name: "alice" }),
///   JSON.stringify({ id: 2, name: "bob" }),
/// ) // 1.0 — same structure
/// ```
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "compareValues")]
pub fn wasm_compare_values(a: &str, b: &str) -> Result<f64, JsValue> {
    let va: Value = serde_json::from_str(a).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let vb: Value = serde_json::from_str(b).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(GENOME.with(|g| g.borrow_mut().compare_values(&va, &vb).score))
}

/// Sets the global config. Call this before using any other functions
/// if you need non-default behaviour.
///
/// ```js
/// import { setConfig, hash } from 'genome'
///
/// setConfig({ ignoreArrayLength: true, ignoreValueTypes: true })
/// const id = hash(JSON.stringify({ items: [1, 2, 3] }))
/// ```
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "setConfig")]
pub fn wasm_set_config(
    new_id_on_collision: bool,
    ignore_array_length: bool,
    ignore_value_types: bool,
) {
    GENOME.with(|g| {
        *g.borrow_mut() = Genome::new(GenomeConfig {
            new_id_on_collision,
            ignore_array_length,
            ignore_value_types,
        });
    });
}

/// Seeds the collision counter for a known signature.
/// Use this to restore persisted counter state.
///
/// ```js
/// import { seed } from 'genome'
/// seed("L1:12345-L2:67890", 3n)
/// ```
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "seed")]
pub fn wasm_seed(signature: &str, count: u64) {
    GENOME.with(|g| g.borrow_mut().seed(signature, count));
}

/// Resets all internal state — clears the key cache and collision counters.
///
/// ```js
/// import { reset } from 'genome'
/// reset()
/// ```
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "reset")]
pub fn wasm_reset() {
    GENOME.with(|g| g.borrow_mut().reset());
}

/// Hashes a string with xxHash32 and returns a hex string.
///
/// ```js
/// import { hashStr } from 'genome'
/// const hex = hashStr("hello", 0)
/// ```
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "hashStr")]
pub fn wasm_hash_str(input: &str, seed: u32) -> String {
    hash_str(input, seed)
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn genome() -> Genome {
        Genome::new(GenomeConfig::default())
    }

    #[test]
    fn hash_str_empty_string() {
        let result = hash_str("", 0);
        assert!(!result.is_empty());
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_str_deterministic() {
        assert_eq!(hash_str("hello", 0), hash_str("hello", 0));
        assert_ne!(hash_str("hello", 0), hash_str("world", 0));
    }

    #[test]
    fn empty_object() {
        assert_eq!(genome().hash(&json!({})), "{}");
    }

    #[test]
    fn empty_array() {
        assert_eq!(genome().hash(&json!([])), "[]");
    }

    #[test]
    fn same_shape_different_values_same_id() {
        let mut g = genome();
        let a = json!({ "id": 1, "name": "michael" });
        let b = json!({ "id": 2, "name": "miguel" });
        assert_eq!(g.hash(&a), g.hash(&b));
    }

    #[test]
    fn different_keys_produce_different_id() {
        let mut g = genome();
        let a = json!({ "id": 1, "name": "michael" });
        let b = json!({ "id": 1, "title": "michael" });
        assert_ne!(g.hash(&a), g.hash(&b));
    }

    #[test]
    fn different_array_lengths_differ_by_default() {
        let mut g = genome();
        let a = json!({ "items": [1, 2, 3] });
        let b = json!({ "items": [1, 2] });
        assert_ne!(g.hash(&a), g.hash(&b));
    }

    #[test]
    fn ignore_array_length_same_element_shape() {
        let mut g = Genome::new(GenomeConfig {
            ignore_array_length: true,
            ..Default::default()
        });
        assert_eq!(
            g.hash(&json!({ "items": [1, 2, 3] })),
            g.hash(&json!({ "items": [1, 2] }))
        );
    }

    #[test]
    fn ignore_array_length_different_shapes_still_differ() {
        let mut g = Genome::new(GenomeConfig {
            ignore_array_length: true,
            ..Default::default()
        });
        assert_ne!(
            g.hash(&json!({ "items": [1, "a"] })),
            g.hash(&json!({ "items": [1, 2] }))
        );
    }

    #[test]
    fn ignore_value_types_same_keys_different_types() {
        let mut g = Genome::new(GenomeConfig {
            ignore_value_types: true,
            ..Default::default()
        });
        let a = json!({ "id": 1, "name": "foo", "sub": { "foo": "bar" } });
        let b = json!({ "id": 2, "name": "x",   "sub": { "foo": 3     } });
        assert_eq!(g.hash(&a), g.hash(&b));
    }

    #[test]
    fn ignore_value_types_different_keys_still_differ() {
        let mut g = Genome::new(GenomeConfig {
            ignore_value_types: true,
            ..Default::default()
        });
        assert_ne!(
            g.hash(&json!({ "id": 1, "name": "foo" })),
            g.hash(&json!({ "id": 1, "title": "foo" }))
        );
    }

    #[test]
    fn identical_ids_score_1() {
        let g = genome();
        let result = g.compare("L0:100-L1:200-L2:300", "L0:100-L1:200-L2:300");
        assert_eq!(result.score, 1.0);
        assert_eq!(result.matched_levels, 3);
    }

    #[test]
    fn completely_different_ids_score_low() {
        let g = genome();
        let result = g.compare("L0:100-L1:200", "L0:999-L1:888");
        assert!(result.score < 0.5);
    }

    #[test]
    fn compare_values_same_shape_scores_1() {
        let mut g = genome();
        let a = json!({ "id": 1, "name": "michael" });
        let b = json!({ "id": 2, "name": "miguel" });
        assert_eq!(g.compare_values(&a, &b).score, 1.0);
    }

    #[test]
    fn conversation_example() {
        let mut g = genome();
        let a = json!({
            "id": 1, "name": "something",
            "sub": { "foo": "bar" },
            "arr": [{ "name": "michael", "age": 23 }, { "name": "miguel", "age": 32 }]
        });
        let b = json!({
            "id": 2, "name": "something else",
            "sub": { "foo": 3 },
            "arr": [{ "name": "michael", "age": 23 }]
        });

        assert_ne!(g.hash(&a), g.hash(&b));

        let mut g2 = Genome::new(GenomeConfig {
            ignore_array_length: true,
            ignore_value_types: true,
            ..Default::default()
        });
        assert_eq!(g2.hash(&a), g2.hash(&b));
    }
}
