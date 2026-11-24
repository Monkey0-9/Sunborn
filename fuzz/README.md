# Sunborn Fuzzing Suite (`fuzz/`)

Coverage-guided fuzzing targets utilizing `cargo-fuzz` and LLVM libFuzzer to test Sunborn parser boundaries, tokenizers, and canonicalization routines against adversarial inputs.

## Fuzz Targets

- **`fuzz_canonicalize`** (`fuzz/fuzz_targets/fuzz_canonicalize.rs`): Tests RFC 8785 JSON Canonicalization Scheme (JCS) and Unicode NFC normalization against arbitrary and malformed byte streams to ensure crash freedom and deterministic canonicalization.

## Running Fuzzers

Fuzz targets require the nightly Rust compiler and `cargo-fuzz`:

```bash
# Install cargo-fuzz if not already installed
cargo install cargo-fuzz

# Run fuzz target
cargo +nightly fuzz run fuzz_canonicalize -- -max_total_time=60
```
