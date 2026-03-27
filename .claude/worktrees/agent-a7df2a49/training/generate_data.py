#!/usr/bin/env python3
"""
Training data generation pipeline for the branch name generator model.

Three-stage pipeline:
1. Load hand-written seeds from seeds.jsonl
2. Rule-based augmentation (~500 examples)
3. Claude API batch generation (~1,200 examples)

Output: data/branch_names.jsonl with train/val/test splits.
"""

import json
import random
import re
import hashlib
import argparse
from pathlib import Path
from typing import Optional

# ---------------------------------------------------------------------------
# Sanitization (mirrors Rust sanitize_branch_name)
# ---------------------------------------------------------------------------

def sanitize_branch_name(raw: str) -> str:
    """Sanitize a raw string into a valid git branch slug."""
    name = raw.strip().split("\n")[0]  # first line only
    name = name.strip("\"'`")
    name = name.lower()
    name = re.sub(r"[^a-z0-9\-/]", "-", name)
    name = re.sub(r"-{2,}", "-", name)
    name = name.strip("-")
    return name[:50]


def is_valid_branch_name(name: str) -> bool:
    """Check if a branch name is valid."""
    if not name or len(name) < 3:
        return False
    if len(name) > 50:
        return False
    if not re.match(r"^[a-z][a-z0-9\-]*$", name):
        return False
    if name.startswith("-") or name.endswith("-"):
        return False
    if "--" in name:
        return False
    return True


# ---------------------------------------------------------------------------
# Stage 1: Load seeds
# ---------------------------------------------------------------------------

def load_seeds(seeds_path: Path) -> list[dict]:
    """Load hand-written seed examples."""
    examples = []
    with open(seeds_path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            ex = json.loads(line)
            examples.append(ex)
    print(f"[seeds] Loaded {len(examples)} seed examples")
    return examples


# ---------------------------------------------------------------------------
# Stage 2: Rule-based augmentation
# ---------------------------------------------------------------------------

# Synonym maps for augmentation
VERB_SYNONYMS = {
    "fix": ["resolve", "repair", "patch", "correct", "address"],
    "add": ["implement", "introduce", "create", "build", "include"],
    "update": ["upgrade", "modify", "change", "revise", "adjust"],
    "remove": ["delete", "drop", "eliminate", "strip", "clean up"],
    "refactor": ["restructure", "reorganize", "simplify", "rework", "overhaul"],
    "improve": ["enhance", "optimize", "boost", "strengthen", "polish"],
    "migrate": ["move", "transfer", "port", "convert", "transition"],
    "configure": ["set up", "enable", "activate", "initialize", "wire up"],
    "document": ["write docs for", "describe", "explain", "add docs for"],
    "test": ["verify", "validate", "check", "assert", "cover"],
}

NOUN_SYNONYMS = {
    "crash": ["failure", "error", "exception", "panic", "abort"],
    "bug": ["issue", "defect", "problem", "glitch", "regression"],
    "feature": ["functionality", "capability", "behavior", "support"],
    "component": ["module", "widget", "element", "section"],
    "API": ["endpoint", "service", "interface", "route"],
    "database": ["DB", "data store", "storage", "persistence layer"],
    "authentication": ["auth", "login", "sign-in", "session management"],
    "performance": ["speed", "latency", "throughput", "responsiveness"],
    "configuration": ["config", "settings", "preferences", "options"],
    "dependency": ["package", "library", "module", "dep"],
}

PREFIX_MAP = {
    "fix": "fix",
    "resolve": "fix",
    "repair": "fix",
    "patch": "fix",
    "correct": "fix",
    "address": "fix",
    "add": "feat",
    "implement": "feat",
    "introduce": "feat",
    "create": "feat",
    "build": "feat",
    "include": "feat",
    "update": "chore",
    "upgrade": "chore",
    "modify": "feat",
    "change": "feat",
    "revise": "chore",
    "refactor": "refactor",
    "restructure": "refactor",
    "reorganize": "refactor",
    "simplify": "refactor",
    "rework": "refactor",
    "overhaul": "refactor",
    "remove": "chore",
    "delete": "chore",
    "drop": "chore",
    "eliminate": "chore",
    "document": "docs",
    "write docs for": "docs",
    "describe": "docs",
    "explain": "docs",
    "add docs for": "docs",
    "test": "test",
    "verify": "test",
    "validate": "test",
    "check": "test",
    "configure": "chore",
    "set up": "chore",
    "enable": "chore",
    "migrate": "chore",
    "move": "refactor",
    "convert": "refactor",
    "improve": "feat",
    "enhance": "feat",
    "optimize": "feat",
    "boost": "feat",
}

CONTEXT_PHRASES = [
    "for the {noun}",
    "in the {noun}",
    "on the {noun}",
    "to the {noun}",
    "when using {noun}",
    "during {noun}",
    "across {noun}",
    "within the {noun}",
]

NOUNS_FOR_CONTEXT = [
    "dashboard", "sidebar", "settings page", "terminal", "workspace",
    "file explorer", "editor", "toolbar", "status bar", "dialog",
    "notifications", "search panel", "command palette", "tab bar",
    "login screen", "profile page", "admin panel", "checkout flow",
    "main layout", "navigation menu", "context menu", "tooltip",
    "modal window", "dropdown", "data table", "form inputs",
    "header section", "footer area", "onboarding wizard", "error page",
]

TASKS_BY_PREFIX = {
    "feat": [
        "Add {thing} support",
        "Implement {thing}",
        "Build {thing} component",
        "Create {thing} functionality",
        "Introduce {thing} feature",
        "Enable {thing} mode",
        "Add ability to {action}",
        "Support {thing} in {area}",
    ],
    "fix": [
        "Fix {thing} not working",
        "Fix crash when {action}",
        "Fix {thing} causing errors",
        "Resolve {thing} failure",
        "Handle {thing} edge case",
        "Fix broken {thing}",
        "Fix {thing} regression",
        "Patch {thing} vulnerability",
    ],
    "refactor": [
        "Refactor {thing} for clarity",
        "Simplify {thing} logic",
        "Extract {thing} into module",
        "Clean up {thing} code",
        "Replace {thing} with {alternative}",
        "Decouple {thing} from {other}",
        "Modularize {thing}",
        "Reduce complexity in {thing}",
    ],
    "docs": [
        "Document {thing} usage",
        "Add docs for {thing}",
        "Write {thing} guide",
        "Update {thing} documentation",
        "Add examples for {thing}",
        "Describe {thing} architecture",
    ],
    "chore": [
        "Update {thing} to latest version",
        "Remove unused {thing}",
        "Clean up {thing} config",
        "Bump {thing} dependency",
        "Configure {thing} for production",
        "Set up {thing} automation",
    ],
    "test": [
        "Add tests for {thing}",
        "Test {thing} edge cases",
        "Add integration tests for {thing}",
        "Cover {thing} with unit tests",
        "Add regression test for {thing}",
        "Verify {thing} behavior",
    ],
    "style": [
        "Fix {thing} alignment",
        "Standardize {thing} spacing",
        "Adjust {thing} colors",
        "Fix {thing} overflow",
        "Normalize {thing} sizes",
        "Clean up {thing} styles",
    ],
}

THINGS = [
    "authentication", "authorization", "caching", "logging", "routing",
    "navigation", "pagination", "sorting", "filtering", "search",
    "notifications", "file upload", "image processing", "PDF export",
    "CSV import", "WebSocket", "SSE events", "API responses",
    "error handling", "input validation", "form submission", "data binding",
    "state management", "session handling", "token refresh", "CORS headers",
    "rate limiting", "connection pooling", "query builder", "ORM models",
    "middleware", "interceptors", "guards", "decorators", "hooks",
    "context providers", "store actions", "reducers", "selectors",
    "scroll behavior", "keyboard events", "mouse interactions", "touch gestures",
    "drag and drop", "clipboard", "undo/redo", "auto-save", "lazy loading",
    "code splitting", "tree shaking", "bundle optimization", "hot reload",
    "PWA manifest", "service worker", "offline mode", "push notifications",
    "dark mode", "responsive layout", "accessibility", "localization",
    "analytics tracking", "A/B testing", "feature flags", "canary deployment",
]

ALTERNATIVES = [
    "new approach", "modern pattern", "simpler design", "async version",
    "streaming API", "batch processing", "event-driven model", "typed interface",
]

AREAS = [
    "frontend", "backend", "API layer", "database layer", "CLI",
    "dashboard", "admin panel", "mobile view", "settings", "workspace",
]


def make_slug(description: str, prefix: str) -> str:
    """Generate a branch name slug from a description."""
    # Remove common prefixes
    desc = description.lower()
    for remove in ["add ", "implement ", "fix ", "refactor ", "update ", "create ",
                    "build ", "introduce ", "enable ", "support ", "resolve ",
                    "document ", "write ", "clean up ", "set up ", "configure ",
                    "test ", "verify ", "cover "]:
        if desc.startswith(remove):
            desc = desc[len(remove):]
            break

    # Slugify
    slug = re.sub(r"[^a-z0-9]+", "-", desc)
    slug = slug.strip("-")

    # Limit length
    parts = slug.split("-")
    result = prefix
    for part in parts:
        candidate = result + "-" + part
        if len(candidate) > 45:
            break
        result = candidate

    return result


def augment_from_templates(count: int = 500) -> list[dict]:
    """Generate examples from templates with random substitution."""
    examples = []
    seen = set()

    for _ in range(count * 3):  # oversample to hit target after dedup
        if len(examples) >= count:
            break

        prefix = random.choice(list(TASKS_BY_PREFIX.keys()))
        template = random.choice(TASKS_BY_PREFIX[prefix])

        thing = random.choice(THINGS)
        alternative = random.choice(ALTERNATIVES)
        other = random.choice(THINGS)
        area = random.choice(AREAS)
        action = random.choice([
            f"using {thing}", f"opening {thing}", f"saving {thing}",
            f"loading {thing}", f"switching {thing}", f"closing {thing}",
        ])

        description = template.format(
            thing=thing, alternative=alternative, other=other,
            area=area, action=action,
        )

        slug = make_slug(description, prefix)

        if not is_valid_branch_name(slug):
            continue

        key = slug
        if key in seen:
            continue
        seen.add(key)

        examples.append({"input": description, "output": slug})

    print(f"[augment] Generated {len(examples)} augmented examples")
    return examples


def augment_seed_variants(seeds: list[dict], count: int = 100) -> list[dict]:
    """Create variants of seeds by prepending/appending context."""
    examples = []
    seen = {ex["output"] for ex in seeds}

    for _ in range(count * 3):
        if len(examples) >= count:
            break

        seed = random.choice(seeds)
        variant_type = random.choice(["prepend_please", "append_context", "rephrase_verb"])

        inp = seed["input"]
        out = seed["output"]

        if variant_type == "prepend_please":
            prefixes = ["Please ", "Can you ", "We need to ", "I want to ", "Let's "]
            inp = random.choice(prefixes) + inp[0].lower() + inp[1:]
            # output stays the same

        elif variant_type == "append_context":
            noun = random.choice(NOUNS_FOR_CONTEXT)
            phrase = random.choice(CONTEXT_PHRASES).format(noun=noun)
            inp = inp.rstrip(".") + " " + phrase

            # Extend slug if room
            noun_slug = re.sub(r"[^a-z0-9]+", "-", noun.lower()).strip("-")
            candidate = out + "-" + noun_slug
            if len(candidate) <= 50 and is_valid_branch_name(candidate):
                out = candidate

        elif variant_type == "rephrase_verb":
            # Try swapping the first verb with a synonym
            first_word = inp.split()[0].lower().rstrip(".,!?")
            if first_word in VERB_SYNONYMS:
                new_verb = random.choice(VERB_SYNONYMS[first_word])
                inp = new_verb.capitalize() + inp[len(first_word):]
                # Adjust prefix in output if applicable
                if first_word in PREFIX_MAP:
                    new_prefix = PREFIX_MAP.get(new_verb.split()[0], PREFIX_MAP.get(first_word, ""))
                    old_prefix = PREFIX_MAP.get(first_word, "")
                    if old_prefix and new_prefix and out.startswith(old_prefix):
                        out = new_prefix + out[len(old_prefix):]

        if not is_valid_branch_name(out):
            continue

        if out in seen:
            continue
        seen.add(out)

        examples.append({"input": inp, "output": out})

    print(f"[seed-variants] Generated {len(examples)} seed variant examples")
    return examples


# ---------------------------------------------------------------------------
# Stage 3: Claude API generation
# ---------------------------------------------------------------------------

LLM_SYSTEM_PROMPT = """You are a training data generator for a git branch name model.

Given a category, generate 20 diverse (description, branch_name) pairs.

Rules for branch names:
- Start with a prefix: feat-, fix-, refactor-, docs-, chore-, test-, style-
- Use only lowercase letters, numbers, and hyphens
- Be descriptive but concise (3-6 words after prefix)
- Max 50 characters total
- No double hyphens, no leading/trailing hyphens

Output format (JSON array):
[{"input": "description", "output": "branch-name"}, ...]

Be creative and realistic. Use real-world software engineering scenarios.
Vary the description length and complexity (some short, some detailed).
Some descriptions should be informal ("fix that weird scrolling bug") and some formal ("Resolve scroll position regression in virtualized lists")."""

CATEGORIES = [
    "Frontend UI bugs (React, Vue, DOM manipulation, CSS issues)",
    "Backend API development (REST, GraphQL, authentication, middleware)",
    "Database operations (migrations, queries, indexes, ORMs)",
    "DevOps and CI/CD (Docker, Kubernetes, GitHub Actions, deployments)",
    "Performance optimization (caching, lazy loading, bundle size, query speed)",
    "Security fixes (XSS, CSRF, SQL injection, auth vulnerabilities)",
    "Mobile development (responsive design, touch interactions, PWA)",
    "Testing infrastructure (unit tests, E2E tests, CI test runners)",
    "Developer experience (tooling, linting, formatting, IDE support)",
    "Accessibility improvements (ARIA, screen readers, keyboard nav, contrast)",
    "Rust systems programming (memory safety, concurrency, FFI, async runtime)",
    "Terminal and CLI applications (TUI, ANSI parsing, PTY, shell integration)",
    "Real-time features (WebSocket, SSE, pub/sub, live updates)",
    "Data processing (ETL pipelines, CSV/JSON parsing, data validation)",
    "Documentation and developer onboarding (guides, examples, API docs)",
    "File system operations (watching, syncing, compression, encoding)",
    "Networking (HTTP client, DNS, proxy, TLS, connection pooling)",
    "State management (Redux, Zustand, signals, reactive stores)",
    "Build system and bundling (Webpack, Vite, esbuild, tree shaking)",
    "Monitoring and observability (logging, metrics, tracing, alerting)",
    "Plugin and extension systems (hooks, middleware, module loading)",
    "Search functionality (full-text search, fuzzy matching, indexing)",
    "Notification systems (email, push, in-app, webhooks)",
    "Configuration management (env vars, feature flags, dynamic config)",
    "Error handling and recovery (retry logic, circuit breakers, fallbacks)",
    "Cross-platform compatibility (Windows, macOS, Linux, WSL)",
    "Internationalization (i18n, locale, RTL, date/number formatting)",
    "Code generation and scaffolding (templates, CLI generators, macros)",
    "Graph and tree data structures (DAG traversal, AST, dependency resolution)",
    "AI/ML integration (model loading, inference, embeddings, fine-tuning)",
    "Payment and billing (Stripe, invoicing, subscriptions, tax calculation)",
    "User management (roles, permissions, teams, invitations)",
    "Media handling (image resize, video transcode, audio processing)",
    "Caching strategies (Redis, memcached, browser cache, CDN invalidation)",
    "API versioning and backward compatibility (deprecation, migration paths)",
    "Workflow automation (task queues, cron jobs, event triggers)",
    "Code review tooling (PR templates, auto-review, merge strategies)",
    "Secrets management (vault, env encryption, key rotation)",
    "Microservices communication (gRPC, message queues, service mesh)",
    "Desktop application features (window management, system tray, shortcuts)",
    "Git operations (hooks, worktrees, merge strategies, rebasing)",
    "Package management (npm, cargo, pip, dependency resolution)",
    "Keyboard shortcuts and input handling (hotkeys, key combos, focus management)",
    "Scrollback and history (buffer management, search in history, pagination)",
    "Canvas and rendering (2D drawing, WebGL, SVG, animation frames)",
    "Process management (spawn, kill, signals, IPC, daemon lifecycle)",
    "Serialization formats (JSON, MessagePack, protobuf, CBOR)",
    "Memory management (allocation, pooling, leak detection, profiling)",
    "Concurrency patterns (locks, channels, atomics, work stealing)",
    "Clipboard and drag-drop (copy/paste, file drops, MIME types)",
    "Theme system (dark/light mode, custom themes, CSS variables)",
    "Tab and workspace management (split views, layouts, session restore)",
    "Font rendering (ligatures, emoji, CJK, variable fonts, metrics)",
    "Protocol implementation (SSH, FTP, SMTP, custom binary protocols)",
    "Sandbox and isolation (containers, WASM, iframes, process separation)",
    "Backup and restore (snapshots, incremental backup, disaster recovery)",
    "Rate limiting and throttling (token bucket, sliding window, per-user limits)",
    "URL routing (path params, query strings, redirects, deep linking)",
    "Form handling (validation, multi-step, file upload, autofill)",
    "Chart and data visualization (bar charts, line graphs, heatmaps, tooltips)",
]


def generate_with_openai(api_key: str, target_count: int = 1200) -> list[dict]:
    """Generate examples using OpenAI API."""
    try:
        from openai import OpenAI
    except ImportError:
        print("[openai] openai package not installed, skipping API generation")
        print("  Install with: pip install openai")
        return []

    client = OpenAI(api_key=api_key)
    examples = []
    seen_outputs = set()
    batches_needed = (target_count // 20) + 1

    # Cycle through categories
    for i in range(batches_needed):
        if len(examples) >= target_count:
            break

        category = CATEGORIES[i % len(CATEGORIES)]

        try:
            response = client.chat.completions.create(
                model="gpt-4o-mini",
                max_tokens=2000,
                messages=[
                    {"role": "system", "content": LLM_SYSTEM_PROMPT},
                    {"role": "user", "content": f"Generate 20 (description, branch_name) pairs for: {category}"},
                ],
            )

            text = response.choices[0].message.content

            # Parse JSON from response (handle markdown code blocks)
            json_match = re.search(r"\[[\s\S]*\]", text)
            if not json_match:
                print(f"  [openai] Batch {i+1}: No JSON found, skipping")
                continue

            batch = json.loads(json_match.group())

            added = 0
            for item in batch:
                inp = item.get("input", "")
                out = item.get("output", "")

                # Sanitize and validate
                out = sanitize_branch_name(out)
                if not is_valid_branch_name(out):
                    continue
                if out in seen_outputs:
                    continue
                if not inp or len(inp) < 5:
                    continue

                seen_outputs.add(out)
                examples.append({"input": inp, "output": out})
                added += 1

            print(f"  [openai] Batch {i+1}/{batches_needed}: +{added} examples ({len(examples)} total)")

        except Exception as e:
            print(f"  [openai] Batch {i+1} failed: {e}")
            continue

    print(f"[openai] Generated {len(examples)} examples via API")
    return examples


# ---------------------------------------------------------------------------
# Deduplication and splitting
# ---------------------------------------------------------------------------

def dedup_examples(examples: list[dict]) -> list[dict]:
    """Remove duplicates by output slug."""
    seen = set()
    unique = []
    for ex in examples:
        key = ex["output"]
        if key not in seen:
            seen.add(key)
            unique.append(ex)
    removed = len(examples) - len(unique)
    print(f"[dedup] Removed {removed} duplicates, {len(unique)} remaining")
    return unique


def split_dataset(examples: list[dict], train_ratio: float = 0.8,
                  val_ratio: float = 0.1) -> tuple[list, list, list]:
    """Split into train/val/test with deterministic shuffling."""
    # Sort by hash for deterministic shuffle
    examples_sorted = sorted(examples, key=lambda x: hashlib.md5(
        x["output"].encode()).hexdigest())

    n = len(examples_sorted)
    train_end = int(n * train_ratio)
    val_end = int(n * (train_ratio + val_ratio))

    train = examples_sorted[:train_end]
    val = examples_sorted[train_end:val_end]
    test = examples_sorted[val_end:]

    print(f"[split] Train: {len(train)}, Val: {len(val)}, Test: {len(test)}")
    return train, val, test


def save_jsonl(examples: list[dict], path: Path):
    """Save examples to JSONL file."""
    with open(path, "w") as f:
        for ex in examples:
            f.write(json.dumps(ex) + "\n")
    print(f"[save] Wrote {len(examples)} examples to {path}")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Generate branch name training data")
    parser.add_argument("--seeds", default="seeds.jsonl", help="Path to seeds file")
    parser.add_argument("--output-dir", default="data", help="Output directory")
    parser.add_argument("--api-key", default=None, help="OpenAI API key (or set OPENAI_API_KEY)")
    parser.add_argument("--skip-api", action="store_true", help="Skip OpenAI API generation")
    parser.add_argument("--augment-count", type=int, default=500, help="Template augmentation count")
    parser.add_argument("--api-count", type=int, default=1200, help="OpenAI API generation count")
    args = parser.parse_args()

    script_dir = Path(__file__).parent
    seeds_path = script_dir / args.seeds
    output_dir = script_dir / args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)

    # Stage 1: Seeds
    seeds = load_seeds(seeds_path)

    # Stage 2: Augmentation
    augmented = augment_from_templates(args.augment_count)
    seed_variants = augment_seed_variants(seeds, count=100)

    # Stage 3: OpenAI API
    api_examples = []
    if not args.skip_api:
        import os
        api_key = args.api_key or os.environ.get("OPENAI_API_KEY")
        if api_key:
            api_examples = generate_with_openai(api_key, args.api_count)
        else:
            print("[openai] No API key provided, skipping. Use --api-key or OPENAI_API_KEY env var")

    # Combine and dedup
    all_examples = seeds + seed_variants + augmented + api_examples
    all_examples = dedup_examples(all_examples)

    # Validate all examples
    valid = []
    invalid = 0
    for ex in all_examples:
        sanitized = sanitize_branch_name(ex["output"])
        if is_valid_branch_name(sanitized):
            ex["output"] = sanitized
            valid.append(ex)
        else:
            invalid += 1
    print(f"[validate] {len(valid)} valid, {invalid} invalid (removed)")

    # Split
    train, val, test = split_dataset(valid)

    # Save
    save_jsonl(valid, output_dir / "branch_names.jsonl")
    save_jsonl(train, output_dir / "train.jsonl")
    save_jsonl(val, output_dir / "val.jsonl")
    save_jsonl(test, output_dir / "test.jsonl")

    # Summary
    print(f"\n{'='*50}")
    print(f"Total examples: {len(valid)}")
    print(f"  Seeds: {len(seeds)}")
    print(f"  Seed variants: {len(seed_variants)}")
    print(f"  Template augmented: {len(augmented)}")
    print(f"  OpenAI API: {len(api_examples)}")
    print(f"Splits: train={len(train)}, val={len(val)}, test={len(test)}")
    print(f"Output: {output_dir}")


if __name__ == "__main__":
    main()
