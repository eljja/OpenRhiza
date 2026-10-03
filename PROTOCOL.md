# OpenRhiza Serial Communication Protocol

This protocol is used by the development-time serial bridge between the kernel and host tooling.
It is primarily relevant for `host_brain.py` and any host-side scripts that inject keymaps or Wasm payloads.

Important scope note:

- This is a development and bring-up protocol
- It is not the long-term production transport
- The active kernel still supports it
- It should be treated as a legacy/bootstrap protocol, not as the preferred future runtime model

## Transport

```text
OpenRhiza guest (QEMU)  <->  UART 16550 (COM1)  <->  TCP 127.0.0.1:4444  <->  host_brain.py
```

- Physical abstraction: QEMU-emulated 16550 UART
- Framing: raw byte stream
- Text logs: UTF-8 lines ending in `\n`

## Host -> OS Bytes

| Byte | Name | Meaning |
|------|------|---------|
| `0xFD` | `KEYMAP_RESET` | Reset the 256-byte dynamic keymap buffer |
| `0xFE` | `CALIBRATION_FAIL` | Notify the OS that calibration failed |
| `0xFB` | `DRIVER_GENERATING` | Host-side AI is generating a driver |
| `0xFA` | `DRIVER_GEN_FAILED` | Host-side AI failed to generate a driver |
| `0xFC` | `WASM_TRANSFER_START` | Start of a Wasm payload transfer |
| `0x00`..`0xFF` | keymap bytes | Keymap payload after `0xFD` |

### Wasm transfer format

```text
[0xFC] [size0] [size1] [size2] [size3] [wasm bytes...]
```

- Size is a little-endian `u32`
- The host tooling currently inserts a small delay between bytes to avoid overrunning early serial handling

## OS -> Host Bytes

| Byte | Name | Meaning |
|------|------|---------|
| `0xF8` | `WASM_EXEC_SUCCESS` | Wasm execution succeeded |
| `0xF9` | `WASM_EXEC_PANIC` | Wasm execution failed |

### Panic payload format

```text
[0xF9] [len0] [len1] [len2] [len3] [utf8 error bytes...]
```

## Text Logs

Regular kernel logging is sent over the same serial stream as UTF-8 text.
Host tools should treat newline-separated text and protocol bytes carefully if they mix both paths.

## Byte Map

```text
0x00 ~ 0xEF : general data
0xF0 ~ 0xF7 : reserved for future use
0xF8        : WASM_EXEC_SUCCESS
0xF9        : WASM_EXEC_PANIC
0xFA        : DRIVER_GEN_FAILED
0xFB        : DRIVER_GENERATING
0xFC        : WASM_TRANSFER_START
0xFD        : KEYMAP_RESET
0xFE        : CALIBRATION_FAIL
0xFF        : reserved
```

## Current Relevance

The current `main.rs` boot path still recognizes:

- dynamic keymap reset/injection
- host-side Wasm transfer start
- generation status bytes

At the same time, the kernel now also contains native keyboard and native xHCI logic, so the serial protocol
is no longer the only route for early input and experimentation.

Current preferred usage:

- serial is primarily for debug logs and legacy bring-up experiments
- normal AI interaction should happen through the guest prompt path and direct OpenRhiza/Gemini API path
- generated capabilities should increasingly move through OpenRhiza.com, local seed slots, and sandbox validation rather than ad-hoc serial injection
