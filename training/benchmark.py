#!/usr/bin/env python3
"""
Phase 4: Benchmark inference latency for the student model.

Tests both HuggingFace (torch) and GGUF (llama-cpp-python) inference.
Reports P50/P95/P99 latency over 100 generations.

Usage:
    python benchmark.py [--model-dir models/student] [--gguf models/branch-name-generator.gguf]
"""

import argparse
import json
import re
import statistics
import time
from pathlib import Path


SYSTEM_PROMPT = (
    "You generate concise git branch names from descriptions. "
    "Output only the branch name slug (e.g. fix-crash-on-resize). "
    "Use prefixes: feat-, fix-, refactor-, docs-, chore-, test-, style-. "
    "Use only lowercase, numbers, hyphens. Max 50 chars."
)

SAMPLE_PROMPTS = [
    "Fix crash when terminal has zero-width columns",
    "Add dark mode toggle to settings",
    "Refactor database connection pooling",
    "Update README with installation instructions",
    "Bump dependencies to latest versions",
    "Add unit tests for authentication middleware",
    "Fix inconsistent button spacing in sidebar",
    "Implement WebSocket reconnection with exponential backoff",
    "Fix memory leak in event listener cleanup",
    "Extract shared validation logic into utils module",
    "Add search functionality to file explorer",
    "Fix race condition in concurrent file writes",
    "Build plugin system for custom extensions",
    "Fix SSL certificate validation bypass",
    "Add multi-language support with i18n",
    "Fix infinite loop in retry logic",
    "Implement auto-complete for search input",
    "Fix session token not refreshing on expiry",
    "Add keyboard shortcuts for common actions",
    "Implement virtual scrolling for large lists",
    "Fix deadlock in thread pool shutdown",
    "Add end-to-end tests for checkout flow",
    "Configure ESLint strict mode",
    "Implement drag and drop file upload",
    "Fix off-by-one error in pagination",
    "Add clipboard copy support for code blocks",
    "Fix null pointer exception in user profile",
    "Migrate from Webpack to Vite",
    "Implement undo/redo for text editor",
    "Fix file descriptor leak in pipe handler",
    "Build responsive grid layout system",
    "Fix buffer overflow in binary parser",
    "Add two-factor authentication support",
    "Fix process zombie after abnormal exit",
    "Implement command palette with fuzzy search",
    "Fix pipe reader hanging after child exit",
    "Add tests for concurrent session handling",
    "Implement tab completion in terminal input",
    "Fix window resize causing layout corruption",
    "Implement image annotation and markup tools",
    "Fix signal handler not cleaning up temp files",
    "Add batch processing for bulk operations",
    "Fix stack overflow in recursive tree traversal",
    "Add toast notification component",
    "Fix XSS vulnerability in comment rendering",
    "Implement column resizing for data tables",
    "Fix incorrect cache invalidation on update",
    "Build custom theming engine for white-label",
    "Fix incorrect permissions check for admin role",
    "Implement split pane view for diff comparison",
]


def sanitize(name: str) -> str:
    name = name.strip().split("\n")[0].strip("\"'`").lower()
    name = re.sub(r"[^a-z0-9\\-]", "-", name)
    name = re.sub(r"-{2,}", "-", name).strip("-")
    return name[:50]


def benchmark_torch(model_dir: Path, prompts: list[str]) -> dict:
    """Benchmark HuggingFace torch inference."""
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer

    print(f"\n{'='*60}")
    print(f"Benchmarking Torch model: {model_dir}")
    print(f"{'='*60}")

    tokenizer = AutoTokenizer.from_pretrained(str(model_dir))
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    model = AutoModelForCausalLM.from_pretrained(str(model_dir), torch_dtype=torch.float32)
    model.eval()

    param_count = sum(p.numel() for p in model.parameters())
    print(f"Parameters: {param_count / 1e6:.1f}M")

    # Warmup
    print("Warming up...")
    for prompt in prompts[:3]:
        full_prompt = (
            f"<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n"
            f"<|im_start|>user\n{prompt}<|im_end|>\n"
            f"<|im_start|>assistant\n"
        )
        inputs = tokenizer(full_prompt, return_tensors="pt")
        with torch.no_grad():
            model.generate(**inputs, max_new_tokens=20, do_sample=False,
                           pad_token_id=tokenizer.eos_token_id)

    # Benchmark
    print(f"Running {len(prompts)} generations...")
    latencies = []
    outputs = []
    format_ok = 0

    for prompt in prompts:
        full_prompt = (
            f"<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n"
            f"<|im_start|>user\n{prompt}<|im_end|>\n"
            f"<|im_start|>assistant\n"
        )
        inputs = tokenizer(full_prompt, return_tensors="pt")

        start = time.perf_counter()
        with torch.no_grad():
            output = model.generate(**inputs, max_new_tokens=20,
                                    temperature=0.1, do_sample=True,
                                    pad_token_id=tokenizer.eos_token_id)
        elapsed = (time.perf_counter() - start) * 1000  # ms

        generated = tokenizer.decode(output[0][inputs["input_ids"].shape[1]:],
                                     skip_special_tokens=True)
        generated = generated.split("<|im_end|>")[0].strip()
        generated = sanitize(generated)

        latencies.append(elapsed)
        outputs.append(generated)
        if re.match(r"^[a-z][a-z0-9\-]*$", generated) and len(generated) >= 3:
            format_ok += 1

    return _report(latencies, outputs, format_ok, len(prompts))


def benchmark_gguf(gguf_path: Path, tokenizer_dir: Path, prompts: list[str]) -> dict:
    """Benchmark GGUF model via llama-cpp-python."""
    try:
        from llama_cpp import Llama
    except ImportError:
        print("\nllama-cpp-python not installed. Install with:")
        print("  pip install llama-cpp-python")
        return {}

    print(f"\n{'='*60}")
    print(f"Benchmarking GGUF model: {gguf_path}")
    print(f"File size: {gguf_path.stat().st_size / 1e6:.1f} MB")
    print(f"{'='*60}")

    llm = Llama(
        model_path=str(gguf_path),
        n_ctx=256,
        n_threads=4,
        verbose=False,
    )

    # Warmup
    print("Warming up...")
    for prompt in prompts[:3]:
        full_prompt = (
            f"<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n"
            f"<|im_start|>user\n{prompt}<|im_end|>\n"
            f"<|im_start|>assistant\n"
        )
        llm(full_prompt, max_tokens=20, temperature=0.1, stop=["<|im_end|>"])

    # Benchmark
    print(f"Running {len(prompts)} generations...")
    latencies = []
    outputs = []
    format_ok = 0

    for prompt in prompts:
        full_prompt = (
            f"<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n"
            f"<|im_start|>user\n{prompt}<|im_end|>\n"
            f"<|im_start|>assistant\n"
        )

        start = time.perf_counter()
        result = llm(full_prompt, max_tokens=20, temperature=0.1, stop=["<|im_end|>"])
        elapsed = (time.perf_counter() - start) * 1000  # ms

        generated = result["choices"][0]["text"].strip()
        generated = sanitize(generated)

        latencies.append(elapsed)
        outputs.append(generated)
        if re.match(r"^[a-z][a-z0-9\-]*$", generated) and len(generated) >= 3:
            format_ok += 1

    return _report(latencies, outputs, format_ok, len(prompts))


def _report(latencies: list[float], outputs: list[str], format_ok: int, total: int) -> dict:
    """Print and return benchmark results."""
    latencies.sort()
    p50 = latencies[len(latencies) // 2]
    p95 = latencies[int(len(latencies) * 0.95)]
    p99 = latencies[int(len(latencies) * 0.99)]
    mean = statistics.mean(latencies)
    stdev = statistics.stdev(latencies) if len(latencies) > 1 else 0

    print(f"\nLatency (ms):")
    print(f"  P50:  {p50:>7.1f}")
    print(f"  P95:  {p95:>7.1f}")
    print(f"  P99:  {p99:>7.1f}")
    print(f"  Mean: {mean:>7.1f} +/- {stdev:.1f}")
    print(f"  Min:  {min(latencies):>7.1f}")
    print(f"  Max:  {max(latencies):>7.1f}")
    print(f"\nFormat compliance: {format_ok}/{total} ({100*format_ok/total:.1f}%)")

    # Show sample outputs
    print(f"\nSample outputs (first 10):")
    for i, out in enumerate(outputs[:10]):
        print(f"  {out}")

    target_met = p95 < 100
    print(f"\nP95 < 100ms target: {'PASS' if target_met else 'FAIL'}")

    return {
        "p50_ms": p50, "p95_ms": p95, "p99_ms": p99,
        "mean_ms": mean, "stdev_ms": stdev,
        "format_compliance": format_ok / total,
        "target_met": target_met,
    }


def main():
    parser = argparse.ArgumentParser(description="Benchmark branch name generator")
    parser.add_argument("--model-dir", default="models/student", help="HF model directory")
    parser.add_argument("--gguf", default="models/branch-name-generator.gguf", help="GGUF file path")
    parser.add_argument("--count", type=int, default=50, help="Number of test prompts")
    parser.add_argument("--format", choices=["torch", "gguf", "both"], default="both")
    args = parser.parse_args()

    script_dir = Path(__file__).parent
    model_dir = script_dir / args.model_dir
    gguf_path = script_dir / args.gguf

    prompts = SAMPLE_PROMPTS[:args.count]
    if len(prompts) < args.count:
        # Repeat if we need more
        prompts = (prompts * ((args.count // len(prompts)) + 1))[:args.count]

    results = {}

    if args.format in ("torch", "both") and model_dir.exists():
        results["torch"] = benchmark_torch(model_dir, prompts)
    elif args.format == "torch":
        print(f"ERROR: Model not found at {model_dir}")

    if args.format in ("gguf", "both") and gguf_path.exists():
        results["gguf"] = benchmark_gguf(gguf_path, model_dir, prompts)
    elif args.format == "gguf":
        print(f"ERROR: GGUF not found at {gguf_path}")

    # Save results
    results_path = script_dir / "data" / "benchmark_results.json"
    results_path.parent.mkdir(exist_ok=True)
    with open(results_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to {results_path}")


if __name__ == "__main__":
    main()
