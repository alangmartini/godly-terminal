#!/usr/bin/env python3
"""
Phase 4: Export student model to GGUF format for candle inference.

Converts the HuggingFace student model to GGUF with Q4_K_M quantization.
Requires llama.cpp's convert script (auto-cloned if not present).

Usage:
    python export_gguf.py [--student-dir models/student] [--output branch-name-generator.gguf]

If llama.cpp is not available, falls back to ONNX export.
"""

import argparse
import os
import subprocess
import sys
from pathlib import Path


def check_llama_cpp(script_dir: Path) -> Path:
    """Find or clone llama.cpp for the convert script."""
    llama_dir = script_dir / "llama.cpp"
    if llama_dir.exists():
        return llama_dir

    print("llama.cpp not found. Cloning (shallow)...")
    subprocess.run(
        ["git", "clone", "--depth", "1", "https://github.com/ggerganov/llama.cpp.git",
         str(llama_dir)],
        check=True,
    )
    return llama_dir


def export_gguf(student_dir: Path, output_path: Path, script_dir: Path):
    """Export model to GGUF format."""
    llama_dir = check_llama_cpp(script_dir)
    convert_script = llama_dir / "convert_hf_to_gguf.py"

    if not convert_script.exists():
        # Try alternative name
        convert_script = llama_dir / "convert-hf-to-gguf.py"
    if not convert_script.exists():
        print("ERROR: Cannot find convert_hf_to_gguf.py in llama.cpp")
        print("Try: pip install llama-cpp-python")
        sys.exit(1)

    # Install llama.cpp Python requirements
    req_file = llama_dir / "requirements.txt"
    if req_file.exists():
        subprocess.run(
            [sys.executable, "-m", "pip", "install", "-r", str(req_file), "-q"],
            check=False,
        )

    # Step 1: Convert to F16 GGUF
    f16_path = output_path.with_suffix(".f16.gguf")
    print(f"\nConverting to F16 GGUF: {f16_path}")
    subprocess.run(
        [sys.executable, str(convert_script), str(student_dir),
         "--outfile", str(f16_path), "--outtype", "f16"],
        check=True,
    )

    # Step 2: Quantize to Q4_K_M
    # Check multiple possible locations (Linux vs Windows MSVC layout)
    quantize_candidates = [
        llama_dir / "build" / "bin" / "llama-quantize",
        llama_dir / "build" / "bin" / "llama-quantize.exe",
        llama_dir / "build" / "bin" / "Release" / "llama-quantize.exe",
        llama_dir / "build" / "bin" / "Release" / "llama-quantize",
    ]
    quantize_bin = next((p for p in quantize_candidates if p.exists()), None)
    if quantize_bin is None:
        # Try building
        print("\nBuilding llama.cpp quantize tool...")
        build_dir = llama_dir / "build"
        build_dir.mkdir(exist_ok=True)
        subprocess.run(["cmake", "..", "-DCMAKE_BUILD_TYPE=Release"],
                       cwd=str(build_dir), check=True)
        subprocess.run(["cmake", "--build", ".", "--config", "Release", "-j"],
                       cwd=str(build_dir), check=True)

    # Re-check after build
    if quantize_bin is None:
        quantize_bin = next((p for p in quantize_candidates if p.exists()), None)

    if quantize_bin is not None:
        print(f"\nQuantizing to Q4_K_M: {output_path}")
        if output_path.exists():
            output_path.unlink()
        subprocess.run(
            [str(quantize_bin), str(f16_path), str(output_path), "Q4_K_M"],
            check=True,
        )
        # Clean up F16 intermediate
        f16_path.unlink()
        print(f"\nQuantized model: {output_path} ({output_path.stat().st_size / 1e6:.1f} MB)")
    else:
        print("\nWARNING: Cannot find llama-quantize binary. Keeping F16 GGUF.")
        print("To quantize manually:")
        print(f"  llama-quantize {f16_path} {output_path} Q4_K_M")
        # Rename F16 as output (replace on Windows requires unlink first)
        if output_path.exists():
            output_path.unlink()
        f16_path.rename(output_path)


def export_onnx(student_dir: Path, output_path: Path):
    """Fallback: Export to ONNX format."""
    try:
        from optimum.exporters.onnx import main_export
    except ImportError:
        print("Installing optimum for ONNX export...")
        subprocess.run(
            [sys.executable, "-m", "pip", "install", "optimum[onnxruntime]", "-q"],
            check=True,
        )
        from optimum.exporters.onnx import main_export

    onnx_dir = output_path.with_suffix("")
    print(f"\nExporting to ONNX: {onnx_dir}")
    main_export(
        model_name_or_path=str(student_dir),
        output=str(onnx_dir),
        task="text-generation",
    )

    # Quantize INT8
    try:
        from optimum.onnxruntime import ORTQuantizer
        from optimum.onnxruntime.configuration import AutoQuantizationConfig

        print("\nQuantizing ONNX to INT8...")
        quantizer = ORTQuantizer.from_pretrained(str(onnx_dir))
        qconfig = AutoQuantizationConfig.avx2(is_static=False)
        quantizer.quantize(save_dir=str(onnx_dir) + "-int8", quantization_config=qconfig)
        print(f"Quantized ONNX: {onnx_dir}-int8")
    except Exception as e:
        print(f"ONNX quantization failed: {e}")
        print(f"Unquantized ONNX model at: {onnx_dir}")


def main():
    parser = argparse.ArgumentParser(description="Export student model to GGUF/ONNX")
    parser.add_argument("--student-dir", default="models/student", help="Student model directory")
    parser.add_argument("--output", default="models/branch-name-generator.gguf",
                        help="Output GGUF file path")
    parser.add_argument("--format", choices=["gguf", "onnx", "both"], default="gguf",
                        help="Export format")
    args = parser.parse_args()

    script_dir = Path(__file__).parent
    student_dir = script_dir / args.student_dir
    output_path = script_dir / args.output

    if not student_dir.exists():
        print(f"ERROR: Student model not found at {student_dir}")
        print("Run distill.py first.")
        sys.exit(1)

    output_path.parent.mkdir(parents=True, exist_ok=True)

    if args.format in ("gguf", "both"):
        export_gguf(student_dir, output_path, script_dir)

    if args.format in ("onnx", "both"):
        onnx_output = output_path.with_suffix(".onnx")
        export_onnx(student_dir, onnx_output)

    print("\nExport complete!")


if __name__ == "__main__":
    main()
