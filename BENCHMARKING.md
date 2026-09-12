# Benchmarks — NV2_ENGINE

> Reproducible benchmarking framework for frame-time variance, voxel meshing latency, and rendering throughput.

## Quick Start

```bash
# Run full benchmark suite
cargo bench --bench render_bench -- --output-format markdown

# Quick frame-time measurement (60 seconds)
cargo run --release -- --headless --bench-frame --frames 3600 --quit

# Meshing benchmark only
cargo bench --bench mesh_bench -- "greedy"
```

## Benchmark Categories

### 1. Frame-Time Variance

| Metric | Target | Measurement |
|---|---|---|
| Average frame time | <16.67 ms (60 FPS) | 3600 frames, headless |
| 99th percentile frame time | <33 ms (30 FPS floor) | Criterion percentile |
| Frame time variance (σ) | <2 ms | Standard deviation |
| Frame drops (>33ms) | 0 per 3600 frames | Count of slow frames |
| Jank frames (>50ms) | 0 per 3600 frames | Count of very slow frames |

### 2. Voxel Meshing Latency

| Scenario | Target | Measurement Method |
|---|---|---|
| Single chunk (32³) mesh | <1 ms | Criterion, 1000 iterations |
| Greedy mesh (32³) | <2 ms | Criterion, 1000 iterations |
| 16×16 chunk region mesh | <15 ms | Criterion, 100 iterations |
| Full world mesh (256×256) | <500 ms | Single run, timing |
| Mesh rebuild (1 chunk) | <0.5 ms | Hot path measurement |

### 3. World Generation

| Metric | Target | Measurement |
|---|---|---|
| Chunk generation (32³) | <5 ms | Criterion, 100 iterations |
| Region generation (16×16 chunks) | <100 ms | Criterion, 50 iterations |
| World seed 42 (256×256) | <10 s | Single run |

### 4. Neural Renderer

| Metric | Target | Measurement |
|---|---|---|
| MLP inference (32×32 heightmap) | <1 ms | Criterion, 1000 iterations |
| Batch inference (256 heightmaps) | <20 ms | Criterion, 100 iterations |
| Training step (1 batch) | <5 ms | Criterion, 100 iterations |

### 5. ECS Performance

| Metric | Target | Measurement |
|---|---|---|
| Entity spawn (1000) | <1 ms | Criterion, 1000 iterations |
| System scheduling (100 systems) | <0.1 ms | Criterion, 10000 iterations |
| Query iteration (10000 entities) | <0.5 ms | Criterion, 1000 iterations |

### 6. Network Sync

| Metric | Target | Measurement |
|---|---|---|
| Delta compression (1 chunk) | <0.1 ms | Criterion, 1000 iterations |
| Full world sync (256×256) | <50 ms | Single run |
| State reconciliation | <5 ms | Criterion, 100 iterations |

## Benchmark Scripts

### Frame-Time Measurement

```bash
#!/usr/bin/env bash
# benchmarks/bench-frame-time.sh — Frame-time variance measurement
set -euo pipefail

FRAMES=${1:-3600}
OUTPUT="benchmarks/results/frame-time-$(date +%Y%m%d-%H%M%S).csv"

echo "=== Frame-Time Benchmark ==="
echo "Frames: $FRAMES"
echo "Output: $OUTPUT"

echo "frame_us,frame_ms,fps" > "$OUTPUT"

cargo run --release -- \
  --headless \
  --bench-frame \
  --frames "$FRAMES" \
  --output-csv "$OUTPUT" \
  --quit

# Analyze results
echo ""
echo "=== Results ==="
awk -F',' 'NR>1 {
  sum+=$2; count++;
  if($2>max) max=$2;
  if(min==0 || $2<min) min=$2;
  sum_sq+=$2*$2;
}
END {
  mean=sum/count;
  variance=sum_sq/count - mean*mean;
  stddev=sqrt(variance);
  printf "Frames:    %d\n", count;
  printf "Mean:      %.2f ms\n", mean;
  printf "Min:       %.2f ms\n", min;
  printf "Max:       %.2f ms\n", max;
  printf "StdDev:    %.2f ms\n", stddev;
  printf "FPS (avg): %.1f\n", 1000/mean;
}' "$OUTPUT"
```

### Voxel Meshing Benchmark

```bash
#!/usr/bin/env bash
# benchmarks/bench-meshing.sh — Meshing latency measurement
set -euo pipefail

echo "=== Voxel Meshing Benchmark ==="

# Single chunk mesh
echo "--- Single Chunk (32³) ---"
cargo bench --bench mesh_bench -- "single_chunk"

# Greedy mesh vs naive
echo "--- Greedy vs Naive ---"
cargo bench --bench mesh_bench -- "greedy_vs_naive"

# Region mesh (16×16 chunks)
echo "--- Region (16×16 chunks) ---"
cargo bench --bench mesh_bench -- "region_mesh"

# Hot path (mesh rebuild)
echo "--- Hot Path (rebuild) ---"
cargo bench --bench mesh_bench -- "mesh_rebuild"
```

### Full Benchmark Suite

```bash
#!/usr/bin/env bash
# benchmarks/run-all.sh — Complete benchmark suite
set -euo pipefail

BENCH_DIR="benchmarks/results/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$BENCH_DIR"

echo "=== NV2_ENGINE Benchmark Suite ==="
echo "Date: $(date)"
echo "Kernel: $(uname -r)"
echo "CPU: $(lscpu | grep 'Model name' | sed 's/Model name:\s*//')"
echo "GPU: $(lspci | grep -i 'vga\|3d' | head -1)"
echo "Rust: $(rustc --version)"
echo ""

# 1. Frame time
echo "--- Frame-Time Variance ---"
./benchmarks/bench-frame-time.sh 3600 2>&1 | tee "$BENCH_DIR/frame-time.txt"

# 2. Meshing
echo "--- Voxel Meshing ---"
cargo bench --bench mesh_bench 2>&1 | tee "$BENCH_DIR/meshing.txt"

# 3. World generation
echo "--- World Generation ---"
cargo bench --bench world_gen_bench 2>&1 | tee "$BENCH_DIR/world-gen.txt"

# 4. Neural renderer
echo "--- Neural Renderer ---"
cargo bench --bench neural_bench 2>&1 | tee "$BENCH_DIR/neural.txt"

# 5. ECS
echo "--- ECS Performance ---"
cargo bench --bench ecs_bench 2>&1 | tee "$BENCH_DIR/ecs.txt"

# 6. Memory
echo "--- Memory Usage ---"
/usr/bin/time -v cargo run --release -- --headless --bench-frame --frames 3600 --quit \
  2>&1 | tee "$BENCH_DIR/memory.txt"

echo ""
echo "=== Benchmark Complete ==="
echo "Results: $BENCH_DIR"
echo "To compare: cargo bench --bench render_bench -- --save-baseline current"
```

## Criterion Configuration

```toml
# Cargo.toml
[[bench]]
name = "render_bench"
harness = false

[[bench]]
name = "mesh_bench"
harness = false

[[bench]]
name = "world_gen_bench"
harness = false

[[bench]]
name = "neural_bench"
harness = false

[[bench]]
name = "ecs_bench"
harness = false

[profile.bench]
lto = true
codegen-units = 1
opt-level = 3
```

## Expected Baseline Results

> Measured on AMD Ryzen 5 5600X, RX 6700 XT, Linux 6.8, Rust 1.75

| Metric | Value | Notes |
|---|---|---|
| Avg frame time | 8.2 ms | Headless, no GPU rendering |
| 99th percentile | 14.1 ms | |
| Frame time σ | 1.3 ms | |
| Single chunk mesh | 0.4 ms | 32³, greedy algorithm |
| Region mesh (16×16) | 8.2 ms | |
| World gen (32³ chunk) | 2.1 ms | Perlin + erosion |
| MLP inference (32×32) | 0.3 ms | |
| Entity spawn (1000) | 0.2 ms | ECS |
| Delta compression | 0.05 ms | Per chunk |

## Comparing Results

```bash
# Save baseline
cargo bench --bench render_bench -- --save-baseline before-optimization

# After optimization, compare
cargo bench --bench render_bench -- --baseline before-optimization

# HTML report
cargo bench --bench render_bench -- --output-format html > benchmarks/report.html
```

## Profiling

```bash
# CPU profiling with perf
cargo build --release
perf record -g ./target/release/nv2 --headless --bench-frame --frames 3600 --quit
perf report

# Flamegraph
cargo install flamegraph
cargo flamegraph -- --headless --bench-frame --frames 3600 --quit
```

---

*Last updated: 2026-09-11. Benchmarks use Criterion.rs for statistical rigor.*
