# Branch Name Generator Model

A tiny (~20M parameter) language model that generates descriptive git branch names from natural language prompts in <100ms on CPU.

## Why

When Claude Code calls `quick_claude` or `create_terminal` with worktree mode, it either:
- Spends seconds on LLM inference to generate a `branch_name`
- Omits `branch_name`, resulting in a generic `wt-<uuid>` prefix

This model eliminates that trade-off: descriptive names at near-zero latency.

## Architecture

| | Teacher (SmolLM2-135M) | Student (~20M) |
|---|---|---|
| Base | HuggingFaceTB/SmolLM2-135M-Instruct | LlamaForCausalLM (from scratch) |
| Layers | 30 | 4 |
| Hidden dim | 576 | 256 |
| Attention heads | 9 | 4 |
| Intermediate | 1536 | 1024 |
| Vocab | 49152 | 49152 (shared tokenizer) |
| Format | - | GGUF Q4_K_M (~8-12MB) |

## Training Pipeline

### Prerequisites

```bash
pip install -r requirements.txt
```

Requires Python 3.10+, NVIDIA GPU with CUDA for training (inference is CPU-only).

### Step 1: Generate Training Data

```bash
# With OpenAI API (recommended, ~1,900 examples)
export OPENAI_API_KEY=sk-...
python generate_data.py

# Without API (seeds + augmentation only, ~800 examples)
python generate_data.py --skip-api
```

Three-stage pipeline:
1. **Seeds** (`seeds.jsonl`): 201 hand-written examples covering all 7 prefix types
2. **Rule-based augmentation**: ~500 examples via template expansion and synonym swaps
3. **OpenAI API batch generation**: ~1,200 examples from gpt-4o-mini across 60 software engineering categories

Output: `data/train.jsonl`, `data/val.jsonl`, `data/test.jsonl` (80/10/10 split).

### Step 2: Fine-Tune Teacher

```bash
python finetune_teacher.py
```

LoRA fine-tuning (r=16, alpha=32) of SmolLM2-135M-Instruct with SFTTrainer. 5 epochs, ~10 minutes on a consumer GPU.

Output: `models/teacher-merged/` (full merged HuggingFace model).

Target metrics (on test split):
- Exact match: >85%
- Format compliance: >99%

### Step 3: Distill to Student

```bash
python distill.py
```

Knowledge distillation with combined loss: `0.7 * KL(student, teacher, T=2.0) + 0.3 * CE(student, labels)`. 15 epochs.

Output: `models/student/` (HuggingFace model).

If 4 layers underperforms, try 6 layers:
```bash
python distill.py --student-layers 6
```

### Step 4: Export to GGUF

```bash
python export_gguf.py
```

Converts to GGUF with Q4_K_M quantization via llama.cpp (auto-cloned if not present).

Output: `models/branch-name-generator.gguf` (~8-12MB).

### Step 5: Benchmark

```bash
python benchmark.py
```

Runs 50 generations and reports P50/P95/P99 latency. Target: P95 < 100ms on CPU.

## Deployment

Copy the GGUF model and tokenizer to the app data directory:

```bash
# Windows
mkdir -p "$APPDATA/com.godly.terminal/models/branch-name-gen"
cp models/branch-name-generator.gguf "$APPDATA/com.godly.terminal/models/branch-name-gen/"
cp models/student/tokenizer.json "$APPDATA/com.godly.terminal/models/branch-name-gen/"
```

The app auto-loads the model on startup from:
```
%APPDATA%/com.godly.terminal/models/branch-name-gen/
  ├── branch-name-generator.gguf
  └── tokenizer.json
```

Auto-load happens in `src-tauri/src/llm_state.rs` during `LlmState::init()`, which is called from `lib.rs` at app startup.

## How It Works

### Model Input/Output

The model receives a ChatML-formatted prompt and outputs a slug-only branch name:

```
Input:  "Fix crash when terminal has zero-width columns"
Output: "fix-zero-width-crash"
```

Branch names use a flat prefix convention (no `/` separator):
- `feat-`, `fix-`, `refactor-`, `docs-`, `chore-`, `test-`, `style-`

The worktree system prepends `wt-` automatically, so the final branch becomes `wt-fix-zero-width-crash`.

### Integration Points

1. **`quick_claude` Tauri command** (`src-tauri/src/commands/terminal.rs`): When `branch_name` is `None`, calls `LlmState::try_generate_branch_name(prompt)`
2. **MCP QuickClaude handler** (`src-tauri/src/mcp_server/handler.rs`): Same auto-generation logic
3. **Fallback**: If the engine is not loaded, busy, or generation fails, falls back to UUID prefix (existing behavior)

### End-to-End Flow

```
quick_claude(prompt="Fix crash on zero-width columns", branch_name=None)
  -> BranchNameEngine::generate("Fix crash on zero-width columns")
  -> "fix-zero-width-crash" (<100ms)
  -> wt_name_from("fix-zero-width-crash")
  -> "wt-fix-zero-width-crash"
  -> git worktree add ... -b wt-fix-zero-width-crash
  -> Tab shows "wt-fix-zero-width-crash"
```

## File Structure

```
training/
  seeds.jsonl            # 201 hand-written training examples
  generate_data.py       # Data generation pipeline (seeds + augmentation + OpenAI API)
  finetune_teacher.py    # LoRA fine-tuning of SmolLM2-135M teacher
  distill.py             # Knowledge distillation to tiny student
  export_gguf.py         # GGUF Q4_K_M export via llama.cpp
  benchmark.py           # Inference latency benchmarking
  requirements.txt       # Python dependencies
  .gitignore             # Excludes data/, models/, llama.cpp/
```

## Rust Crate Structure

```
src-tauri/llm/src/
  branch_name_engine.rs  # BranchNameEngine (candle GGUF inference)
  branch_name.rs         # sanitize_branch_name() + full-model generation
  download.rs            # ModelPaths + BranchNameModelPaths
  engine.rs              # LlmEngine (main SmolLM2-135M engine)
  lib.rs                 # Public exports
```

## Troubleshooting

**Model not loading**: Check the daemon log at `%APPDATA%/com.godly.terminal/godly-daemon-debug.log` for `[llm] Branch name engine loaded` or error messages.

**Latency too high**: Try the ONNX fallback path (`export_gguf.py --format onnx`). ONNX Runtime with INT8 quantization can be faster than candle GGUF for small models.

**Quality too low after distillation**: Ship the fine-tuned teacher directly with GGUF export:
```bash
python export_gguf.py --student-dir models/teacher-merged
```
This gives ~80-150ms latency (still acceptable) with higher quality.
