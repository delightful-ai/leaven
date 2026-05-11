## Boundary
This crate is the CUDA domain adapter placeholder: kernel artifacts, CUDA
surfaces, profiling evidence, benchmark runners, and evaluators.

Current public names are scaffolding. They do not prove CUDA compilation,
profiling, GPU availability, workspace integration, or benchmark correctness.

## Local Bait
- GPU/runtime details stay here or in a concrete workspace backend. Do not push
  CUDA facts into cold core, engine, or generic evidence types.
- Benchmark evidence must preserve device/config/profiler context when it
  becomes real; do not collapse it to a bare scalar score in this crate.
- Live GPU tests must be opt-in and explicit about hardware/driver
  requirements.

## Verification
- `cargo check -p leaven-cuda` proves only scaffold exports.
- Real behavior needs deterministic parser/surface tests plus opt-in GPU
  benchmark tests that report hardware assumptions and cleanup behavior.
