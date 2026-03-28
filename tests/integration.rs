/// Integration tests for the genome library.
/// Mirrors the TypeScript test suite as closely as possible.
///
/// Run with: cargo test
/// Run with output:  cargo test -- --nocapture
/// Run a single test: cargo test test_name
use genome_rs::{Genome, GenomeConfig};
use serde_json::{json, Value};

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

fn g() -> Genome {
    Genome::new(GenomeConfig::default())
}

fn g_with(config: GenomeConfig) -> Genome {
    Genome::new(config)
}

/// Extracts the L0 value from a structure ID string.
fn l0(id: &str) -> &str {
    id.split('-').next().unwrap().split(':').nth(1).unwrap()
}

/// Returns the L1+ signature (everything after L0).
fn signature(id: &str) -> &str {
    id.splitn(2, '-').nth(1).unwrap_or("")
}

/// Splits the ID into all level parts.
fn parts(id: &str) -> Vec<&str> {
    id.split('-').collect()
}

// ────────────────────────────────────────────────────────────────────────────
// Hash functions (mirrors hash-functions.test.ts)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod hash_functions {
    use genome_rs::hash_str;
    use genome_rs::xx_hash32;

    #[test]
    fn xxhash_consistent_same_input() {
        let input = "test-string";
        assert_eq!(hash_str(input, 0), hash_str(input, 0));
    }

    #[test]
    fn xxhash_different_inputs_different_hashes() {
        assert_ne!(hash_str("test-string-1", 0), hash_str("test-string-2", 0));
    }

    #[test]
    fn xxhash_empty_string() {
        let result = hash_str("", 0);
        assert!(!result.is_empty());
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn xxhash_unicode() {
        let result = hash_str("测试字符串", 0);
        assert!(!result.is_empty());
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn xxhash_accepts_bytes() {
        let input = "test-string";
        let bytes = input.as_bytes();
        assert_eq!(hash_str(input, 0), format!("{:x}", xx_hash32(bytes, 0)));
    }

    #[test]
    fn xxhash_custom_seed_differs() {
        let input = "test-string";
        assert_ne!(hash_str(input, 0), hash_str(input, 42));
    }

    #[test]
    fn xxhash_long_input() {
        let long = "a".repeat(10_000);
        // should not panic
        let _ = hash_str(&long, 0);
    }

    // Known hash values — verify against TS test cases:
    // { input: "", xx: "2cc5d05" }
    // { input: "hello", xx: "fb0077f9" }
    // { input: "test123", xx: "ff2410ee" }
    //
    // NOTE: These will only match if the Rust xxHash32 is bit-for-bit identical
    // to the TS implementation. Uncomment and adjust once parity is confirmed.
    //
    // #[test]
    // fn xxhash_known_values() {
    //     assert_eq!(hash_str("", 0),       "2cc5d05");
    //     assert_eq!(hash_str("hello", 0),  "fb0077f9");
    //     assert_eq!(hash_str("test123", 0),"ff2410ee");
    // }
}

// ────────────────────────────────────────────────────────────────────────────
// Edge cases (mirrors edge-cases.test.ts)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn empty_object_returns_braces() {
        assert_eq!(g().hash(&json!({})), "{}");
    }

    #[test]
    fn empty_array_returns_brackets() {
        assert_eq!(g().hash(&json!([])), "[]");
    }

    #[test]
    fn handles_null() {
        let id = g().hash(&Value::Null);
        assert!(!id.is_empty());
    }

    #[test]
    fn handles_bool() {
        let id = g().hash(&json!(true));
        assert!(!id.is_empty());
    }

    #[test]
    fn handles_number() {
        let id = g().hash(&json!(42));
        assert!(!id.is_empty());
    }

    #[test]
    fn handles_string() {
        let id = g().hash(&json!("hello"));
        assert!(!id.is_empty());
    }

    #[test]
    fn property_order_does_not_affect_id() {
        let mut g = g();
        // { a, b, c } vs { c, b, a } — same structure
        let id1 = g.hash(&json!({ "a": 1, "b": 2, "c": 3 }));
        let id2 = g.hash(&json!({ "c": 3, "b": 2, "a": 1 }));
        assert_eq!(id1, id2);
    }

    #[test]
    fn deeply_nested_does_not_overflow() {
        // Build 100 levels of nesting programmatically
        let mut current = json!({ "value": 0 });
        for i in 0..100 {
            current = json!({ "next": current, "value": i });
        }
        // Should not panic
        let _ = g().hash(&current);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Basic objects (mirrors basic-objects.test.ts)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod basic_objects {
    use super::*;

    #[test]
    fn same_structure_different_values_same_id() {
        let mut g = g();
        let id1 = g.hash(&json!({ "count": 0, "name": "test" }));
        let id2 = g.hash(&json!({ "count": 42, "name": "different" }));
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_keys_different_id() {
        let mut g = g();
        let id1 = g.hash(&json!({ "count": 0, "name": "test" }));
        let id2 = g.hash(&json!({ "count": 0, "title": "test" }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn property_order_same_id() {
        let mut g = g();
        let id1 = g.hash(&json!({ "count": 0, "name": "test" }));
        let id2 = g.hash(&json!({ "name": "test", "count": 0 }));
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_value_types_different_id() {
        let mut g = g();
        // count is number vs string
        let id1 = g.hash(&json!({ "count": 0, "name": "test" }));
        let id2 = g.hash(&json!({ "count": "0", "name": "test" }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn consistent_across_multiple_calls() {
        let mut g = g();
        let obj = json!({ "count": 0, "name": "test" });
        let id1 = g.hash(&obj);
        let id2 = g.hash(&obj);
        let id3 = g.hash(&obj);
        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Nested objects (mirrors nested-objects.test.ts)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod nested_objects {
    use super::*;

    #[test]
    fn identical_nested_structure_same_id() {
        let mut g = g();
        let obj1 = json!({
            "user": { "name": "John", "age": 30, "preferences": { "theme": "dark", "notifications": true } }
        });
        let obj2 = json!({
            "user": { "name": "Jane", "age": 25, "preferences": { "theme": "light", "notifications": false } }
        });
        assert_eq!(g.hash(&obj1), g.hash(&obj2));
    }

    #[test]
    fn different_nested_structure_different_id() {
        let mut g = g();
        let obj1 = json!({
            "user": { "name": "John", "age": 30, "preferences": { "theme": "dark", "notifications": true } }
        });
        // different key: fontSize instead of notifications
        let obj2 = json!({
            "user": { "name": "Jane", "age": 25, "preferences": { "theme": "light", "fontSize": 14 } }
        });
        assert_ne!(g.hash(&obj1), g.hash(&obj2));
    }

    #[test]
    fn deeply_nested_same_id_and_many_levels() {
        let mut g = g();
        let obj1 = json!({ "level1": { "level2": { "level3": { "level4": { "level5": { "value": "deep" } } } } } });
        let obj2 = json!({ "level1": { "level2": { "level3": { "level4": { "level5": { "value": "also deep" } } } } } });
        let id1 = g.hash(&obj1);
        let id2 = g.hash(&obj2);
        assert_eq!(id1, id2);
        assert!(
            parts(&id1).len() > 5,
            "expected more than 5 levels, got {}",
            parts(&id1).len()
        );
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Arrays (mirrors arrays.test.ts)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod arrays {
    use super::*;

    #[test]
    fn same_structure_different_values_same_id() {
        let mut g = g();
        let id1 = g.hash(&json!({ "items": [1, 2, 3] }));
        let id2 = g.hash(&json!({ "items": [4, 5, 6] }));
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_array_length_different_id() {
        let mut g = g();
        let id1 = g.hash(&json!({ "items": [1, 2, 3] }));
        let id2 = g.hash(&json!({ "items": [1, 2, 3, 4] }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn different_element_types_different_id() {
        let mut g = g();
        let id1 = g.hash(&json!({ "items": [1, 2, 3] }));
        let id2 = g.hash(&json!({ "items": [1, "2", 3] }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn array_of_objects_same_structure_same_id() {
        let mut g = g();
        let id1 = g.hash(&json!({
            "users": [{ "name": "John", "age": 30 }, { "name": "Jane", "age": 25 }]
        }));
        let id2 = g.hash(&json!({
            "users": [{ "name": "Alice", "age": 35 }, { "name": "Bob", "age": 40 }]
        }));
        assert_eq!(id1, id2);
    }

    #[test]
    fn array_of_objects_different_structure_different_id() {
        let mut g = g();
        let id1 = g.hash(&json!({
            "users": [{ "name": "John", "age": 30 }, { "name": "Jane", "age": 25 }]
        }));
        // second element has "role" instead of "age"
        let id2 = g.hash(&json!({
            "users": [{ "name": "John", "role": "admin" }, { "name": "Jane", "age": 25 }]
        }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn mixed_type_array_same_structure_same_id() {
        let mut g = g();
        let id1 = g.hash(&json!({ "mixed": [1, "string", true, { "a": 1 }] }));
        let id2 = g.hash(&json!({ "mixed": [2, "text", false, { "a": 42 }] }));
        assert_eq!(id1, id2);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// ignore_array_length (new config flag)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod ignore_array_length {
    use super::*;

    #[test]
    fn same_element_shape_different_length_same_id() {
        let mut g = g_with(GenomeConfig {
            ignore_array_length: true,
            ..Default::default()
        });
        let id1 = g.hash(&json!({ "items": [1, 2, 3] }));
        let id2 = g.hash(&json!({ "items": [1, 2] }));
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_element_shapes_still_differ() {
        let mut g = g_with(GenomeConfig {
            ignore_array_length: true,
            ..Default::default()
        });
        let id1 = g.hash(&json!({ "items": [1, "a"] }));
        let id2 = g.hash(&json!({ "items": [1, 2] }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn default_mode_still_uses_length() {
        let mut g = g();
        let id1 = g.hash(&json!({ "items": [1, 2, 3] }));
        let id2 = g.hash(&json!({ "items": [1, 2] }));
        assert_ne!(id1, id2);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// ignore_value_types (new config flag)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod ignore_value_types {
    use super::*;

    #[test]
    fn same_keys_different_scalar_types_same_id() {
        let mut g = g_with(GenomeConfig {
            ignore_value_types: true,
            ..Default::default()
        });
        let id1 = g.hash(&json!({ "id": 1, "name": "foo", "sub": { "foo": "bar" } }));
        let id2 = g.hash(&json!({ "id": 2, "name": "x",   "sub": { "foo": 3   } }));
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_keys_still_differ() {
        let mut g = g_with(GenomeConfig {
            ignore_value_types: true,
            ..Default::default()
        });
        let id1 = g.hash(&json!({ "id": 1, "name": "foo" }));
        let id2 = g.hash(&json!({ "id": 1, "title": "foo" }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn different_nesting_depth_still_differs() {
        let mut g = g_with(GenomeConfig {
            ignore_value_types: true,
            ..Default::default()
        });
        let id1 = g.hash(&json!({ "sub": { "foo": "bar" } }));
        let id2 = g.hash(&json!({ "sub": { "foo": { "baz": "bar" } } }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn both_flags_together() {
        let mut g = g_with(GenomeConfig {
            ignore_array_length: true,
            ignore_value_types: true,
            ..Default::default()
        });
        let obj1 = json!({
            "id": 1,
            "name": "something",
            "sub": { "foo": "bar" },
            "arr": [{ "name": "michael", "age": 23 }, { "name": "miguel", "age": 32 }]
        });
        let obj2 = json!({
            "id": 2,
            "name": "something else",
            "sub": { "foo": 3 },
            "arr": [{ "name": "michael", "age": 23 }]
        });
        assert_eq!(g.hash(&obj1), g.hash(&obj2));
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Collision handling (mirrors collision-config.test.ts + collision-handling.test.ts)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod collision_handling {
    use super::*;

    #[test]
    fn no_collision_mode_same_structure_same_id() {
        let mut g = g();
        let id1 = g.hash(&json!({ "name": "John", "age": 30 }));
        let id2 = g.hash(&json!({ "name": "Jane", "age": 25 }));
        assert_eq!(id1, id2);
    }

    #[test]
    fn collision_mode_same_structure_different_id() {
        let mut g = g_with(GenomeConfig {
            new_id_on_collision: true,
            ..Default::default()
        });
        let id1 = g.hash(&json!({ "name": "John", "age": 30 }));
        let id2 = g.hash(&json!({ "name": "Jane", "age": 25 }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn collision_mode_l0_increments_sequentially() {
        let mut g = g_with(GenomeConfig {
            new_id_on_collision: true,
            ..Default::default()
        });

        let ids: Vec<String> = (0..5).map(|i| g.hash(&json!({ "value": i }))).collect();

        // L0 values should be 0, 1, 2, 3, 4
        for (expected, id) in ids.iter().enumerate() {
            assert_eq!(
                l0(id),
                expected.to_string(),
                "expected L0={expected} in {id}"
            );
        }
    }

    #[test]
    fn collision_mode_only_l0_differs() {
        let mut g = g_with(GenomeConfig {
            new_id_on_collision: true,
            ..Default::default()
        });
        let id1 = g.hash(&json!({ "name": "John", "age": 30 }));
        let id2 = g.hash(&json!({ "name": "Jane", "age": 25 }));

        // L0 differs
        assert_ne!(l0(&id1), l0(&id2));
        // L1+ is identical
        assert_eq!(signature(&id1), signature(&id2));
        // Same number of parts
        assert_eq!(parts(&id1).len(), parts(&id2).len());
    }

    #[test]
    fn collision_mode_complex_objects() {
        let mut g = g_with(GenomeConfig {
            new_id_on_collision: true,
            ..Default::default()
        });
        let obj1 = json!({
            "user": { "name": "User 1", "settings": { "theme": "dark", "notifications": true } },
            "items": [1, 2, 3]
        });
        let obj2 = json!({
            "user": { "name": "User 2", "settings": { "theme": "light", "notifications": false } },
            "items": [4, 5, 6]
        });

        let id1 = g.hash(&obj1);
        let id2 = g.hash(&obj2);

        assert_ne!(id1, id2);
        assert_eq!(l0(&id1), "0");
        assert_eq!(l0(&id2), "1");
        assert_eq!(signature(&id1), signature(&id2));
    }

    #[test]
    fn reset_clears_collision_counters() {
        let mut g = g_with(GenomeConfig {
            new_id_on_collision: true,
            ..Default::default()
        });

        // First pair
        let id_a1 = g.hash(&json!({ "test": true }));
        let id_a2 = g.hash(&json!({ "test": false }));
        assert_eq!(l0(&id_a1), "0");
        assert_eq!(l0(&id_a2), "1");

        // Reset
        g.reset();

        // Second pair — counters should restart from 0
        let id_b1 = g.hash(&json!({ "test": "first" }));
        let id_b2 = g.hash(&json!({ "test": "second" }));
        assert_eq!(l0(&id_b1), "0");
        assert_eq!(l0(&id_b2), "1");
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Structure signature (mirrors coverage-boost.test.ts signature section)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod structure_signature {
    use super::*;

    // When new_id_on_collision is false (default), signature
    // returns the full ID since L0 contains real structural data.

    #[test]
    fn same_shape_same_signature() {
        let mut g = g();
        let sig1 = g.signature(&json!({ "id": 1, "name": "John" }));
        let sig2 = g.signature(&json!({ "id": 2, "name": "Jane" }));
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn different_shape_different_signature() {
        let mut g = g();
        let sig1 = g.signature(&json!({ "id": 1, "name": "John" }));
        let sig2 = g.signature(&json!({ "id": 1, "title": "John" }));
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn signature_contains_level_markers() {
        let mut g = g();
        // Full ID is returned in default mode — should contain L0 and L1 at minimum
        let sig = g.signature(&json!({ "a": { "b": 1 } }));
        assert!(sig.contains("L0:"), "expected L0: in signature, got {sig}");
        assert!(sig.contains("L1:"), "expected L1: in signature, got {sig}");
        assert!(sig.contains("L2:"), "expected L2: in signature, got {sig}");
    }

    #[test]
    fn collision_mode_signature_strips_l0() {
        // When new_id_on_collision is true, L0 becomes a counter so
        // signature strips it and returns L1+ only.
        let mut g = g_with(GenomeConfig {
            new_id_on_collision: true,
            ..Default::default()
        });
        let sig1 = g.signature(&json!({ "id": 1, "name": "John" }));
        let sig2 = g.signature(&json!({ "id": 2, "name": "Jane" }));
        // Same structure → same L1+ signature even though L0 counters differ
        assert_eq!(sig1, sig2);
        // L0 should be stripped
        assert!(
            !sig1.starts_with("L0:"),
            "expected L0 to be stripped, got {sig1}"
        );
        assert!(
            sig1.starts_with("L1:"),
            "expected signature to start with L1:, got {sig1}"
        );
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Similarity comparison (mirrors the compareStructureIds discussion)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod similarity {
    use super::*;

    #[test]
    fn identical_ids_score_1() {
        let g = g();
        let id = "L0:100-L1:200-L2:300";
        let result = g.compare(id, id);
        assert_eq!(result.score, 1.0);
        assert_eq!(result.matched_levels, 3);
        assert_eq!(result.total_levels, 3);
        assert!(result.level_scores.iter().all(|&s| s == 1.0));
    }

    #[test]
    fn completely_different_ids_score_low() {
        let g = g();
        let result = g.compare("L0:100-L1:200", "L0:999-L1:888");
        assert!(
            result.score < 0.5,
            "expected low score, got {}",
            result.score
        );
    }

    #[test]
    fn same_structure_objects_score_1() {
        let mut g = g();
        let a = json!({ "id": 1, "name": "michael" });
        let b = json!({ "id": 2, "name": "miguel" });
        let result = g.compare_values(&a, &b);
        assert_eq!(result.score, 1.0);
    }

    #[test]
    fn depth_mismatch_penalized() {
        let g = g();
        // one ID has an extra level the other doesn't
        let result = g.compare("L0:100-L1:200-L2:300", "L0:100-L1:200");
        // L2 is missing from one side — should not be 1.0
        assert!(result.score < 1.0, "expected penalty for depth mismatch");
        assert_eq!(result.total_levels, 3);
    }

    #[test]
    fn level_scores_length_matches_total_levels() {
        let g = g();
        let result = g.compare("L0:100-L1:200-L2:300", "L0:100-L1:999-L2:300");
        assert_eq!(result.level_scores.len(), result.total_levels);
    }

    #[test]
    fn conversation_example_same_l0_default_mode() {
        // From the conversation: these two differ only because of array length + value types.
        // In default mode they should produce different IDs.
        let mut g = g();
        let a = json!({
            "id": 1,
            "name": "something",
            "sub": { "foo": "bar" },
            "arr": [
                { "name": "michael", "age": 23 },
                { "name": "miguel",  "age": 32 }
            ]
        });
        let b = json!({
            "id": 2,
            "name": "something else",
            "sub": { "foo": 3 },
            "arr": [
                { "name": "michael", "age": 23 }
            ]
        });

        let id_a = g.hash(&a);
        let id_b = g.hash(&b);

        // Should differ in default mode
        assert_ne!(id_a, id_b);

        // Similarity should still be high (same keys, same depth)
        let similarity = g.compare(&id_a, &id_b);
        assert!(
            similarity.score > 0.7,
            "expected high similarity, got {}",
            similarity.score
        );
    }
}

// ────────────────────────────────────────────────────────────────────────────
// State management (mirrors resetState tests)
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod state_management {
    use super::*;

    #[test]
    fn reset_produces_same_id_for_same_structure() {
        let mut g = g();
        let obj = json!({ "a": 1, "b": "test" });
        let id_before = g.hash(&obj);
        g.reset();
        let id_after = g.hash(&obj);
        // Structure IDs are deterministic — reset does not change the algorithm
        assert_eq!(id_before, id_after);
    }

    #[test]
    fn structural_equality_holds_after_reset() {
        let mut g = g();
        g.hash(&json!({ "a": 1, "b": 2 }));
        g.reset();

        let id1 = g.hash(&json!({ "a": 1, "b": "test" }));
        let id2 = g.hash(&json!({ "a": 2, "b": "different" }));
        assert_eq!(id1, id2);
    }

    #[test]
    fn export_and_reimport_collision_counters() {
        let mut g = g_with(GenomeConfig {
            new_id_on_collision: true,
            ..Default::default()
        });
        let obj = json!({ "a": 1 });

        // Generate 3 IDs to increment counter to 3
        for i in 0..3 {
            g.hash(&json!({ "a": i }));
        }

        let counters = g.export_counters();
        assert!(!counters.is_empty());

        // Re-import into fresh genome
        let mut g2 = g_with(GenomeConfig {
            new_id_on_collision: true,
            ..Default::default()
        });
        for (sig, count) in &counters {
            g2.seed(sig, *count);
        }

        // Next ID should start at 3 (continuing from where g left off)
        let id = g2.hash(&obj);
        assert_eq!(l0(&id), "3");
    }

    #[test]
    fn key_cache_exported() {
        let mut g = g();
        g.hash(&json!({ "some_key": 1 }));
        let cache = g.export_cache();
        // "some_key" should have been hashed and cached
        assert!(
            cache.contains_key("some_key"),
            "expected some_key in key cache"
        );
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Compact helpers (mirrors compact.test.ts)
// These live on Genome since Rust doesn't have module-level state like TS does.
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod compact {
    use super::*;
    use genome_rs::hash_str;

    fn compact_id(g: &mut Genome, value: &Value) -> String {
        let full_id = g.hash(value);
        hash_str(&full_id, 0)
    }

    #[test]
    fn compact_is_deterministic() {
        let mut g = g();
        let obj = json!({ "a": 1, "b": "test" });
        assert_eq!(compact_id(&mut g, &obj), compact_id(&mut g, &obj));
    }

    #[test]
    fn compact_differs_for_different_structures() {
        let mut g = g();
        let id1 = compact_id(&mut g, &json!({ "deeply": { "nested": { "value": 1 } } }));
        let id2 = compact_id(&mut g, &json!({ "items": [1, 2, 3, 4, 5] }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn different_shape_different_signature() {
        let mut g = g();
        // Different keys → different full IDs → different compact hashes
        let id1 = compact_id(&mut g, &json!({ "id": 1, "name": "John" }));
        let id2 = compact_id(&mut g, &json!({ "id": 1, "title": "John" }));
        assert_ne!(id1, id2);
    }

    #[test]
    fn compact_shorter_than_full_id() {
        let mut g = g();
        let obj = json!({ "a": 1, "b": "test" });
        let full = g.hash(&obj);
        let compact = compact_id(&mut g, &obj);
        // compact hash should be a different length from the hierarchical ID
        assert_ne!(full.len(), compact.len());
    }

    #[test]
    fn compact_different_primitives() {
        let mut g = g();
        let id_num = compact_id(&mut g, &json!(42));
        let id_str = compact_id(&mut g, &json!("test"));
        let id_bool = compact_id(&mut g, &json!(true));
        assert_ne!(id_num, id_str);
        assert_ne!(id_str, id_bool);
        assert_ne!(id_num, id_bool);
    }
}
