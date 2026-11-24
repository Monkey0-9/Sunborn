# Sunborn Fuzzing Suite (`fuzz/`)

Coverage-guided fuzzing targets utilizing `cargo-fuzz` and LLVM libFuzzer to test Sunborn parser boundaries, tokenizers, and canonicalization routines.

## Targets

- **`fuzz_canonicalize`**: Tests RFC 8785 JSON canonicalization against arbitrary byte inputs.
- **`fuzz_merkle`**: Tests Merkle DAG deserialization and digest generation against malformed trees.
- **`fuzz_pcsr`**: Tests PCSR certificate parsing and cryptographic boundary validation.

## Running Fuzzers

```bash
cargo fuzz run fuzz_canonicalize
```
