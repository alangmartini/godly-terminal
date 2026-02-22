#!/usr/bin/env python3
"""
Phase 2: Fine-tune SmolLM2-135M-Instruct as the teacher model.

Uses LoRA (r=16, alpha=32) with SFTTrainer for efficient fine-tuning.
Outputs a merged model ready for inference and distillation.

Usage:
    python finetune_teacher.py [--data-dir data] [--output-dir models/teacher-merged]
"""

import argparse
import json
from pathlib import Path

import torch
from datasets import Dataset
from peft import LoraConfig, get_peft_model
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
    TrainingArguments,
)
from trl import SFTTrainer, SFTConfig


BASE_MODEL = "HuggingFaceTB/SmolLM2-135M-Instruct"

# Training prompt format (ChatML, matching SmolLM2-Instruct)
SYSTEM_PROMPT = (
    "You generate concise git branch names from descriptions. "
    "Output only the branch name slug (e.g. fix-crash-on-resize). "
    "Use prefixes: feat-, fix-, refactor-, docs-, chore-, test-, style-. "
    "Use only lowercase, numbers, hyphens. Max 50 chars."
)


def format_example(example: dict) -> str:
    """Format a single example as a ChatML conversation."""
    return (
        f"<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n"
        f"<|im_start|>user\n{example['input']}<|im_end|>\n"
        f"<|im_start|>assistant\n{example['output']}<|im_end|>"
    )


def load_data(data_dir: Path) -> tuple[Dataset, Dataset]:
    """Load train and validation splits."""
    def load_jsonl(path: Path) -> list[dict]:
        examples = []
        with open(path) as f:
            for line in f:
                if line.strip():
                    examples.append(json.loads(line))
        return examples

    train_examples = load_jsonl(data_dir / "train.jsonl")
    val_examples = load_jsonl(data_dir / "val.jsonl")

    # Format as text for SFTTrainer
    train_texts = [{"text": format_example(ex)} for ex in train_examples]
    val_texts = [{"text": format_example(ex)} for ex in val_examples]

    return Dataset.from_list(train_texts), Dataset.from_list(val_texts)


def main():
    parser = argparse.ArgumentParser(description="Fine-tune SmolLM2-135M teacher")
    parser.add_argument("--data-dir", default="data", help="Training data directory")
    parser.add_argument("--output-dir", default="models/teacher-merged", help="Output directory")
    parser.add_argument("--epochs", type=int, default=5, help="Number of training epochs")
    parser.add_argument("--batch-size", type=int, default=8, help="Per-device batch size")
    parser.add_argument("--lr", type=float, default=2e-4, help="Learning rate")
    parser.add_argument("--lora-r", type=int, default=16, help="LoRA rank")
    parser.add_argument("--lora-alpha", type=int, default=32, help="LoRA alpha")
    parser.add_argument("--max-seq-length", type=int, default=256, help="Max sequence length")
    args = parser.parse_args()

    script_dir = Path(__file__).parent
    data_dir = script_dir / args.data_dir
    output_dir = script_dir / args.output_dir

    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"Device: {device}")
    if device == "cuda":
        print(f"GPU: {torch.cuda.get_device_name(0)}")
        print(f"VRAM: {torch.cuda.get_device_properties(0).total_mem / 1e9:.1f} GB")

    # Load data
    print(f"\nLoading data from {data_dir}...")
    train_dataset, val_dataset = load_data(data_dir)
    print(f"Train: {len(train_dataset)}, Val: {len(val_dataset)}")

    # Load model and tokenizer
    print(f"\nLoading {BASE_MODEL}...")
    tokenizer = AutoTokenizer.from_pretrained(BASE_MODEL)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    model = AutoModelForCausalLM.from_pretrained(
        BASE_MODEL,
        torch_dtype=torch.float16 if device == "cuda" else torch.float32,
        device_map="auto" if device == "cuda" else None,
    )

    # Apply LoRA
    print(f"\nApplying LoRA (r={args.lora_r}, alpha={args.lora_alpha})...")
    lora_config = LoraConfig(
        r=args.lora_r,
        lora_alpha=args.lora_alpha,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj"],
        lora_dropout=0.05,
        bias="none",
        task_type="CAUSAL_LM",
    )
    model = get_peft_model(model, lora_config)
    model.print_trainable_parameters()

    # Training config
    lora_output = script_dir / "models" / "teacher-lora"
    training_args = SFTConfig(
        output_dir=str(lora_output),
        num_train_epochs=args.epochs,
        per_device_train_batch_size=args.batch_size,
        per_device_eval_batch_size=args.batch_size,
        gradient_accumulation_steps=2,
        learning_rate=args.lr,
        lr_scheduler_type="cosine",
        warmup_ratio=0.1,
        weight_decay=0.01,
        fp16=device == "cuda",
        bf16=False,
        use_cpu=device == "cpu",
        logging_steps=10,
        eval_strategy="epoch",
        save_strategy="epoch",
        save_total_limit=2,
        load_best_model_at_end=True,
        metric_for_best_model="eval_loss",
        greater_is_better=False,
        max_length=args.max_seq_length,
        dataset_text_field="text",
        report_to="none",
    )

    # Train
    print("\nStarting training...")
    trainer = SFTTrainer(
        model=model,
        args=training_args,
        train_dataset=train_dataset,
        eval_dataset=val_dataset,
        processing_class=tokenizer,
    )
    trainer.train()

    # Save LoRA adapter
    print(f"\nSaving LoRA adapter to {lora_output}...")
    trainer.save_model(str(lora_output))

    # Merge and save full model
    print(f"\nMerging LoRA weights and saving to {output_dir}...")
    merged_model = model.merge_and_unload()
    output_dir.mkdir(parents=True, exist_ok=True)
    merged_model.save_pretrained(str(output_dir))
    tokenizer.save_pretrained(str(output_dir))

    # Quick eval
    print("\nRunning quick evaluation...")
    evaluate_teacher(merged_model, tokenizer, data_dir / "test.jsonl", device)

    print(f"\nDone! Merged teacher model saved to: {output_dir}")


def evaluate_teacher(model, tokenizer, test_path: Path, device: str):
    """Quick evaluation on test split."""
    import re

    def sanitize(name: str) -> str:
        name = name.strip().split("\n")[0].strip("\"'`").lower()
        name = re.sub(r"[^a-z0-9\-]", "-", name)
        name = re.sub(r"-{2,}", "-", name).strip("-")
        return name[:50]

    examples = []
    with open(test_path) as f:
        for line in f:
            if line.strip():
                examples.append(json.loads(line))

    exact_match = 0
    format_ok = 0
    total = min(len(examples), 100)  # eval on up to 100

    model.eval()
    for ex in examples[:total]:
        prompt = (
            f"<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n"
            f"<|im_start|>user\n{ex['input']}<|im_end|>\n"
            f"<|im_start|>assistant\n"
        )
        inputs = tokenizer(prompt, return_tensors="pt").to(device)

        with torch.no_grad():
            outputs = model.generate(
                **inputs,
                max_new_tokens=30,
                temperature=0.1,
                do_sample=True,
                pad_token_id=tokenizer.eos_token_id,
            )

        generated = tokenizer.decode(outputs[0][inputs["input_ids"].shape[1]:],
                                     skip_special_tokens=True)
        # Strip any trailing chat markers
        generated = generated.split("<|im_end|>")[0].strip()
        generated_sanitized = sanitize(generated)

        if generated_sanitized == ex["output"]:
            exact_match += 1
        if re.match(r"^[a-z][a-z0-9\-]*$", generated_sanitized) and len(generated_sanitized) >= 3:
            format_ok += 1

    print(f"  Exact match: {exact_match}/{total} ({100*exact_match/total:.1f}%)")
    print(f"  Format compliance: {format_ok}/{total} ({100*format_ok/total:.1f}%)")


if __name__ == "__main__":
    main()
