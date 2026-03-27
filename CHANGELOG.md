# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog,
and this project adheres to Semantic Versioning.

## [Unreleased]
### Added
- Initial repository setup.
- Basic `no_std` bare-metal Rust environment using `cargo-bootimage`.
- QEMU runner integration and VGA text buffer display.
- Exception handling (IDT) and PIC initialization to prevent kernel panics.
- UART (COM1) serial communication module for Host PC interaction (Umbilical Cord).
- Dual-Brain architecture implementation with Python (`host_brain.py`) using Google Gemini API.
- Dynamic hardware driver injection (sending 128-byte PS/2 keymap via serial to the running kernel).
- RAG (Retrieval-Augmented Generation) style prompt injection for hardware manuals to fix AI hallucination.
- CLI argument parsing in `host_brain.py` to allow dynamic LLM model selection (e.g., `--model gemini-2.5-pro`).
- 2D VGA Text Buffer implementation with Enter, Backspace, Space support, and vertical scrolling.
- Expanded State Machine in Layer 0 to track Shift, Ctrl, Alt, and E0 extended keys, using a 256-byte keymap.
- Implementation of "Generative Calibration": OS prompts "Hi.OpenRhiza!", Host AI analyzes the scancode sequence to perfectly infer keyboard layout (QWERTY/Dvorak) and injects the corresponding driver.
- Integrated `wasmi` crate to establish a true WebAssembly (Wasm) runtime sandbox within the bare-metal Layer 0 kernel.