#!/usr/bin/env python3
"""
Phase 3: Knowledge distillation from teacher (135M) to student (~20M).

Student architecture: 4-layer LlamaForCausalLM with 256 hidden dim.
Loss: 0.7 * KL(student, teacher, T=2.0) + 0.3 * CE(student, labels)

Usage:
    python distill.py [--teacher-dir models/teacher-merged] [--output-dir models/student]
"""

import argparse
import json
import re
from pathlib import Path

import torch
import torch.nn as nn
import torch.nn.functional as F
from datasets import Dataset
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
    LlamaConfig,
    LlamaForCausalLM,
    Trainer,
    TrainingArguments,
)


# Student architecture (same LlamaForCausalLM family, smaller)
STUDENT_CONFIG = {
    "num_hidden_layers": 4,
    "hidden_size": 256,
    "num_attention_heads": 4,
    "num_key_value_heads": 4,
    "intermediate_size": 1024,
    "max_position_embeddings": 256,
    "rms_norm_eps": 1e-5,
    "rope_theta": 10000.0,
    "tie_word_embeddings": True,
}

# Distillation hyperparameters
ALPHA_KL = 0.7       # KL divergence weight
ALPHA_CE = 0.3       # Cross-entropy weight
TEMPERATURE = 2.0    # Softmax temperature for distillation

SYSTEM_PROMPT = (
    "You generate concise git branch names from descriptions. "
    "Output only the branch name slug (e.g. fix-crash-on-resize). "
    "Use prefixes: feat-, fix-, refactor-, docs-, chore-, test-, style-. "
    "Use only lowercase, numbers, hyphens. Max 50 chars."
)


def format_example(example: dict) -> str:
    """Format as ChatML for tokenization."""
    return (
        f"<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n"
        f"<|im_start|>user\n{example['input']}<|im_end|>\n"
        f"<|im_start|>assistant\n{example['output']}<|im_end|>"
    )


def load_data(data_dir: Path, tokenizer, max_length: int = 256) -> tuple[Dataset, Dataset]:
    """Load and tokenize train/val splits."""
    def load_jsonl(path: Path) -> list[dict]:
        examples = []
        with open(path) as f:
            for line in f:
                if line.strip():
                    examples.append(json.loads(line))
        return examples

    def tokenize_examples(examples: list[dict]) -> dict:
        texts = [format_example(ex) for ex in examples]
        encodings = tokenizer(
            texts,
            padding="max_length",
            truncation=True,
            max_length=max_length,
            return_tensors="pt",
        )
        # Labels = input_ids (for causal LM), with padding tokens set to -100
        labels = encodings["input_ids"].clone()
        labels[labels == tokenizer.pad_token_id] = -100
        return {
            "input_ids": encodings["input_ids"],
            "attention_mask": encodings["attention_mask"],
            "labels": labels,
        }

    train_examples = load_jsonl(data_dir / "train.jsonl")
    val_examples = load_jsonl(data_dir / "val.jsonl")

    train_tok = tokenize_examples(train_examples)
    val_tok = tokenize_examples(val_examples)

    train_ds = Dataset.from_dict({
        "input_ids": train_tok["input_ids"].tolist(),
        "attention_mask": train_tok["attention_mask"].tolist(),
        "labels": train_tok["labels"].tolist(),
    })
    val_ds = Dataset.from_dict({
        "input_ids": val_tok["input_ids"].tolist(),
        "attention_mask": val_tok["attention_mask"].tolist(),
        "labels": val_tok["labels"].tolist(),
    })

    train_ds.set_format("torch")
    val_ds.set_format("torch")

    return train_ds, val_ds


class DistillationTrainer(Trainer):
    """Custom Trainer that computes distillation loss using teacher logits."""

    def __init__(self, teacher_model=None, alpha_kl=ALPHA_KL, alpha_ce=ALPHA_CE,
                 temperature=TEMPERATURE, **kwargs):
        super().__init__(**kwargs)
        self.teacher = teacher_model
        self.alpha_kl = alpha_kl
        self.alpha_ce = alpha_ce
        self.temperature = temperature

        if self.teacher is not None:
            self.teacher.eval()
            for param in self.teacher.parameters():
                param.requires_grad = False

    def compute_loss(self, model, inputs, return_outputs=False, **kwargs):
        labels = inputs.pop("labels")
        student_outputs = model(**inputs)
        student_logits = student_outputs.logits

        # Standard cross-entropy loss
        shift_logits = student_logits[..., :-1, :].contiguous()
        shift_labels = labels[..., 1:].contiguous()
        ce_loss = F.cross_entropy(
            shift_logits.view(-1, shift_logits.size(-1)),
            shift_labels.view(-1),
            ignore_index=-100,
        )

        # KL divergence loss with teacher
        if self.teacher is not None:
            with torch.no_grad():
                teacher_outputs = self.teacher(**inputs)
                teacher_logits = teacher_outputs.logits

            # Soft targets with temperature scaling
            teacher_probs = F.softmax(teacher_logits / self.temperature, dim=-1)
            student_log_probs = F.log_softmax(student_logits / self.temperature, dim=-1)

            # KL divergence (only on non-padding positions)
            mask = (labels != -100).unsqueeze(-1).float()
            kl_loss = F.kl_div(
                student_log_probs * mask,
                teacher_probs * mask,
                reduction="batchmean",
            ) * (self.temperature ** 2)

            loss = self.alpha_kl * kl_loss + self.alpha_ce * ce_loss
        else:
            loss = ce_loss

        return (loss, student_outputs) if return_outputs else loss


def create_student_model(tokenizer, device: str) -> LlamaForCausalLM:
    """Create a tiny LlamaForCausalLM student model."""
    config = LlamaConfig(
        vocab_size=tokenizer.vocab_size,
        **STUDENT_CONFIG,
    )
    model = LlamaForCausalLM(config)

    param_count = sum(p.numel() for p in model.parameters())
    print(f"Student model: {param_count / 1e6:.1f}M parameters")
    print(f"  Layers: {config.num_hidden_layers}")
    print(f"  Hidden: {config.hidden_size}")
    print(f"  Heads: {config.num_attention_heads}")
    print(f"  Intermediate: {config.intermediate_size}")

    return model.to(device)


def main():
    parser = argparse.ArgumentParser(description="Distill teacher to tiny student")
    parser.add_argument("--teacher-dir", default="models/teacher-merged", help="Teacher model directory")
    parser.add_argument("--data-dir", default="data", help="Training data directory")
    parser.add_argument("--output-dir", default="models/student", help="Student output directory")
    parser.add_argument("--epochs", type=int, default=15, help="Number of training epochs")
    parser.add_argument("--batch-size", type=int, default=16, help="Per-device batch size")
    parser.add_argument("--lr", type=float, default=5e-4, help="Learning rate")
    parser.add_argument("--alpha-kl", type=float, default=ALPHA_KL, help="KL divergence weight")
    parser.add_argument("--alpha-ce", type=float, default=ALPHA_CE, help="Cross-entropy weight")
    parser.add_argument("--temperature", type=float, default=TEMPERATURE, help="Distillation temperature")
    parser.add_argument("--student-layers", type=int, default=4, help="Number of student layers")
    parser.add_argument("--student-hidden", type=int, default=256, help="Student hidden dim")
    parser.add_argument("--max-seq-length", type=int, default=256, help="Max sequence length")
    args = parser.parse_args()

    script_dir = Path(__file__).parent
    data_dir = script_dir / args.data_dir
    teacher_dir = script_dir / args.teacher_dir
    output_dir = script_dir / args.output_dir

    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"Device: {device}")

    # Update student config from args
    STUDENT_CONFIG["num_hidden_layers"] = args.student_layers
    STUDENT_CONFIG["hidden_size"] = args.student_hidden

    # Load tokenizer (shared with teacher)
    print(f"\nLoading tokenizer from {teacher_dir}...")
    tokenizer = AutoTokenizer.from_pretrained(str(teacher_dir))
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    # Load data
    print(f"\nLoading data from {data_dir}...")
    train_dataset, val_dataset = load_data(data_dir, tokenizer, args.max_seq_length)
    print(f"Train: {len(train_dataset)}, Val: {len(val_dataset)}")

    # Load teacher
    print(f"\nLoading teacher model from {teacher_dir}...")
    teacher = AutoModelForCausalLM.from_pretrained(
        str(teacher_dir),
        torch_dtype=torch.float16 if device == "cuda" else torch.float32,
        device_map="auto" if device == "cuda" else None,
    )
    teacher.eval()

    # Create student
    print("\nCreating student model...")
    student = create_student_model(tokenizer, device)

    # Training config
    training_args = TrainingArguments(
        output_dir=str(output_dir / "checkpoints"),
        num_train_epochs=args.epochs,
        per_device_train_batch_size=args.batch_size,
        per_device_eval_batch_size=args.batch_size,
        gradient_accumulation_steps=1,
        learning_rate=args.lr,
        lr_scheduler_type="cosine",
        warmup_ratio=0.1,
        weight_decay=0.01,
        fp16=device == "cuda",
        logging_steps=10,
        eval_strategy="epoch",
        save_strategy="epoch",
        save_total_limit=2,
        load_best_model_at_end=True,
        metric_for_best_model="eval_loss",
        greater_is_better=False,
        report_to="none",
        remove_unused_columns=False,
    )

    # Distillation trainer
    trainer = DistillationTrainer(
        teacher_model=teacher,
        alpha_kl=args.alpha_kl,
        alpha_ce=args.alpha_ce,
        temperature=args.temperature,
        model=student,
        args=training_args,
        train_dataset=train_dataset,
        eval_dataset=val_dataset,
    )

    # Train
    print("\nStarting distillation...")
    trainer.train()

    # Save student
    print(f"\nSaving student model to {output_dir}...")
    output_dir.mkdir(parents=True, exist_ok=True)
    student.save_pretrained(str(output_dir))
    tokenizer.save_pretrained(str(output_dir))

    # Evaluate
    print("\nEvaluating student...")
    evaluate_model(student, tokenizer, data_dir / "test.jsonl", device)

    # Also evaluate teacher for comparison
    print("\nEvaluating teacher (for comparison)...")
    evaluate_model(teacher, tokenizer, data_dir / "test.jsonl", device)

    print(f"\nDone! Student model saved to: {output_dir}")


def evaluate_model(model, tokenizer, test_path: Path, device: str):
    """Evaluate exact match and format compliance on test split."""
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
    total = min(len(examples), 100)

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
