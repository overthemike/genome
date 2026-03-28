![NPM Version](https://img.shields.io/npm/v/genome?style=flat-square&color=%23e8b339)
![Crates.io Version](https://img.shields.io/crates/v/genome-rs?style=flat-square&color=%23e8b339)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/overthemike/genome/ci.yml?style=flat-square&color=%23e8b339)
![npm bundle size](https://img.shields.io/bundlephobia/minzip/genome?style=flat-square&color=%23e8b339)
![NPM License](https://img.shields.io/npm/l/genome?style=flat-square&color=%23e8b339)
# `genome`

Deterministic structural hashing for JSON values. Generates hierarchical 
IDs that capture the shape of an object — same structure, same ID, 
regardless of values.

## Install

**[npm (WASM) ↗](https://www.npmjs.com/package/genome)**
```bash
npm install genome
```

**[Rust ↗](https://crates.io/crates/genome-rs)**
```toml
[dependencies]
genome = "1.0.0"
```

## Usage

**TypeScript / JavaScript**
```ts
import init, { hash, signature, compare, compareValues, setConfig, reset } from 'genome'

// `init()` loads and compiles the WASM binary — call it once before using any other functions.
await init()

const id = hash(JSON.stringify({ id: 1, name: "alice" }))
const score = compare(id1, id2)

// setConfig(newIdOnCollision, ignoreArrayLength, ignoreValueTypes)
setConfig(false, true, false)
```

**Rust**
```rust
use genome::{Genome, GenomeConfig};
use serde_json::json;

let mut g = Genome::new(GenomeConfig::default());

let id1 = g.hash(&json!({ "id": 1, "name": "alice" }));
let id2 = g.hash(&json!({ "id": 2, "name": "bob" }));

assert_eq!(id1, id2); // same structure
```

## Config

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `newIdOnCollision` | bool | false | Give structurally identical values distinct IDs |
| `ignoreArrayLength` | bool | false | Treat arrays with different lengths but same element shapes as equivalent |
| `ignoreValueTypes` | bool | false | Treat all scalar types as equivalent — only key names and depth matter |

## API

### `hash(json)`
Accepts a JSON string. Use `JSON.stringify()` before passing.

### `signature(value)`
Returns the structural fingerprint. In default mode returns the full ID. 
When `newIdOnCollision` is true, strips L0 and returns L1+ only.

### `compare(idA, idB) → number`
Compares two structure IDs and returns a similarity score from 0.0 to 1.0.

### `compareValues(a, b)`
Compares two JSON strings structurally.

### `seed(sig, count)`
Seeds the collision counter for a known signature. Useful for restoring persisted state.

### `reset()`
Clears all internal state.

## How it works

genome builds a hierarchical hash where each level (`L0`, `L1`, `L2`...) 
represents a depth in the object tree. Two objects with the same keys, 
same nesting structure, and same value types will always produce the same ID 
regardless of the actual values stored.

```
{ id: 1, name: "alice", address: { city: "NYC" } }
  └── L0: root level  (keys: id, name, address + their types)
  └── L1: depth 1     (keys inside address: city + its type)
```

## License
MIT