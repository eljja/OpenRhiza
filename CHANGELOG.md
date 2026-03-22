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