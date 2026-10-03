from __future__ import annotations

import argparse
import json
import tempfile
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path


if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(errors="replace")

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ELF = REPO_ROOT / "target" / "aarch64-openrhiza-none" / "debug" / "openrhiza-arm64.elf"
QEMU_CANDIDATES = [
    Path("C:/Program Files/qemu/qemu-system-aarch64.exe"),
    Path("qemu-system-aarch64.exe"),
]
INJECT_NOTES: list[str] = []
PRODUCTION_RUNTIME_DEFAULT_BATCHES = 8


def production_expected_subsystem(ticks: int, device: str | None) -> str:
    values = {
        "block_read": 0,
        "block_write": 0,
        "gpu_control": 0,
        "net_rx": 0,
        "net_tx": 0,
        "input_events": 0,
    }
    if device is None or device == "block":
        values["block_read"] = ticks
        values["block_write"] = ticks
    if device is None or device == "gpu":
        values["gpu_control"] = ticks
    if device is None or device == "net":
        values["net_rx"] = ticks
        values["net_tx"] = ticks
    if device is None or device == "input":
        values["input_events"] = ticks
    return (
        f"subsystem producer counters block_read={values['block_read']} "
        f"block_write={values['block_write']} gpu_control={values['gpu_control']} "
        f"net_rx={values['net_rx']} net_tx={values['net_tx']} "
        f"input_events={values['input_events']} rejected=0"
    )


def production_expected_hardware_payload(ticks: int, device: str | None) -> str:
    return production_expected_subsystem(ticks, device).replace(
        "subsystem producer counters", "ARM64 hardware payload dispatch counters"
    )


def live_pump_expected_counters(ticks: int, device: str | None) -> str:
    values = {
        "block_read": 0,
        "block_write": 0,
        "gpu_control": 0,
        "net_rx": 0,
        "net_tx": 0,
        "input_events": 0,
    }
    if device is None or device == "block":
        values["block_read"] = ticks
        values["block_write"] = ticks
    if device is None or device == "gpu":
        values["gpu_control"] = ticks
    if device is None or device == "net":
        values["net_rx"] = ticks
        values["net_tx"] = ticks
    if device is None or device == "input":
        values["input_events"] = ticks
    return (
        f"live OS pump counters block_read={values['block_read']} "
        f"block_write={values['block_write']} gpu_control={values['gpu_control']} "
        f"net_rx={values['net_rx']} net_tx={values['net_tx']} "
        f"input_events={values['input_events']} failures=0"
    )


def virtio_device_mask(device: str | None) -> str:
    values = {
        "block": 0x00000001,
        "gpu": 0x00000002,
        "net": 0x00000004,
        "input": 0x00000008,
        None: 0x0000000F,
    }
    return f"0x{values[device]:08x}"


def production_expected_inbox(expected_items: int) -> str:
    return (
        "production subsystem inbox block_read_pending=0 block_write_pending=0 "
        "gpu_control_pending=0 net_rx_pending=0 net_tx_pending=0 "
        f"input_event_pending=0 enqueued={expected_items} "
        f"forwarded={expected_items} rejected=0"
    )


def validation_failed(reason: str) -> int:
    print(f"[harness] validation-failed {reason}", file=sys.stderr)
    return 1


def resolve_qemu() -> str:
    for candidate in QEMU_CANDIDATES:
        if candidate.exists():
            return str(candidate)
    return "qemu-system-aarch64"


def reserve_tcp_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def reserve_vnc_display() -> tuple[int, socket.socket]:
    for display in range(40, 100):
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            sock.bind(("127.0.0.1", 5900 + display))
        except OSError:
            sock.close()
            continue
        return display, sock
    raise RuntimeError("no free local VNC display slot found")


def send_qmp_pointer(port: int) -> None:
    deadline = time.time() + 2.0
    last_error: OSError | None = None
    while time.time() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.25) as conn:
                conn.settimeout(0.25)
                try:
                    conn.recv(4096)
                except OSError:
                    pass
                conn.sendall(json.dumps({"execute": "qmp_capabilities"}).encode("ascii") + b"\r\n")
                try:
                    conn.recv(4096)
                except OSError:
                    pass
                pointer = {
                    "execute": "input-send-event",
                    "arguments": {
                        "events": [
                            {"type": "rel", "data": {"axis": "x", "value": 12}},
                            {"type": "rel", "data": {"axis": "y", "value": 7}},
                            {"type": "btn", "data": {"down": True, "button": "left"}},
                            {"type": "btn", "data": {"down": False, "button": "left"}},
                        ],
                    },
                }
                conn.sendall(json.dumps(pointer).encode("ascii") + b"\r\n")
                try:
                    response = conn.recv(4096)
                    if response and len([note for note in INJECT_NOTES if note.startswith("qmp-pointer-response")]) < 2:
                        INJECT_NOTES.append(
                            "qmp-pointer-response "
                            + response.decode("utf-8", errors="replace").strip().replace("\r", " ").replace("\n", " ")
                        )
                except OSError:
                    pass
                return
        except OSError as exc:
            last_error = exc
            time.sleep(0.05)
    raise RuntimeError(f"QMP pointer injection failed: {last_error}")


def start_qmp_pointer_injector(port: int, duration: float = 5.0) -> threading.Thread:
    def worker() -> None:
        deadline = time.time() + duration
        while time.time() < deadline:
            try:
                send_qmp_pointer(port)
            except RuntimeError:
                pass
            time.sleep(0.05)

    thread = threading.Thread(target=worker, name="qmp-pointer-injector", daemon=True)
    thread.start()
    return thread


def send_vnc_key(port: int, keysym: int = 0x61) -> None:
    with socket.create_connection(("127.0.0.1", port), timeout=0.5) as conn:
        conn.settimeout(0.5)
        version = conn.recv(12)
        if not version.startswith(b"RFB "):
            raise RuntimeError("VNC handshake failed: missing RFB version")
        conn.sendall(version)
        count = conn.recv(1)
        if not count:
            raise RuntimeError("VNC handshake failed: missing security list")
        security_count = count[0]
        security_types = conn.recv(security_count)
        if 1 not in security_types:
            raise RuntimeError(f"VNC server does not offer no-auth security: {security_types!r}")
        conn.sendall(b"\x01")
        result = conn.recv(4)
        if len(result) != 4 or result != b"\x00\x00\x00\x00":
            raise RuntimeError("VNC no-auth security failed")
        conn.sendall(b"\x01")
        header = conn.recv(24)
        if len(header) < 24:
            raise RuntimeError("VNC server init failed")
        name_len = int.from_bytes(header[20:24], "big")
        while name_len > 0:
            chunk = conn.recv(min(name_len, 256))
            if not chunk:
                break
            name_len -= len(chunk)
        key = keysym.to_bytes(4, "big")
        conn.sendall(b"\x04\x01\x00\x00" + key)
        time.sleep(0.03)
        conn.sendall(b"\x04\x00\x00\x00" + key)


def send_vnc_pointer(port: int, x: int = 320, y: int = 240) -> None:
    with socket.create_connection(("127.0.0.1", port), timeout=0.5) as conn:
        conn.settimeout(0.5)
        version = conn.recv(12)
        if not version.startswith(b"RFB "):
            raise RuntimeError("VNC handshake failed: missing RFB version")
        conn.sendall(version)
        count = conn.recv(1)
        if not count:
            raise RuntimeError("VNC handshake failed: missing security list")
        security_count = count[0]
        security_types = conn.recv(security_count)
        if 1 not in security_types:
            raise RuntimeError(f"VNC server does not offer no-auth security: {security_types!r}")
        conn.sendall(b"\x01")
        result = conn.recv(4)
        if len(result) != 4 or result != b"\x00\x00\x00\x00":
            raise RuntimeError("VNC no-auth security failed")
        conn.sendall(b"\x01")
        header = conn.recv(24)
        if len(header) < 24:
            raise RuntimeError("VNC server init failed")
        name_len = int.from_bytes(header[20:24], "big")
        while name_len > 0:
            chunk = conn.recv(min(name_len, 256))
            if not chunk:
                break
            name_len -= len(chunk)
        conn.sendall(b"\x05\x00" + x.to_bytes(2, "big") + y.to_bytes(2, "big"))
        time.sleep(0.03)
        conn.sendall(b"\x05\x01" + x.to_bytes(2, "big") + y.to_bytes(2, "big"))
        time.sleep(0.03)
        conn.sendall(b"\x05\x00" + (x + 20).to_bytes(2, "big") + (y + 20).to_bytes(2, "big"))


def start_vnc_input_injector(display: int, duration: float = 5.0) -> threading.Thread:
    def worker() -> None:
        port = 5900 + display
        deadline = time.time() + duration
        step = 0
        while time.time() < deadline:
            try:
                send_vnc_pointer(port, 200 + (step % 80), 180 + (step % 60))
                if not any(note.startswith("vnc-input-ok") for note in INJECT_NOTES):
                    INJECT_NOTES.append(f"vnc-input-ok display={display} port={port}")
            except (OSError, RuntimeError):
                if len([note for note in INJECT_NOTES if note.startswith("vnc-input-error")]) < 3:
                    INJECT_NOTES.append(f"vnc-input-error display={display} port={port}")
                pass
            step += 7
            time.sleep(0.08)

    thread = threading.Thread(target=worker, name="vnc-input-injector", daemon=True)
    thread.start()
    return thread


def start_udp_net_injector(port: int, duration: float = 5.0) -> threading.Thread:
    def worker() -> None:
        dst = bytes([0x52, 0x54, 0x00, 0x12, 0x34, 0x56])
        src = bytes([0x52, 0x54, 0x00, 0xab, 0xcd, 0xef])
        frame = dst + src + bytes([0x88, 0xb5]) + b"OpenRhiza ARM64 virtio-net RX injected payload"
        if len(frame) < 60:
            frame += bytes(60 - len(frame))
        deadline = time.time() + duration
        sent = 0
        failures = 0
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
            while time.time() < deadline:
                # The ARM64 virtio-net sandbox seed re-arms RX queues during
                # each live dispatch. Send a small burst so the queue sees a
                # packet even if the harness and guest drift by a few ms.
                for _ in range(8):
                    try:
                        sock.sendto(frame, ("127.0.0.1", port))
                        sent += 1
                    except OSError:
                        failures += 1
                time.sleep(0.005)
        INJECT_NOTES.append(f"udp-net-injector sent={sent} failures={failures} port={port}")

    thread = threading.Thread(target=worker, name="udp-net-injector", daemon=True)
    thread.start()
    return thread


def has_arm64_prompt(text: str) -> bool:
    return "arm64>" in text or "input>" in text


def ends_with_arm64_prompt(text: str) -> bool:
    stripped = text.rstrip()
    return stripped.endswith("arm64>") or stripped.endswith("input>")


def should_send_external_normal_manual_line(
    smoke_input: str, manual_sent: int, text: str
) -> bool:
    if "/vosprod-external-normal-source-live" not in smoke_input:
        return False
    device_round_mode = "/vosprod-external-normal-submit-device-round" in smoke_input
    if device_round_mode:
        round_count = smoke_input.count("/vosprod-external-normal-submit-device-round")
        if (
            manual_sent == 1
            and "external normal subsystem source active=1" in text
            and ends_with_arm64_prompt(text)
        ):
            return True
        if 1 < manual_sent <= round_count:
            completed_tick = manual_sent - 1
            return f"live OS pump mainloop tick={completed_tick} result=1" in text
        if manual_sent == round_count + 1:
            return f"live OS pump mainloop tick={round_count} result=1" in text
        return manual_sent == round_count + 2 and "production queue runtime running=1" in text

    device_submit_mode = "/vosprod-external-normal-submit-block" in smoke_input
    if not device_submit_mode:
        if (
            manual_sent == 1
            and "external normal subsystem source active=1" in text
            and ends_with_arm64_prompt(text)
        ):
            return True
        if manual_sent == 2 and "production external-normal keepalive active=1" in text:
            return True
        return manual_sent == 3 and "production queue runtime running=1" in text

    device_submit_markers = {
        1: "external normal subsystem source active=1",
        2: "external normal subsystem submit-block accepted=2",
        3: "external normal subsystem submit-gpu accepted=1",
        4: "external normal subsystem submit-net accepted=2",
        5: "production external-normal keepalive active=1",
        6: "production queue runtime running=1",
    }
    marker = device_submit_markers.get(manual_sent)
    if marker is None or marker not in text:
        return False
    if manual_sent == 1:
        return ends_with_arm64_prompt(text)
    return True


def collect_serial(
    port: int,
    smoke_input: str,
    timeout: float,
    qmp_port: int | None = None,
    inject_input: bool = False,
) -> str:
    deadline = time.time() + timeout
    last_error: OSError | None = None
    while time.time() < deadline:
        try:
            conn = socket.create_connection(("127.0.0.1", port), timeout=0.25)
            break
        except OSError as exc:
            last_error = exc
            time.sleep(0.05)
    else:
        raise RuntimeError(f"serial tcp connection failed: {last_error}")

    chunks: list[bytes] = []
    manual_lines = (
        [line for line in smoke_input.splitlines() if line.strip()]
        if (
            "/vosprod-source-live" in smoke_input
            or "/vosprod-real-source-live" in smoke_input
            or "/vosprod-normal-source-live" in smoke_input
            or "/vosprod-sandbox-source-live" in smoke_input
            or "/vosprod-payload-source-live" in smoke_input
            or "/vosprod-external-normal-source-live" in smoke_input
            or "/vosboot-normal-live" in smoke_input
        )
        else []
    )
    script_bytes = smoke_input.replace("\n", "\r").encode("ascii")
    script_marker = smoke_input.strip().splitlines()[-1] if smoke_input.strip() else ""

    def send_script(conn: socket.socket) -> None:
        nonlocal manual_sent
        # QEMU's PL011 TCP backend can drop a fast burst during early boot on
        # Windows. Feed the script at UART-like speed so the recovery prompt
        # receives deterministic keystrokes.
        time.sleep(0.2)
        command = (
            manual_lines[manual_sent] + "\r"
            if manual_lines and manual_sent < len(manual_lines)
            else None
        )
        if command is not None:
            payload = command.encode("ascii")
            manual_sent += 1
        else:
            payload = script_bytes
        for byte in payload:
            conn.sendall(bytes([byte]))
            time.sleep(0.018)

    with conn:
        conn.settimeout(0.15)
        sent_input = False
        manual_sent = 0
        last_script_send = 0.0
        qmp_inject_until = 0.0
        last_qmp_inject = 0.0
        while time.time() < deadline:
            try:
                chunk = conn.recv(4096)
            except socket.timeout:
                if (
                    not manual_lines
                    and (
                        not sent_input
                        or (
                            script_marker
                            and script_marker
                            not in b"".join(chunks).decode("utf-8", errors="replace")
                        )
                    )
                ):
                    text = b"".join(chunks).decode("utf-8", errors="replace")
                    if has_arm64_prompt(text) and time.time() - last_script_send > 0.75:
                        send_script(conn)
                        sent_input = True
                        last_script_send = time.time()
                continue
            except OSError as exc:
                chunks.append(f"\n[harness] serial-read-error {exc}\n".encode("utf-8", errors="replace"))
                break
            if not chunk:
                break
            chunks.append(chunk)
            text = b"".join(chunks).decode("utf-8", errors="replace")
            if not sent_input and has_arm64_prompt(text):
                send_script(conn)
                sent_input = True
                last_script_send = time.time()
            if manual_lines and manual_sent < len(manual_lines):
                if should_send_external_normal_manual_line(smoke_input, manual_sent, text):
                    send_script(conn)
                    last_script_send = time.time()
                elif manual_sent == 1 and "production source-live keepalive active=1" in text:
                    send_script(conn)
                    last_script_send = time.time()
                elif manual_sent == 2 and "production queue runtime running=1" in text:
                    send_script(conn)
                    last_script_send = time.time()
            if inject_input and qmp_port is not None:
                if (
                    "loading sandbox skill: virtio-input driver seed" in text
                    or "matched sandbox candidate: virtio-input" in text
                    or "[Skill][virtio] driver_ok begin" in text
                    or "[Skill][virtio] notify begin" in text
                    or "[Skill][virtio] wait_used_idx begin" in text
                    or "virtio-input payload: arm event buffer and poll." in text
                ):
                    qmp_inject_until = max(qmp_inject_until, time.time() + 8.0)
                if time.time() < qmp_inject_until and time.time() - last_qmp_inject > 0.08:
                    try:
                        send_qmp_pointer(qmp_port)
                    except RuntimeError:
                        pass
                    last_qmp_inject = time.time()
            if "/virtio-wasm-injected-selftest" in smoke_input:
                if "virtio injected sandbox runtime selftest complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-live-routing-selftest" in smoke_input:
                if "virtio production-routing diagnostic complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-live-route-all-selftest" in smoke_input:
                if "virtio production route-all selftest complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-live-boot-service-selftest" in smoke_input:
                if "virtio live boot service selftest complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-live-service-loop-selftest" in smoke_input:
                if "virtio live service mainloop selftest complete" in text:
                    break
            elif "/vosprod-start" in smoke_input:
                if "virtio live service stop complete" in text and ends_with_arm64_prompt(text):
                    break
            elif (
                "/vosprod-source-live" in smoke_input
                or "/vosprod-real-source-live" in smoke_input
                or "/vosprod-normal-source-live" in smoke_input
                or "/vosprod-sandbox-source-live" in smoke_input
                or "/vosprod-payload-source-live" in smoke_input
                or "/vosprod-external-normal-source-live" in smoke_input
            ):
                if "virtio live service stop complete" in text:
                    break
            elif "/vosboot-normal-live" in smoke_input:
                if "source_mode=validated-keepalive boot_default_live=0" in text:
                    break
            elif (
                "/virtio-wasm-live-os-pump-selftest" in smoke_input
                or "/vosrq" in smoke_input
                or "/voslive" in smoke_input
                or "/vossys" in smoke_input
                or "/vosps" in smoke_input
                or "/vosprod" in smoke_input
                or "/vosboot" in smoke_input
                or "/vosman" in smoke_input
            ):
                if "virtio live OS pump selftest complete" in text:
                    break
            elif "/virtio-wasm-live-cutover-rollback-" in smoke_input:
                if "live cutover+rollback selftest complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-enable-gate-" in smoke_input:
                if "device enable gate complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-sustained-gate-selftest" in smoke_input:
                if "virtio sustained rollback-window gate complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-traffic-window-selftest" in smoke_input:
                if "virtio traffic-counter window complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-promotion-gate-selftest" in smoke_input:
                if "virtio promotion-gate sandbox runtime selftest complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-rollback-selftest" in smoke_input:
                if "virtio rollback sandbox runtime selftest complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-rust-probe" in smoke_input:
                if "rust payload probe complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-wasm-selftest" in smoke_input:
                if "virtio sandbox runtime selftest complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-payload-selftest" in smoke_input:
                if "virtio payload selftest complete" in text and ends_with_arm64_prompt(text):
                    break
            elif "/virtio-mmio-probe" in smoke_input:
                if "transport identity" in text and ends_with_arm64_prompt(text):
                    break
            elif has_arm64_prompt(text) and sent_input:
                break
    return b"".join(chunks).decode("utf-8", errors="replace")


def collect_stdio(
    proc: subprocess.Popen[str],
    smoke_input: str,
    timeout: float,
    qmp_port: int | None = None,
    inject_input: bool = False,
) -> str:
    deadline = time.time() + timeout
    chunks: list[str] = []
    lock = threading.Lock()

    def reader() -> None:
        assert proc.stdout is not None
        while True:
            char = proc.stdout.read(1)
            if not char:
                break
            with lock:
                chunks.append(char)

    thread = threading.Thread(target=reader, name="arm64-serial-reader", daemon=True)
    thread.start()

    sent_input = False
    text = ""
    scanned_chars = 0
    manual_lines = (
        [line for line in smoke_input.splitlines() if line.strip()]
        if (
            "/vosman" in smoke_input
            or "/vosprod-start" in smoke_input
            or "/vosprod-source-live" in smoke_input
            or "/vosprod-real-source-live" in smoke_input
            or "/vosprod-normal-source-live" in smoke_input
            or "/vosprod-sandbox-source-live" in smoke_input
            or "/vosprod-payload-source-live" in smoke_input
            or "/vosprod-external-normal-source-live" in smoke_input
            or "/vosboot-normal-live" in smoke_input
        )
        else []
    )
    manual_sent = 0
    qmp_inject_until = 0.0
    last_qmp_inject = 0.0

    def send_command(command: str) -> None:
        assert proc.stdin is not None
        for byte in command + "\n":
            proc.stdin.write(byte)
            proc.stdin.flush()
            time.sleep(0.018)

    while time.time() < deadline:
        with lock:
            if len(chunks) > scanned_chars:
                text += "".join(chunks[scanned_chars:])
                scanned_chars = len(chunks)
        if manual_lines and has_arm64_prompt(text) and proc.stdin is not None:
            if manual_sent == 0:
                time.sleep(0.2)
                send_command(manual_lines[0])
                manual_sent = 1
                sent_input = True
            elif "/vosprod-start" in smoke_input:
                if (
                    manual_sent == 1
                    and "command returned; external production inbox submits can now feed sandbox owners." in text
                    and ends_with_arm64_prompt(text)
                    and manual_sent < len(manual_lines)
                ):
                    time.sleep(0.2)
                    send_command(manual_lines[manual_sent])
                    manual_sent += 1
                elif 1 < manual_sent < len(manual_lines):
                    completed_tick = manual_sent - 1
                    if manual_lines[manual_sent] == "/vosprod-all":
                        if f"live OS pump mainloop tick={completed_tick} result=1" in text:
                            time.sleep(0.2)
                            send_command(manual_lines[manual_sent])
                            manual_sent += 1
                    elif manual_lines[manual_sent] == "/vosstat":
                        if f"live OS pump mainloop tick={completed_tick} result=1" in text:
                            time.sleep(0.2)
                            send_command(manual_lines[manual_sent])
                            manual_sent += 1
                    elif (
                        manual_lines[manual_sent] == "/vosstop"
                        and "production queue runtime running=1" in text
                        and ends_with_arm64_prompt(text)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
            elif "/vosprod-external-normal-source-live" in smoke_input:
                if should_send_external_normal_manual_line(
                    smoke_input, manual_sent, text
                ) and manual_sent < len(manual_lines):
                    time.sleep(0.2)
                    send_command(manual_lines[manual_sent])
                    manual_sent += 1
            elif (
                "/vosprod-source-live" in smoke_input
                or "/vosprod-real-source-live" in smoke_input
                or "/vosprod-normal-source-live" in smoke_input
                or "/vosprod-sandbox-source-live" in smoke_input
                or "/vosprod-payload-source-live" in smoke_input
                or "/vosboot-normal-live" in smoke_input
            ):
                if "/vospromote-payload-" in smoke_input:
                    if (
                        manual_sent == 1
                        and "production source-live keepalive active=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 2
                        and "production queue runtime running=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 3
                        and "hardware-payload sandbox-active owner recorded." in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 4
                        and "live dispatch rounds=4 passed=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 5
                        and "rollback proof=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 6
                        and "owner_mask=0x00000000" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                elif "/vospolicy-storage-stage" in smoke_input:
                    storage_proof_mode = "/vospolicy-storage-attach-proof" in smoke_input
                    storage_provider_mode = (
                        "/vospolicy-storage-provider-attach-proof" in smoke_input
                    )
                    storage_provider_restore_mode = (
                        "/vospolicy-storage-provider-restore-proof" in smoke_input
                    )
                    if (
                        manual_sent == 1
                        and "production source-live keepalive active=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 2
                        and "production queue runtime running=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 3
                        and "hardware-payload boot-policy candidate recorded." in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 4
                        and "hardware-payload boot-policy capsule sealed" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 5
                        and "hardware-payload boot-policy capsule verify active=1 ok=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 6
                        and "hardware-payload policy storage write-intent staged" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 7
                        and storage_provider_mode
                        and "sandbox policy storage provider proof attached" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 8
                        and storage_provider_mode
                        and "policy storage provider seal accepted" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 9
                        and storage_provider_restore_mode
                        and "hardware-payload boot-policy capsule cleared" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 10
                        and storage_provider_restore_mode
                        and "policy storage provider restore active=1 ok=1 restored=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 11
                        and storage_provider_restore_mode
                        and "policy storage provider readback active=1 ok=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 12
                        and storage_provider_restore_mode
                        and "policy storage provider proof cleared" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 13
                        and storage_provider_restore_mode
                        and "storage provider active=0" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 9
                        and not storage_provider_restore_mode
                        and storage_provider_mode
                        and "policy storage provider readback active=1 ok=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 10
                        and storage_provider_mode
                        and "policy storage provider proof cleared" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 11
                        and storage_provider_mode
                        and "storage provider active=0" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 7
                        and storage_proof_mode
                        and "sandbox policy storage proof capability attached" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 8
                        and storage_proof_mode
                        and "policy storage commit accepted into volatile proof slot" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 9
                        and storage_proof_mode
                        and "policy storage volatile proof verify active=1 ok=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 10
                        and storage_proof_mode
                        and "policy storage volatile proof slot cleared" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 11
                        and storage_proof_mode
                        and "volatile_committed=0" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 7
                        and not storage_proof_mode
                        and not storage_provider_mode
                        and "policy storage commit denied; storage capability missing" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 8
                        and not storage_proof_mode
                        and not storage_provider_mode
                        and "policy storage write-intent rolled back" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 9
                        and not storage_proof_mode
                        and not storage_provider_mode
                        and "policy storage intent active=0 mask=0x00000000" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                elif "/vospolicy-capsule-seal-" in smoke_input:
                    if (
                        manual_sent == 1
                        and "production source-live keepalive active=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 2
                        and "production queue runtime running=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 3
                        and "hardware-payload boot-policy candidate recorded." in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 4
                        and "hardware-payload boot-policy capsule sealed" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 5
                        and "hardware-payload boot-policy capsule verify active=1 ok=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 6
                        and "hardware-payload boot-policy capsule cleared" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 7
                        and "boot-policy capsule active=0 mask=0x00000000" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                elif "/vospolicy-payload-" in smoke_input:
                    if (
                        manual_sent == 1
                        and "production source-live keepalive active=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 2
                        and "production queue runtime running=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 3
                        and "hardware-payload boot-policy candidate recorded." in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 4
                        and "boot-policy candidate mask=" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 5
                        and "boot-policy candidates cleared" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 6
                        and "boot-policy candidate mask=0x00000000" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                else:
                    if (
                        manual_sent == 1
                        and "production source-live keepalive active=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
                    elif (
                        manual_sent == 2
                        and "production queue runtime running=1" in text
                        and manual_sent < len(manual_lines)
                    ):
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
            else:
                if (
                    manual_sent == 1
                    and "command returned; external subsystem submit commands must feed sandbox owners." in text
                    and ends_with_arm64_prompt(text)
                    and manual_sent < len(manual_lines)
                ):
                    time.sleep(0.2)
                    send_command(manual_lines[manual_sent])
                    manual_sent += 1
                elif 1 < manual_sent < len(manual_lines):
                    completed_tick = manual_sent - 1
                    if f"live OS pump mainloop tick={completed_tick} result=1" in text:
                        time.sleep(0.2)
                        send_command(manual_lines[manual_sent])
                        manual_sent += 1
        elif not sent_input and has_arm64_prompt(text) and proc.stdin is not None:
            time.sleep(0.2)
            # QEMU stdio on Windows needs a real newline to flush the PL011
            # command path; TCP serial stays CR-paced in collect_serial.
            for byte in smoke_input:
                proc.stdin.write(byte)
                proc.stdin.flush()
                time.sleep(0.018)
            sent_input = True
        if inject_input and qmp_port is not None:
            if (
                "loading sandbox skill: virtio-input driver seed" in text
                or "matched sandbox candidate: virtio-input" in text
                or "[Skill][virtio] driver_ok begin" in text
                or "[Skill][virtio] notify begin" in text
                or "[Skill][virtio] wait_used_idx begin" in text
                or "virtio-input payload: arm event buffer and poll." in text
            ):
                qmp_inject_until = max(qmp_inject_until, time.time() + 8.0)
            if time.time() < qmp_inject_until and time.time() - last_qmp_inject > 0.08:
                try:
                    send_qmp_pointer(qmp_port)
                except RuntimeError:
                    pass
                last_qmp_inject = time.time()
        if "/virtio-wasm-injected-selftest" in smoke_input:
            if "virtio injected sandbox runtime selftest complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-live-routing-selftest" in smoke_input:
            if "virtio production-routing diagnostic complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-live-route-all-selftest" in smoke_input:
            if "virtio production route-all selftest complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-live-boot-service-selftest" in smoke_input:
            if "virtio live boot service selftest complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-live-service-loop-selftest" in smoke_input:
            if "virtio live service mainloop selftest complete" in text:
                break
        elif "/vosprod-start" in smoke_input:
            if "virtio live service stop complete" in text and ends_with_arm64_prompt(text):
                break
        elif (
            "/vosprod-source-live" in smoke_input
            or "/vosprod-real-source-live" in smoke_input
            or "/vosprod-normal-source-live" in smoke_input
            or "/vosprod-sandbox-source-live" in smoke_input
            or "/vosprod-payload-source-live" in smoke_input
            or "/vosprod-external-normal-source-live" in smoke_input
        ):
            if "virtio live service stop complete" in text:
                break
        elif "/vosboot-normal-live" in smoke_input:
            if "source_mode=validated-keepalive boot_default_live=0" in text:
                break
        elif (
            "/virtio-wasm-live-os-pump-selftest" in smoke_input
            or "/vospl" in smoke_input
            or "/vosrq" in smoke_input
            or "/voslive" in smoke_input
            or "/vossys" in smoke_input
            or "/vosps" in smoke_input
            or "/vosprod" in smoke_input
            or "/vosboot" in smoke_input
            or "/vosman" in smoke_input
        ):
            if "virtio live OS pump selftest complete" in text:
                break
        elif "/virtio-wasm-live-cutover-rollback-" in smoke_input:
            if "live cutover+rollback selftest complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-enable-gate-" in smoke_input:
            if "device enable gate complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-sustained-gate-selftest" in smoke_input:
            if "virtio sustained rollback-window gate complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-traffic-window-selftest" in smoke_input:
            if "virtio traffic-counter window complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-promotion-gate-selftest" in smoke_input:
            if "virtio promotion-gate sandbox runtime selftest complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-rollback-selftest" in smoke_input:
            if "virtio rollback sandbox runtime selftest complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-rust-probe" in smoke_input:
            if "rust payload probe complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-wasm-selftest" in smoke_input:
            if "virtio sandbox runtime selftest complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-payload-selftest" in smoke_input:
            if "virtio payload selftest complete" in text and ends_with_arm64_prompt(text):
                break
        elif "/virtio-mmio-probe" in smoke_input:
            if "transport identity" in text and ends_with_arm64_prompt(text):
                break
        elif sent_input and ends_with_arm64_prompt(text):
            break
        time.sleep(0.02)

    with lock:
        return "".join(chunks)


def main() -> int:
    parser = argparse.ArgumentParser(description="Smoke-test the OpenRhiza ARM64 serial recovery ELF in QEMU.")
    parser.add_argument("--elf", type=Path, default=DEFAULT_ELF)
    parser.add_argument("--timeout", type=float, default=4.0)
    parser.add_argument(
        "--cli-baseline",
        action="store_true",
        help="Exercise the ARM64 user-facing recovery CLI commands without requiring virtio payload devices.",
    )
    parser.add_argument(
        "--virtio-probe",
        action="store_true",
        help="Attach a QEMU virtio-mmio net device and exercise the read-only transport probe.",
    )
    parser.add_argument(
        "--virtio-payload",
        action="store_true",
        help="Attach virtio-mmio net/block/input/gpu devices and run the ARM64 payload verification harness.",
    )
    parser.add_argument(
        "--virtio-wasm",
        action="store_true",
        help="With --virtio-payload, require the ARM64 Wasm sandbox runtime virtio selftest to complete.",
    )
    parser.add_argument(
        "--virtio-wasm-injected",
        action="store_true",
        help="With --virtio-payload --inject-input --inject-net-rx, move injected input/net RX payloads through the tiny Wasm host-import executor.",
    )
    parser.add_argument(
        "--virtio-wasm-rollback",
        action="store_true",
        help="With --virtio-payload, repeat device-specific tiny Wasm payload tests across explicit reset/rollback cycles.",
    )
    parser.add_argument(
        "--virtio-wasm-promotion-gate",
        action="store_true",
        help="With --virtio-payload --inject-input --inject-net-rx, run full Wasm virtio driver seeds through proof-only promotion gates.",
    )
    parser.add_argument(
        "--virtio-wasm-sustained-gate",
        action="store_true",
        help="With --virtio-payload --inject-input --inject-net-rx, repeat full Wasm promotion proof gates across a sustained rollback window.",
    )
    parser.add_argument(
        "--virtio-wasm-traffic-window",
        action="store_true",
        help="With --virtio-payload --inject-input --inject-net-rx, accumulate per-device full Wasm traffic counters before live promotion.",
    )
    parser.add_argument(
        "--virtio-wasm-device-enable-gate",
        choices=["block", "gpu", "net", "input"],
        help="With --virtio-payload, run one proof-only 12-round full Wasm device enable gate.",
    )
    parser.add_argument(
        "--virtio-wasm-device-live-cutover",
        choices=["block", "gpu", "net", "input"],
        help="With --virtio-payload, run one explicit live owner cutover and rollback selftest for a device.",
    )
    parser.add_argument(
        "--virtio-wasm-live-routing",
        action="store_true",
        help="With --virtio-payload, cut over each virtio device, run sandbox-owner dispatch windows, then rollback.",
    )
    parser.add_argument(
        "--virtio-wasm-live-route-all",
        action="store_true",
        help="With --virtio-payload, cut over all virtio devices together, run dispatch windows, then rollback all owners.",
    )
    parser.add_argument(
        "--virtio-wasm-live-boot-service",
        action="store_true",
        help="With --virtio-payload, boot recovery, start the sandbox-owner service pump, run bounded dispatch ticks, then rollback all owners.",
    )
    parser.add_argument(
        "--virtio-wasm-live-service-loop",
        action="store_true",
        help="With --virtio-payload, start the sandbox-owner service and require the ARM64 main loop idle pump to dispatch and rollback.",
    )
    parser.add_argument(
        "--virtio-wasm-live-os-pump",
        action="store_true",
        help="With --virtio-payload, route typed OS block/net/input/gpu request/event work through the sandbox-owner main loop pump.",
    )
    parser.add_argument(
        "--virtio-wasm-live-os-pump-long",
        action="store_true",
        help="With --virtio-payload, run a longer typed OS request/event pump window before rollback.",
    )
    parser.add_argument(
        "--virtio-wasm-live-os-request-api",
        action="store_true",
        help="With --virtio-payload, prequeue typed OS work through the producer API, then drain it through the sandbox-owner main loop pump.",
    )
    parser.add_argument(
        "--virtio-wasm-live-os-runtime",
        action="store_true",
        help="With --virtio-payload, start the opt-in live OS runtime, prequeue subsystem work, drain it through sandbox owners, then rollback.",
    )
    parser.add_argument(
        "--virtio-wasm-live-os-subsystem-bridge",
        action="store_true",
        help="With --virtio-payload, submit typed work through subsystem-facing producer APIs, drain it through the opt-in sandbox runtime, then rollback.",
    )
    parser.add_argument(
        "--virtio-wasm-live-subsystem-runtime",
        action="store_true",
        help="With --virtio-payload, run a persistent subsystem producer that feeds sandbox-active virtio owners from the ARM64 main loop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-queue",
        action="store_true",
        help="With --virtio-payload, run a normal subsystem inbox queue that forwards work into sandbox-active virtio owners from the ARM64 main loop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-runtime",
        action="store_true",
        help="With --virtio-payload, start the production inbox runtime, submit external inbox work, drain through sandbox owners, and stop through rollback.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-subsystem-source",
        action="store_true",
        help="With --virtio-payload, let ARM64 subsystem sources produce work through virtio_production_request_* and drain it through sandbox owners.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-subsystem-source-live",
        action="store_true",
        help="With --virtio-payload, validate ARM64 subsystem queues feeding production routing, then keep sandbox owners active until stop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-subsystem-source-live-soak",
        action="store_true",
        help="With --virtio-payload, run the longer ARM64 subsystem-source production routing keepalive window before stop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-real-subsystem-source-live",
        action="store_true",
        help="With --virtio-payload, validate detected ARM64 virtio devices feeding production routing, then keep sandbox owners active until stop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-real-subsystem-source-live-soak",
        action="store_true",
        help="With --virtio-payload, run the longer detected-device ARM64 production routing keepalive window before stop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-normal-subsystem-source-live",
        action="store_true",
        help="With --virtio-payload, validate the normal ARM64 subsystem producer boundary feeding production routing, then keep sandbox owners active until stop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-sandbox-source-live",
        action="store_true",
        help="With --virtio-payload, require sandbox virtio drivers to submit production work through the host ABI instead of generated core producers.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-sandbox-source-live-soak",
        action="store_true",
        help="With --virtio-payload, run a longer sandbox-driver source proof window with generated core producers disabled.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-external-normal-source-live",
        action="store_true",
        help="With --virtio-payload, require externally submitted normal subsystem work to drain through the sandbox-owner production path.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-hardware-payload-source-live",
        action="store_true",
        help="With --virtio-payload, route detected-device production descriptors through existing virtio payload submitters, then keep sandbox owners active until stop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-hardware-payload-source-live-device",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, run one device-scoped hardware payload source proof gate.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-hardware-payload-source-live-device-soak",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, run one longer device-scoped hardware payload source proof gate.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-hardware-payload-promote-rollback-device",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, prove a device hardware payload path, promote its sandbox owner, dispatch, then rollback.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-hardware-payload-policy-candidate-device",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, prove a device hardware payload path and mark it as a non-persisted boot-policy candidate.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-hardware-payload-policy-capsule-device",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, prove a device hardware payload path, mark it as a candidate, seal a runtime boot-policy capsule, verify it, then clear it.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-hardware-payload-policy-storage-device",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, prove a device hardware payload path, seal a capsule, stage a storage write-intent, require commit denial without storage capability, then rollback.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-hardware-payload-policy-storage-proof-device",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, prove a device hardware payload path, seal a capsule, attach a volatile sandbox-storage proof capability, commit/verify/clear the policy slot, and keep boot-default live routing disabled.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-hardware-payload-policy-storage-provider-proof-device",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, prove a device hardware payload path, seal a capsule, stage a write-intent, then exercise a sandbox storage provider proof ABI with attach/seal/read/clear while keeping boot-default live routing disabled.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-hardware-payload-policy-storage-provider-restore-proof-device",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, prove provider-backed restore by sealing a policy object, clearing runtime capsule state, restoring it from the provider slot, then read/clear verifying without enabling boot-default live routing.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-external-normal-device-submit-live",
        action="store_true",
        help="With --virtio-payload, submit external normal subsystem work one device at a time through the sandbox-owner production path.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-external-normal-device-submit-soak",
        action="store_true",
        help="With --virtio-payload, repeat device-round external normal subsystem submits through the sandbox-owner production path.",
    )
    parser.add_argument(
        "--external-normal-device-submit-batches",
        type=int,
        default=8,
        help="Number of repeated device-round batches for --virtio-wasm-live-production-external-normal-device-submit-soak.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-normal-subsystem-source-live-soak",
        action="store_true",
        help="With --virtio-payload, run the longer normal ARM64 subsystem producer keepalive window before stop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-sandbox-source-live-device",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, run one device-scoped sandbox-driver source proof gate.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-sandbox-source-live-device-soak",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, run one longer device-scoped sandbox-driver source proof gate.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-real-subsystem-source-live-device",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, run one detected-device ARM64 production routing keepalive gate before stop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-real-subsystem-source-live-device-soak",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, run one longer detected-device ARM64 production routing keepalive gate before stop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-boot-gate",
        action="store_true",
        help="With --virtio-payload, run the opt-in production inbox boot gate from the ARM64 main loop.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-boot-live",
        action="store_true",
        help="With --virtio-payload, run the opt-in production boot gate and keep sandbox owners active after it passes.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-boot-live-soak",
        action="store_true",
        help="With --virtio-payload, run a longer opt-in production boot-live window and keep sandbox owners active after it passes.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-normal-boot-live",
        action="store_true",
        help="With --virtio-payload, run the normal subsystem producer backed boot-live gate and keep sandbox owners active after it passes.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-normal-boot-live-soak",
        action="store_true",
        help="With --virtio-payload, run a longer normal subsystem producer backed boot-live window and keep sandbox owners active after it passes.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-device-boot-gate",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, run one device-specific opt-in production inbox boot gate.",
    )
    parser.add_argument(
        "--virtio-wasm-live-production-device-boot-live",
        choices=("block", "gpu", "net", "input"),
        help="With --virtio-payload, run one device-specific opt-in production boot-live gate and keep that sandbox owner active.",
    )
    parser.add_argument(
        "--production-runtime-batches",
        type=int,
        default=PRODUCTION_RUNTIME_DEFAULT_BATCHES,
        help="Number of /vosprod-all batches to submit when --virtio-wasm-live-production-runtime is active.",
    )
    parser.add_argument(
        "--virtio-wasm-live-manual-subsystem",
        action="store_true",
        help="With --virtio-payload, start the live runtime, feed it through explicit subsystem submit commands, and require main-loop drain plus rollback.",
    )
    parser.add_argument(
        "--virtio-wasm-rust-probe",
        action="store_true",
        help="With --virtio-payload, invoke the Rust-generated Wasm run_skill probe and require it to return.",
    )
    parser.add_argument(
        "--inject-input",
        action="store_true",
        help="With --virtio-payload, open a QEMU HMP monitor and inject a key while the virtio-input queue is armed.",
    )
    parser.add_argument(
        "--inject-net-rx",
        action="store_true",
        help="With --virtio-payload, use a socket netdev and inject raw Ethernet frames into virtio-net RX.",
    )
    args = parser.parse_args()
    if args.virtio_wasm_live_production_hardware_payload_promote_rollback_device is not None:
        args.virtio_wasm_live_production_hardware_payload_source_live_device = (
            args.virtio_wasm_live_production_hardware_payload_promote_rollback_device
        )
    if args.virtio_wasm_live_production_hardware_payload_policy_candidate_device is not None:
        args.virtio_wasm_live_production_hardware_payload_source_live_device = (
            args.virtio_wasm_live_production_hardware_payload_policy_candidate_device
        )
    if args.virtio_wasm_live_production_hardware_payload_policy_capsule_device is not None:
        args.virtio_wasm_live_production_hardware_payload_source_live_device = (
            args.virtio_wasm_live_production_hardware_payload_policy_capsule_device
        )
    if args.virtio_wasm_live_production_hardware_payload_policy_storage_device is not None:
        args.virtio_wasm_live_production_hardware_payload_source_live_device = (
            args.virtio_wasm_live_production_hardware_payload_policy_storage_device
        )
    if (
        args.virtio_wasm_live_production_hardware_payload_policy_storage_proof_device
        is not None
    ):
        args.virtio_wasm_live_production_hardware_payload_source_live_device = (
            args.virtio_wasm_live_production_hardware_payload_policy_storage_proof_device
        )
    if (
        args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_proof_device
        is not None
    ):
        args.virtio_wasm_live_production_hardware_payload_source_live_device = (
            args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_proof_device
        )
    if (
        args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_restore_proof_device
        is not None
    ):
        args.virtio_wasm_live_production_hardware_payload_source_live_device = (
            args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_restore_proof_device
        )

    elf = args.elf.resolve()
    if not elf.exists():
        raise SystemExit(f"ARM64 recovery ELF not found: {elf}")

    if args.virtio_wasm_device_enable_gate == "net":
        args.inject_net_rx = True
    if args.virtio_wasm_device_enable_gate == "input":
        args.inject_input = True
    if args.virtio_wasm_device_live_cutover == "net":
        args.inject_net_rx = True
    if args.virtio_wasm_device_live_cutover == "input":
        args.inject_input = True
    if args.virtio_wasm_live_routing:
        args.inject_net_rx = True
        args.inject_input = True
    if args.virtio_wasm_live_route_all:
        args.inject_net_rx = True
        args.inject_input = True
    if args.virtio_wasm_live_boot_service:
        args.inject_net_rx = True
        args.inject_input = True
    if args.virtio_wasm_live_service_loop:
        args.inject_net_rx = True
        args.inject_input = True
    if (
        args.virtio_wasm_live_os_pump
        or args.virtio_wasm_live_os_pump_long
        or args.virtio_wasm_live_os_request_api
        or args.virtio_wasm_live_os_runtime
        or args.virtio_wasm_live_os_subsystem_bridge
        or args.virtio_wasm_live_subsystem_runtime
        or args.virtio_wasm_live_production_queue
        or args.virtio_wasm_live_production_runtime
        or args.virtio_wasm_live_production_subsystem_source
        or args.virtio_wasm_live_production_subsystem_source_live
        or args.virtio_wasm_live_production_subsystem_source_live_soak
        or args.virtio_wasm_live_production_real_subsystem_source_live
        or args.virtio_wasm_live_production_real_subsystem_source_live_soak
        or args.virtio_wasm_live_production_normal_subsystem_source_live
        or args.virtio_wasm_live_production_sandbox_source_live
        or args.virtio_wasm_live_production_sandbox_source_live_soak
        or args.virtio_wasm_live_production_external_normal_source_live
        or args.virtio_wasm_live_production_hardware_payload_source_live
        or args.virtio_wasm_live_production_hardware_payload_source_live_device is not None
        or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is not None
        or args.virtio_wasm_live_production_external_normal_device_submit_live
        or args.virtio_wasm_live_production_external_normal_device_submit_soak
        or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
        or args.virtio_wasm_live_production_normal_boot_live
        or args.virtio_wasm_live_production_normal_boot_live_soak
        or args.virtio_wasm_live_production_sandbox_source_live_device is not None
        or args.virtio_wasm_live_production_sandbox_source_live_device_soak is not None
        or args.virtio_wasm_live_production_real_subsystem_source_live_device is not None
        or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
        or args.virtio_wasm_live_production_boot_gate
        or args.virtio_wasm_live_production_boot_live
        or args.virtio_wasm_live_production_boot_live_soak
        or args.virtio_wasm_live_production_device_boot_gate is not None
        or args.virtio_wasm_live_production_device_boot_live is not None
        or args.virtio_wasm_live_manual_subsystem
    ):
        args.inject_net_rx = True
        args.inject_input = True

    use_stdio = (
        args.virtio_wasm_injected
        or args.virtio_wasm_promotion_gate
        or args.virtio_wasm_sustained_gate
        or args.virtio_wasm_traffic_window
        or args.virtio_wasm_device_enable_gate is not None
        or args.virtio_wasm_device_live_cutover is not None
        or args.virtio_wasm_live_routing
        or args.virtio_wasm_live_route_all
        or args.virtio_wasm_live_boot_service
        or args.virtio_wasm_live_service_loop
        or args.virtio_wasm_live_os_pump
        or args.virtio_wasm_live_os_pump_long
        or args.virtio_wasm_live_os_request_api
        or args.virtio_wasm_live_os_runtime
        or args.virtio_wasm_live_os_subsystem_bridge
        or args.virtio_wasm_live_subsystem_runtime
        or args.virtio_wasm_live_production_queue
        or args.virtio_wasm_live_production_runtime
        or args.virtio_wasm_live_production_subsystem_source
        or args.virtio_wasm_live_production_subsystem_source_live
        or args.virtio_wasm_live_production_subsystem_source_live_soak
        or args.virtio_wasm_live_production_real_subsystem_source_live
        or args.virtio_wasm_live_production_real_subsystem_source_live_soak
        or args.virtio_wasm_live_production_normal_subsystem_source_live
        or args.virtio_wasm_live_production_sandbox_source_live
        or args.virtio_wasm_live_production_sandbox_source_live_soak
        or args.virtio_wasm_live_production_hardware_payload_source_live
        or args.virtio_wasm_live_production_hardware_payload_source_live_device is not None
        or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is not None
        or args.virtio_wasm_live_production_external_normal_device_submit_live
        or args.virtio_wasm_live_production_external_normal_device_submit_soak
        or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
        or args.virtio_wasm_live_production_normal_boot_live
        or args.virtio_wasm_live_production_normal_boot_live_soak
        or args.virtio_wasm_live_production_sandbox_source_live_device is not None
        or args.virtio_wasm_live_production_sandbox_source_live_device_soak is not None
        or args.virtio_wasm_live_production_real_subsystem_source_live_device is not None
        or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
        or args.virtio_wasm_live_production_boot_gate
        or args.virtio_wasm_live_production_boot_live
        or args.virtio_wasm_live_production_boot_live_soak
        or args.virtio_wasm_live_production_device_boot_gate is not None
        or args.virtio_wasm_live_production_device_boot_live is not None
        or args.virtio_wasm_live_manual_subsystem
    )
    serial_port = None if use_stdio else reserve_tcp_port()
    qmp_port = reserve_tcp_port() if args.inject_input else None
    vnc_display = None
    vnc_guard: socket.socket | None = None
    if args.inject_input:
        vnc_display, vnc_guard = reserve_vnc_display()
    net_rx_port = reserve_tcp_port() if args.inject_net_rx else None
    net_tx_port = reserve_tcp_port() if args.inject_net_rx else None
    command = [
        resolve_qemu(),
        "-machine",
        "virt",
        "-cpu",
        "cortex-a72",
        "-m",
        "1024",
        "-no-reboot",
        "-kernel",
        str(elf),
    ]
    if use_stdio:
        command.extend(["-serial", "stdio"])
    else:
        command.extend(["-serial", f"tcp:127.0.0.1:{serial_port},server"])
    command.extend(["-monitor", "none"])
    if vnc_display is not None:
        command.extend(["-display", f"vnc=127.0.0.1:{vnc_display}"])
    else:
        command.extend(["-display", "none"])
    if qmp_port is not None:
        command.extend(["-qmp", f"tcp:127.0.0.1:{qmp_port},server,nowait"])
    smoke_input = (
        "/help\n"
        "/clear\n"
        "/status\n"
        "/registry-context\n"
        "/skill-cache\n"
        "/driver-host-status\n"
        "/display-status\n"
        "/input-routing-status\n"
        "check arm64 cli parity\n"
    ) if args.cli_baseline else "/status\n/abi-status\n"
    temp_disk = None
    if args.virtio_probe or args.virtio_payload:
        if args.inject_net_rx:
            command.extend(
                [
                    "-netdev",
                    f"socket,id=n1,udp=127.0.0.1:{net_tx_port},localaddr=127.0.0.1:{net_rx_port}",
                    "-device",
                    "virtio-net-device,netdev=n1,mac=52:54:00:12:34:56",
                ]
            )
        else:
            command.extend(
                [
                    "-netdev",
                    "user,id=n1",
                    "-device",
                    "virtio-net-device,netdev=n1,mac=52:54:00:12:34:56",
                ]
            )
    if args.virtio_probe:
        smoke_input += "/virtio-mmio-probe\n"
    if args.virtio_payload:
        if args.virtio_wasm_injected:
            command.extend(["-device", "virtio-mouse-device,id=vinput0"])
        else:
            temp_disk = tempfile.NamedTemporaryFile(prefix="openrhiza-arm64-virtio-", suffix=".img", delete=False)
            temp_disk.truncate(8 * 1024 * 1024)
            temp_disk.close()
            command.extend(
                [
                    "-drive",
                    f"if=none,file={temp_disk.name},format=raw,id=vd0",
                    "-device",
                    "virtio-blk-device,drive=vd0",
                    "-device",
                    "virtio-mouse-device,id=vinput0" if args.inject_input else "virtio-keyboard-device,id=vkbd0",
                    "-device",
                    "virtio-gpu-device,id=video0",
                ]
            )
            if args.virtio_wasm_rust_probe:
                command.extend(["-device", "virtio-rng-device,id=vrng0"])
            smoke_input += "/virtio-payload-selftest\n"
    if args.virtio_wasm and not args.virtio_payload:
        raise SystemExit("--virtio-wasm requires --virtio-payload")
    if args.virtio_wasm_injected and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-injected requires --virtio-payload")
    if args.virtio_wasm_rollback and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-rollback requires --virtio-payload")
    if args.virtio_wasm_promotion_gate and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-promotion-gate requires --virtio-payload")
    if args.virtio_wasm_sustained_gate and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-sustained-gate requires --virtio-payload")
    if args.virtio_wasm_traffic_window and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-traffic-window requires --virtio-payload")
    if args.virtio_wasm_device_enable_gate and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-device-enable-gate requires --virtio-payload")
    if args.virtio_wasm_device_live_cutover and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-device-live-cutover requires --virtio-payload")
    if args.virtio_wasm_live_routing and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-routing requires --virtio-payload")
    if args.virtio_wasm_live_route_all and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-route-all requires --virtio-payload")
    if args.virtio_wasm_live_boot_service and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-boot-service requires --virtio-payload")
    if args.virtio_wasm_live_service_loop and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-service-loop requires --virtio-payload")
    if args.virtio_wasm_live_os_pump and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-os-pump requires --virtio-payload")
    if args.virtio_wasm_live_os_pump_long and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-os-pump-long requires --virtio-payload")
    if args.virtio_wasm_live_os_request_api and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-os-request-api requires --virtio-payload")
    if args.virtio_wasm_live_os_runtime and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-os-runtime requires --virtio-payload")
    if args.virtio_wasm_live_os_subsystem_bridge and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-os-subsystem-bridge requires --virtio-payload")
    if args.virtio_wasm_live_subsystem_runtime and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-subsystem-runtime requires --virtio-payload")
    if args.virtio_wasm_live_production_queue and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-queue requires --virtio-payload")
    if args.virtio_wasm_live_production_runtime and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-runtime requires --virtio-payload")
    if args.virtio_wasm_live_production_subsystem_source and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-subsystem-source requires --virtio-payload")
    if args.virtio_wasm_live_production_subsystem_source_live and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-subsystem-source-live requires --virtio-payload")
    if args.virtio_wasm_live_production_subsystem_source_live_soak and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-subsystem-source-live-soak requires --virtio-payload")
    if args.virtio_wasm_live_production_real_subsystem_source_live and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-real-subsystem-source-live requires --virtio-payload")
    if args.virtio_wasm_live_production_real_subsystem_source_live_soak and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-real-subsystem-source-live-soak requires --virtio-payload")
    if args.virtio_wasm_live_production_normal_subsystem_source_live and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-normal-subsystem-source-live requires --virtio-payload")
    if args.virtio_wasm_live_production_sandbox_source_live and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-sandbox-source-live requires --virtio-payload")
    if args.virtio_wasm_live_production_sandbox_source_live_soak and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-sandbox-source-live-soak requires --virtio-payload")
    if args.virtio_wasm_live_production_external_normal_source_live and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-external-normal-source-live requires --virtio-payload")
    if args.virtio_wasm_live_production_hardware_payload_source_live and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-hardware-payload-source-live requires --virtio-payload")
    if args.virtio_wasm_live_production_hardware_payload_source_live_device and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-hardware-payload-source-live-device requires --virtio-payload")
    if args.virtio_wasm_live_production_hardware_payload_source_live_device_soak and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-hardware-payload-source-live-device-soak requires --virtio-payload")
    if args.virtio_wasm_live_production_external_normal_device_submit_live and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-external-normal-device-submit-live requires --virtio-payload")
    if args.virtio_wasm_live_production_external_normal_device_submit_soak and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-external-normal-device-submit-soak requires --virtio-payload")
    if args.virtio_wasm_live_production_normal_subsystem_source_live_soak and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-normal-subsystem-source-live-soak requires --virtio-payload")
    if args.virtio_wasm_live_production_sandbox_source_live_device and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-sandbox-source-live-device requires --virtio-payload")
    if args.virtio_wasm_live_production_sandbox_source_live_device_soak and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-sandbox-source-live-device-soak requires --virtio-payload")
    if args.virtio_wasm_live_production_real_subsystem_source_live_device and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-real-subsystem-source-live-device requires --virtio-payload")
    if args.virtio_wasm_live_production_real_subsystem_source_live_device_soak and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-real-subsystem-source-live-device-soak requires --virtio-payload")
    if args.virtio_wasm_live_production_boot_gate and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-boot-gate requires --virtio-payload")
    if args.virtio_wasm_live_production_boot_live and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-boot-live requires --virtio-payload")
    if args.virtio_wasm_live_production_boot_live_soak and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-boot-live-soak requires --virtio-payload")
    if args.virtio_wasm_live_production_normal_boot_live and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-normal-boot-live requires --virtio-payload")
    if args.virtio_wasm_live_production_normal_boot_live_soak and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-normal-boot-live-soak requires --virtio-payload")
    if args.virtio_wasm_live_production_device_boot_gate and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-device-boot-gate requires --virtio-payload")
    if args.virtio_wasm_live_production_device_boot_live and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-production-device-boot-live requires --virtio-payload")
    if args.production_runtime_batches < 1:
        raise SystemExit("--production-runtime-batches must be at least 1")
    if args.external_normal_device_submit_batches < 1:
        raise SystemExit("--external-normal-device-submit-batches must be at least 1")
    if args.virtio_wasm_live_manual_subsystem and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-live-manual-subsystem requires --virtio-payload")
    if args.virtio_wasm_rust_probe and not args.virtio_payload:
        raise SystemExit("--virtio-wasm-rust-probe requires --virtio-payload")
    if args.virtio_wasm_injected and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-injected requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_promotion_gate and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-promotion-gate requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_sustained_gate and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-sustained-gate requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_traffic_window and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-traffic-window requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_routing and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-routing requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_route_all and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-route-all requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_boot_service and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-boot-service requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_service_loop and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-service-loop requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_os_pump and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-os-pump requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_os_pump_long and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-os-pump-long requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_os_request_api and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-os-request-api requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_os_runtime and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-os-runtime requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_os_subsystem_bridge and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-os-subsystem-bridge requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_subsystem_runtime and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-subsystem-runtime requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_queue and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-queue requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_runtime and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-runtime requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_subsystem_source and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-subsystem-source requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_subsystem_source_live and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-subsystem-source-live requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_subsystem_source_live_soak and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-subsystem-source-live-soak requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_real_subsystem_source_live and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-real-subsystem-source-live requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_real_subsystem_source_live_soak and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-real-subsystem-source-live-soak requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_normal_subsystem_source_live and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-normal-subsystem-source-live requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_sandbox_source_live and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-sandbox-source-live requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_sandbox_source_live_soak and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-sandbox-source-live-soak requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_external_normal_source_live and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-external-normal-source-live requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_hardware_payload_source_live and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-hardware-payload-source-live requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_hardware_payload_source_live_device and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-hardware-payload-source-live-device requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_hardware_payload_source_live_device_soak and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-hardware-payload-source-live-device-soak requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_external_normal_device_submit_live and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-external-normal-device-submit-live requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_external_normal_device_submit_soak and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-external-normal-device-submit-soak requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_normal_subsystem_source_live_soak and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-normal-subsystem-source-live-soak requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_sandbox_source_live_device and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-sandbox-source-live-device requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_sandbox_source_live_device_soak and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-sandbox-source-live-device-soak requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_real_subsystem_source_live_device and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-real-subsystem-source-live-device requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_real_subsystem_source_live_device_soak and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-real-subsystem-source-live-device-soak requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_boot_gate and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-boot-gate requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_boot_live and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-boot-live requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_boot_live_soak and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-boot-live-soak requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_normal_boot_live and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-normal-boot-live requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_normal_boot_live_soak and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-normal-boot-live-soak requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_device_boot_gate and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-device-boot-gate requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_production_device_boot_live and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-production-device-boot-live requires --inject-input and --inject-net-rx")
    if args.virtio_wasm_live_manual_subsystem and not (args.inject_input and args.inject_net_rx):
        raise SystemExit("--virtio-wasm-live-manual-subsystem requires --inject-input and --inject-net-rx")
    if args.inject_input and not args.virtio_payload:
        raise SystemExit("--inject-input requires --virtio-payload")
    if args.inject_net_rx and not args.virtio_payload:
        raise SystemExit("--inject-net-rx requires --virtio-payload")
    if args.virtio_wasm_rust_probe:
        # The Windows QEMU TCP serial backend is not reliable for injecting
        # PL011 input after boot. A test-only virtio-rng marker requests the
        # same diagnostic at boot without enabling this path in normal boots.
        smoke_input = ""
    elif args.virtio_wasm_injected:
        # This path intentionally exercises only the tiny hand-authored
        # executor import. The Rust-generated wasmi::TypedFunc::call() path
        # has a separate marker-gated smoke so injected tests stay focused.
        smoke_input = "/virtio-wasm-injected-selftest\n"
    elif args.virtio_wasm_promotion_gate:
        smoke_input = "/virtio-wasm-promotion-gate-selftest\n"
    elif args.virtio_wasm_sustained_gate:
        smoke_input = "/virtio-wasm-sustained-gate-selftest\n"
    elif args.virtio_wasm_traffic_window:
        smoke_input = "/virtio-wasm-traffic-window-selftest\n"
    elif args.virtio_wasm_device_enable_gate:
        smoke_input = f"/virtio-wasm-enable-gate-{args.virtio_wasm_device_enable_gate}\n"
    elif args.virtio_wasm_device_live_cutover:
        smoke_input = f"/virtio-wasm-live-cutover-rollback-{args.virtio_wasm_device_live_cutover}\n"
    elif args.virtio_wasm_live_routing:
        smoke_input = "/virtio-wasm-live-routing-selftest\n"
    elif args.virtio_wasm_live_route_all:
        smoke_input = "/virtio-wasm-live-route-all-selftest\n"
    elif args.virtio_wasm_live_boot_service:
        smoke_input = "/virtio-wasm-live-boot-service-selftest\n"
    elif args.virtio_wasm_live_service_loop:
        smoke_input = "/virtio-wasm-live-service-loop-selftest\n"
    elif args.virtio_wasm_live_os_pump:
        smoke_input = "/virtio-wasm-live-os-pump-selftest\n"
    elif args.virtio_wasm_live_os_pump_long:
        smoke_input = "/vospl\n"
    elif args.virtio_wasm_live_os_request_api:
        smoke_input = "/vosrq\n"
    elif args.virtio_wasm_live_os_runtime:
        smoke_input = "/voslive\n"
    elif args.virtio_wasm_live_os_subsystem_bridge:
        smoke_input = "/vossys\n"
    elif args.virtio_wasm_live_subsystem_runtime:
        smoke_input = "/vosps\n"
    elif args.virtio_wasm_live_production_queue:
        smoke_input = "/vosprod\n"
    elif args.virtio_wasm_live_production_boot_gate:
        smoke_input = "/vosboot\n"
    elif args.virtio_wasm_live_production_boot_live:
        smoke_input = "/vosboot-live\n"
    elif args.virtio_wasm_live_production_boot_live_soak:
        smoke_input = "/vosboot-live-soak\n"
    elif args.virtio_wasm_live_production_normal_boot_live:
        smoke_input = "/vosboot-normal-live\n/vosstat\n"
    elif args.virtio_wasm_live_production_normal_boot_live_soak:
        smoke_input = "/vosboot-normal-live-soak\n/vosstat\n"
    elif args.virtio_wasm_live_production_device_boot_gate:
        smoke_input = f"/vosboot-{args.virtio_wasm_live_production_device_boot_gate}\n"
    elif args.virtio_wasm_live_production_device_boot_live:
        smoke_input = f"/vosboot-live-{args.virtio_wasm_live_production_device_boot_live}\n"
    elif args.virtio_wasm_live_production_runtime:
        smoke_input = "/vosprod-start\n"
        smoke_input += "/vosprod-all\n" * args.production_runtime_batches
        smoke_input += "/vosstat\n/vosstop\n"
    elif args.virtio_wasm_live_production_subsystem_source:
        smoke_input = "/vosprod-source\n"
    elif args.virtio_wasm_live_production_subsystem_source_live:
        smoke_input = "/vosprod-source-live\n/vosstat\n/stop\n"
    elif args.virtio_wasm_live_production_subsystem_source_live_soak:
        smoke_input = "/vosprod-source-live-soak\n/vosstat\n/stop\n"
    elif args.virtio_wasm_live_production_real_subsystem_source_live:
        smoke_input = "/vosprod-real-source-live\n/vosstat\n/stop\n"
    elif args.virtio_wasm_live_production_real_subsystem_source_live_soak:
        smoke_input = "/vosprod-real-source-live-soak\n/vosstat\n/stop\n"
    elif args.virtio_wasm_live_production_normal_subsystem_source_live:
        smoke_input = "/vosprod-normal-source-live\n/vosstat\n/stop\n"
    elif args.virtio_wasm_live_production_sandbox_source_live:
        smoke_input = "/vosprod-sandbox-source-live\n/vosstat\n/stop\n"
    elif args.virtio_wasm_live_production_sandbox_source_live_soak:
        smoke_input = "/vosprod-sandbox-source-live-soak\n/vosstat\n/stop\n"
    elif args.virtio_wasm_live_production_hardware_payload_source_live:
        smoke_input = "/vosprod-payload-source-live\n/vosstat\n/stop\n"
    elif args.virtio_wasm_live_production_hardware_payload_promote_rollback_device:
        device = args.virtio_wasm_live_production_hardware_payload_promote_rollback_device
        smoke_input = (
            f"/vosprod-payload-source-live-{device}\n"
            "/vosstat\n"
            f"/vospromote-payload-{device}\n"
            f"/virtio-wasm-live-dispatch-{device}\n"
            f"/virtio-wasm-live-rollback-{device}\n"
            "/vosstat\n"
            "/stop\n"
        )
    elif args.virtio_wasm_live_production_hardware_payload_policy_candidate_device:
        device = args.virtio_wasm_live_production_hardware_payload_policy_candidate_device
        smoke_input = (
            f"/vosprod-payload-source-live-{device}\n"
            "/vosstat\n"
            f"/vospolicy-payload-{device}\n"
            "/vosstat\n"
            "/vospolicy-payload-clear\n"
            "/vosstat\n"
            "/stop\n"
        )
    elif args.virtio_wasm_live_production_hardware_payload_policy_capsule_device:
        device = args.virtio_wasm_live_production_hardware_payload_policy_capsule_device
        smoke_input = (
            f"/vosprod-payload-source-live-{device}\n"
            "/vosstat\n"
            f"/vospolicy-payload-{device}\n"
            f"/vospolicy-capsule-seal-{device}\n"
            "/vospolicy-capsule-verify\n"
            "/vospolicy-capsule-clear\n"
            "/vosstat\n"
            "/stop\n"
        )
    elif (
        args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_restore_proof_device
    ):
        device = (
            args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_restore_proof_device
        )
        smoke_input = (
            f"/vosprod-payload-source-live-{device}\n"
            "/vosstat\n"
            f"/vospolicy-payload-{device}\n"
            f"/vospolicy-capsule-seal-{device}\n"
            "/vospolicy-capsule-verify\n"
            "/vospolicy-storage-stage\n"
            "/vospolicy-storage-provider-attach-proof\n"
            "/vospolicy-storage-provider-seal-proof\n"
            "/vospolicy-capsule-clear\n"
            "/vospolicy-storage-provider-restore-proof\n"
            "/vospolicy-storage-provider-read-proof\n"
            "/vospolicy-storage-provider-clear-proof\n"
            "/vosstat\n"
            "/stop\n"
        )
    elif (
        args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_proof_device
    ):
        device = (
            args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_proof_device
        )
        smoke_input = (
            f"/vosprod-payload-source-live-{device}\n"
            "/vosstat\n"
            f"/vospolicy-payload-{device}\n"
            f"/vospolicy-capsule-seal-{device}\n"
            "/vospolicy-capsule-verify\n"
            "/vospolicy-storage-stage\n"
            "/vospolicy-storage-provider-attach-proof\n"
            "/vospolicy-storage-provider-seal-proof\n"
            "/vospolicy-storage-provider-read-proof\n"
            "/vospolicy-storage-provider-clear-proof\n"
            "/vosstat\n"
            "/stop\n"
        )
    elif args.virtio_wasm_live_production_hardware_payload_policy_storage_proof_device:
        device = args.virtio_wasm_live_production_hardware_payload_policy_storage_proof_device
        smoke_input = (
            f"/vosprod-payload-source-live-{device}\n"
            "/vosstat\n"
            f"/vospolicy-payload-{device}\n"
            f"/vospolicy-capsule-seal-{device}\n"
            "/vospolicy-capsule-verify\n"
            "/vospolicy-storage-stage\n"
            "/vospolicy-storage-attach-proof\n"
            "/vospolicy-storage-commit\n"
            "/vospolicy-storage-verify\n"
            "/vospolicy-storage-clear-proof\n"
            "/vosstat\n"
            "/stop\n"
        )
    elif args.virtio_wasm_live_production_hardware_payload_policy_storage_device:
        device = args.virtio_wasm_live_production_hardware_payload_policy_storage_device
        smoke_input = (
            f"/vosprod-payload-source-live-{device}\n"
            "/vosstat\n"
            f"/vospolicy-payload-{device}\n"
            f"/vospolicy-capsule-seal-{device}\n"
            "/vospolicy-capsule-verify\n"
            "/vospolicy-storage-stage\n"
            "/vospolicy-storage-commit\n"
            "/vospolicy-storage-rollback\n"
            "/vosstat\n"
            "/stop\n"
        )
    elif args.virtio_wasm_live_production_hardware_payload_source_live_device:
        smoke_input = (
            f"/vosprod-payload-source-live-{args.virtio_wasm_live_production_hardware_payload_source_live_device}\n"
            "/vosstat\n/stop\n"
        )
    elif args.virtio_wasm_live_production_hardware_payload_source_live_device_soak:
        smoke_input = (
            f"/vosprod-payload-source-live-soak-{args.virtio_wasm_live_production_hardware_payload_source_live_device_soak}\n"
            "/vosstat\n/stop\n"
        )
    elif args.virtio_wasm_live_production_external_normal_source_live:
        smoke_input = (
            "/vosprod-external-normal-source-live\n"
            "/vosprod-external-normal-submit-all\n"
            "/vosstat\n"
            "/stop\n"
        )
    elif args.virtio_wasm_live_production_external_normal_device_submit_live:
        smoke_input = (
            "/vosprod-external-normal-source-live\n"
            "/vosprod-external-normal-submit-block\n"
            "/vosprod-external-normal-submit-gpu\n"
            "/vosprod-external-normal-submit-net\n"
            "/vosprod-external-normal-submit-input\n"
            "/vosstat\n"
            "/stop\n"
        )
    elif args.virtio_wasm_live_production_external_normal_device_submit_soak:
        rounds = "/vosprod-external-normal-submit-device-round\n" * args.external_normal_device_submit_batches
        smoke_input = (
            "/vosprod-external-normal-source-live\n"
            f"{rounds}"
            "/vosstat\n"
            "/stop\n"
        )
    elif args.virtio_wasm_live_production_normal_subsystem_source_live_soak:
        smoke_input = "/vosprod-normal-source-live-soak\n/vosstat\n/stop\n"
    elif args.virtio_wasm_live_production_sandbox_source_live_device:
        smoke_input = (
            f"/vosprod-sandbox-source-live-{args.virtio_wasm_live_production_sandbox_source_live_device}\n"
            "/vosstat\n/stop\n"
        )
    elif args.virtio_wasm_live_production_sandbox_source_live_device_soak:
        smoke_input = (
            f"/vosprod-sandbox-source-live-soak-{args.virtio_wasm_live_production_sandbox_source_live_device_soak}\n"
            "/vosstat\n/stop\n"
        )
    elif args.virtio_wasm_live_production_real_subsystem_source_live_device:
        smoke_input = (
            f"/vosprod-real-source-live-{args.virtio_wasm_live_production_real_subsystem_source_live_device}\n"
            "/vosstat\n/stop\n"
        )
    elif args.virtio_wasm_live_production_real_subsystem_source_live_device_soak:
        smoke_input = (
            f"/vosprod-real-source-live-soak-{args.virtio_wasm_live_production_real_subsystem_source_live_device_soak}\n"
            "/vosstat\n/stop\n"
        )
    elif args.virtio_wasm_live_manual_subsystem:
        smoke_input = "/vosman\n/vossall\n/vossall\n/vossall\n/vossall\n"
    elif args.virtio_wasm_rollback:
        smoke_input = "/virtio-wasm-rollback-selftest\n"
    elif args.virtio_wasm:
        # Multi-device ARM64 payload selftest autoruns at boot. Keep the
        # follow-up script short so PL011 TCP input does not need to be
        # reliable for this smoke.
        smoke_input = "/virtio-wasm-selftest\n"

    output = ""
    collection_error: Exception | None = None
    try:
        if vnc_guard is not None:
            vnc_guard.close()
            vnc_guard = None
        proc = subprocess.Popen(
            command,
            cwd=REPO_ROOT,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            errors="replace",
        )
        try:
            injector = None
            vnc_injector = None
            net_injector = None
            if (
                args.virtio_wasm_traffic_window
                or args.virtio_wasm_device_enable_gate is not None
                or args.virtio_wasm_device_live_cutover is not None
                or args.virtio_wasm_live_routing
                or args.virtio_wasm_live_route_all
                or args.virtio_wasm_live_boot_service
                or args.virtio_wasm_live_service_loop
                or args.virtio_wasm_live_os_pump
                or args.virtio_wasm_live_os_pump_long
                or args.virtio_wasm_live_os_request_api
                or args.virtio_wasm_live_os_runtime
                or args.virtio_wasm_live_os_subsystem_bridge
                or args.virtio_wasm_live_subsystem_runtime
                or args.virtio_wasm_live_production_queue
                or args.virtio_wasm_live_production_runtime
                or args.virtio_wasm_live_production_subsystem_source
                or args.virtio_wasm_live_production_subsystem_source_live
                or args.virtio_wasm_live_production_subsystem_source_live_soak
                or args.virtio_wasm_live_production_real_subsystem_source_live
                or args.virtio_wasm_live_production_real_subsystem_source_live_soak
                or args.virtio_wasm_live_production_normal_subsystem_source_live
                or args.virtio_wasm_live_production_sandbox_source_live
        or args.virtio_wasm_live_production_hardware_payload_source_live
        or args.virtio_wasm_live_production_hardware_payload_source_live_device is not None
        or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is not None
        or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
                or args.virtio_wasm_live_production_normal_boot_live
                or args.virtio_wasm_live_production_normal_boot_live_soak
                or args.virtio_wasm_live_production_real_subsystem_source_live_device is not None
                or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
                or args.virtio_wasm_live_production_boot_gate
                or args.virtio_wasm_live_production_boot_live
                or args.virtio_wasm_live_production_boot_live_soak
                or args.virtio_wasm_live_production_device_boot_gate is not None
                or args.virtio_wasm_live_production_device_boot_live is not None
                or args.virtio_wasm_live_manual_subsystem
            ):
                inject_duration = max(args.timeout - 5.0, 90.0)
            elif args.virtio_wasm_sustained_gate:
                inject_duration = max(args.timeout - 5.0, 60.0)
            else:
                inject_duration = 60.0 if args.virtio_wasm_promotion_gate else (18.0 if args.virtio_wasm_injected else 5.0)
            if args.inject_input and qmp_port is not None:
                injector = start_qmp_pointer_injector(qmp_port, duration=inject_duration)
            if args.inject_input and vnc_display is not None:
                vnc_injector = start_vnc_input_injector(vnc_display, duration=inject_duration)
            if args.inject_net_rx and net_rx_port is not None:
                net_injector = start_udp_net_injector(net_rx_port, duration=inject_duration)
            try:
                if use_stdio:
                    output = collect_stdio(
                        proc,
                        smoke_input,
                        args.timeout,
                        qmp_port=qmp_port,
                        inject_input=args.inject_input,
                    )
                else:
                    assert serial_port is not None
                    output = collect_serial(
                        serial_port,
                        smoke_input,
                        args.timeout,
                        qmp_port=qmp_port,
                        inject_input=args.inject_input,
                    )
            except Exception as exc:
                collection_error = exc
                output += f"\n[harness] collection-error {exc}\n"
            if injector is not None:
                injector.join(timeout=0.25)
            if vnc_injector is not None:
                vnc_injector.join(timeout=0.25)
            if net_injector is not None:
                net_injector.join(timeout=0.25)
        finally:
            proc.terminate()
            try:
                qemu_output, _ = proc.communicate(timeout=1.0)
            except subprocess.TimeoutExpired:
                proc.kill()
                qemu_output, _ = proc.communicate(timeout=1.0)
            if qemu_output:
                output += qemu_output
            if INJECT_NOTES:
                output += "\n".join(f"[harness] {note}" for note in INJECT_NOTES) + "\n"
    except Exception as exc:
        output += f"\n[harness] launch-error {exc}\n"
    finally:
        if vnc_guard is not None:
            vnc_guard.close()
        if temp_disk is not None:
            try:
                Path(temp_disk.name).unlink(missing_ok=True)
            except OSError:
                pass

    print(output)
    if collection_error is not None:
        return 1
    if "OpenRhiza ARM64 recovery core" not in output or not has_arm64_prompt(output):
        return 1
    if args.cli_baseline:
        for marker in (
            "OpenRhiza ARM64 recovery CLI",
            "Local survival commands:",
            "[Registry] internal console handoff is present",
            "[Skill Cache] ARM64 recovery knows seed skill names only",
            "[ARM64 ABI] bounded platform identity probes only",
            "[Display] recovery surface=PL011 serial",
            "[Input] recovery input=PL011 serial",
            "[Prompt] internal console handoff is required",
        ):
            if marker not in output:
                return validation_failed(f"missing ARM64 CLI marker: {marker}")
    if args.virtio_probe and "read-only virtio-mmio identity probe begins" not in output:
        return 1
    if (
        args.virtio_payload
        and not args.virtio_wasm
        and not args.virtio_wasm_injected
        and not args.virtio_wasm_rollback
        and not args.virtio_wasm_promotion_gate
        and not args.virtio_wasm_sustained_gate
        and not args.virtio_wasm_traffic_window
        and args.virtio_wasm_device_enable_gate is None
        and args.virtio_wasm_device_live_cutover is None
        and not args.virtio_wasm_live_routing
        and not args.virtio_wasm_live_route_all
        and not args.virtio_wasm_live_boot_service
        and not args.virtio_wasm_live_service_loop
        and not args.virtio_wasm_live_os_pump
        and not args.virtio_wasm_live_os_pump_long
        and not args.virtio_wasm_live_os_request_api
        and not args.virtio_wasm_live_os_runtime
        and not args.virtio_wasm_live_os_subsystem_bridge
        and not args.virtio_wasm_live_subsystem_runtime
        and not args.virtio_wasm_live_production_queue
        and not args.virtio_wasm_live_production_runtime
        and not args.virtio_wasm_live_production_subsystem_source
        and not args.virtio_wasm_live_production_subsystem_source_live
        and not args.virtio_wasm_live_production_subsystem_source_live_soak
        and not args.virtio_wasm_live_production_real_subsystem_source_live
        and not args.virtio_wasm_live_production_real_subsystem_source_live_soak
        and not args.virtio_wasm_live_production_normal_subsystem_source_live
        and not args.virtio_wasm_live_production_sandbox_source_live
        and not args.virtio_wasm_live_production_sandbox_source_live_soak
        and not args.virtio_wasm_live_production_external_normal_source_live
        and not args.virtio_wasm_live_production_hardware_payload_source_live
        and args.virtio_wasm_live_production_hardware_payload_source_live_device is None
        and args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is None
        and not args.virtio_wasm_live_production_external_normal_device_submit_live
        and not args.virtio_wasm_live_production_external_normal_device_submit_soak
        and not args.virtio_wasm_live_production_normal_subsystem_source_live_soak
        and not args.virtio_wasm_live_production_normal_boot_live
        and not args.virtio_wasm_live_production_normal_boot_live_soak
        and args.virtio_wasm_live_production_sandbox_source_live_device is None
        and args.virtio_wasm_live_production_sandbox_source_live_device_soak is None
        and args.virtio_wasm_live_production_real_subsystem_source_live_device is None
        and args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is None
        and not args.virtio_wasm_live_production_boot_gate
        and not args.virtio_wasm_live_production_boot_live
        and not args.virtio_wasm_live_production_boot_live_soak
        and args.virtio_wasm_live_production_device_boot_gate is None
        and args.virtio_wasm_live_production_device_boot_live is None
        and not args.virtio_wasm_live_manual_subsystem
        and not args.virtio_wasm_rust_probe
        and "virtio payload selftest complete" not in output
    ):
        return 1
    if args.virtio_wasm and "virtio sandbox runtime selftest complete" not in output:
        return 1
    if args.virtio_wasm_injected and "virtio injected sandbox runtime selftest complete" not in output:
        return 1
    if args.virtio_wasm_injected and "tiny injected payload smoke returned 1" not in output:
        return 1
    if args.virtio_wasm_rollback and "virtio rollback sandbox runtime selftest complete" not in output:
        return 1
    if args.virtio_wasm_rollback and "tiny rollback payload smoke returned 1" not in output:
        return 1
    if args.virtio_wasm_rollback and "rollback cycle=10 result=1" not in output:
        return 1
    if args.virtio_wasm_promotion_gate and "virtio promotion-gate sandbox runtime selftest complete" not in output:
        return 1
    if args.virtio_wasm_promotion_gate and "all device-specific promotion gates passed" not in output:
        return 1
    if args.virtio_wasm_promotion_gate and "promotion gate block=1 gpu=1 net=1 input=1" not in output:
        return 1
    if args.virtio_wasm_promotion_gate and "promotion gate full-seed net=1 input=1" not in output:
        return 1
    if args.virtio_wasm_sustained_gate and "virtio sustained rollback-window gate complete" not in output:
        return 1
    if args.virtio_wasm_sustained_gate and "sustained gate rounds=5 passed=1" not in output:
        return 1
    if args.virtio_wasm_sustained_gate and "promotion gate full-seed net=1 input=1" not in output:
        return 1
    if args.virtio_wasm_traffic_window and "virtio traffic-counter window complete" not in output:
        return 1
    if args.virtio_wasm_traffic_window and "traffic window rounds=12 passed=1" not in output:
        return 1
    if args.virtio_wasm_traffic_window and "traffic counters block_read=12 block_write=12 gpu_control=12 net_rx=12 net_tx=12 input_events=12" not in output:
        return 1
    if args.virtio_wasm_device_enable_gate:
        device = args.virtio_wasm_device_enable_gate
        if f"virtio {device} device enable gate complete" not in output:
            return 1
        if f"{device} enable gate rounds=12 passed=1" not in output:
            return 1
        expected_counters = {
            "block": "device gate block counters block_read=12 block_write=12",
            "gpu": "device gate gpu counters gpu_control=12",
            "net": "device gate net counters net_rx=12 net_tx=12",
            "input": "device gate input counters input_events=12",
        }
        if expected_counters[device] not in output:
            return 1
    if args.virtio_wasm_device_live_cutover:
        device = args.virtio_wasm_device_live_cutover
        if f"virtio {device} live cutover+rollback selftest complete" not in output:
            return 1
        if f"virtio {device} live cutover complete" not in output:
            return 1
        if f"virtio {device} live rollback complete" not in output:
            return 1
        if f"{device} owner active after cutover=1" not in output:
            return 1
        if f"{device} owner active after rollback=0" not in output:
            return 1
        if f"{device} rollback proof=1" not in output:
            return 1
    if args.virtio_wasm_live_routing:
        if "virtio production-routing diagnostic complete" not in output:
            return 1
        if "production live routing diagnostic passed=1" not in output:
            return 1
        for device in ("block", "gpu", "net", "input"):
            if f"route selftest {device} passed=1" not in output:
                return 1
            if f"{device} owner active after routing rollback=0" not in output:
                return 1
        if "live dispatch counters block_ok=4 block_fail=0 gpu_ok=4 gpu_fail=0 net_ok=4 net_fail=0 input_ok=4 input_fail=0" not in output:
            return 1
    if args.virtio_wasm_live_route_all:
        if "virtio production route-all selftest complete" not in output:
            return 1
        if "production route-all selftest passed=1" not in output:
            return 1
        if "route-all owner mask active=1" not in output:
            return 1
        if "route-all dispatch passed=1" not in output:
            return 1
        if "route-all owner mask after rollback=0" not in output:
            return 1
        if "live dispatch counters block_ok=4 block_fail=0 gpu_ok=4 gpu_fail=0 net_ok=4 net_fail=0 input_ok=4 input_fail=0" not in output:
            return 1
    if args.virtio_wasm_live_boot_service:
        if "virtio live boot service selftest complete" not in output:
            return 1
        if "virtio live boot service selftest passed=1" not in output:
            return 1
        if "live service started=1" not in output:
            return 1
        if "live service window ticks=4 passed=1" not in output:
            return 1
        if "live service stopped cleanly=1" not in output:
            return 1
        if "live service running after stop=0" not in output:
            return 1
        if "live dispatch counters block_ok=4 block_fail=0 gpu_ok=4 gpu_fail=0 net_ok=4 net_fail=0 input_ok=4 input_fail=0" not in output:
            return 1
        if "live request queue block_pending=0 gpu_pending=0 net_pending=0 input_pending=0 enqueued=16 drained=16" not in output:
            return 1
    if args.virtio_wasm_live_service_loop:
        if "virtio live service mainloop selftest complete" not in output:
            return 1
        if "virtio live service mainloop selftest passed=1" not in output:
            return 1
        if "live service mainloop selftest armed=1" not in output:
            return 1
        if "command returned; ARM64 main loop must pump service ticks." not in output:
            return 1
        if "live service mainloop target ticks=4 queue_ok=1 counters_ok=1" not in output:
            return 1
        if "live service running after stop=0" not in output:
            return 1
        if "live dispatch counters block_ok=4 block_fail=0 gpu_ok=4 gpu_fail=0 net_ok=4 net_fail=0 input_ok=4 input_fail=0" not in output:
            return 1
        if "live request queue block_pending=0 gpu_pending=0 net_pending=0 input_pending=0 enqueued=16 drained=16" not in output:
            return 1
    if args.virtio_wasm_live_production_runtime:
        expected_batches = args.production_runtime_batches
        expected_items = expected_batches * 6
        if "production inbox runtime active=1" not in output:
            return 1
        if "command returned; external production inbox submits can now feed sandbox owners." not in output:
            return 1
        if output.count("production inbox submit-all accepted=6") < expected_batches:
            return 1
        if output.count("production subsystem inbox accepted=0 forwarded=6") < expected_batches:
            return 1
        expected_counters = (
            f"live OS pump counters block_read={expected_batches} "
            f"block_write={expected_batches} gpu_control={expected_batches} "
            f"net_rx={expected_batches} net_tx={expected_batches} "
            f"input_events={expected_batches} failures=0"
        )
        if expected_counters not in output:
            return 1
        expected_inbox = (
            "production subsystem inbox block_read_pending=0 block_write_pending=0 "
            "gpu_control_pending=0 net_rx_pending=0 net_tx_pending=0 "
            f"input_event_pending=0 enqueued={expected_items} "
            f"forwarded={expected_items} rejected=0"
        )
        if expected_inbox not in output:
            return 1
        if "virtio live service stop complete" not in output:
            return 1
        if "live service running after stop=0" not in output:
            return 1
    if (
        args.virtio_wasm_live_os_pump
        or args.virtio_wasm_live_os_pump_long
        or args.virtio_wasm_live_os_request_api
        or args.virtio_wasm_live_os_runtime
        or args.virtio_wasm_live_os_subsystem_bridge
        or args.virtio_wasm_live_subsystem_runtime
        or args.virtio_wasm_live_production_queue
        or args.virtio_wasm_live_production_subsystem_source
        or args.virtio_wasm_live_production_subsystem_source_live
        or args.virtio_wasm_live_production_subsystem_source_live_soak
        or args.virtio_wasm_live_production_real_subsystem_source_live
        or args.virtio_wasm_live_production_real_subsystem_source_live_soak
        or args.virtio_wasm_live_production_normal_subsystem_source_live
        or args.virtio_wasm_live_production_sandbox_source_live
        or args.virtio_wasm_live_production_sandbox_source_live_soak
        or args.virtio_wasm_live_production_external_normal_source_live
        or args.virtio_wasm_live_production_hardware_payload_source_live
        or args.virtio_wasm_live_production_hardware_payload_source_live_device is not None
        or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is not None
        or args.virtio_wasm_live_production_external_normal_device_submit_live
        or args.virtio_wasm_live_production_external_normal_device_submit_soak
        or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
        or args.virtio_wasm_live_production_normal_boot_live
        or args.virtio_wasm_live_production_normal_boot_live_soak
        or args.virtio_wasm_live_production_sandbox_source_live_device is not None
        or args.virtio_wasm_live_production_sandbox_source_live_device_soak is not None
        or args.virtio_wasm_live_production_real_subsystem_source_live_device is not None
        or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
        or args.virtio_wasm_live_production_boot_gate
        or args.virtio_wasm_live_production_boot_live
        or args.virtio_wasm_live_production_boot_live_soak
        or args.virtio_wasm_live_production_device_boot_gate is not None
        or args.virtio_wasm_live_production_device_boot_live is not None
        or args.virtio_wasm_live_manual_subsystem
    ):
        expected_ticks = (
            128
            if (
                args.virtio_wasm_live_production_boot_live_soak
                or args.virtio_wasm_live_production_subsystem_source_live_soak
                or args.virtio_wasm_live_production_real_subsystem_source_live_soak
                or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
                or args.virtio_wasm_live_production_normal_boot_live_soak
                or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
            )
            else (
            64
            if (
                args.virtio_wasm_live_production_boot_gate
                or args.virtio_wasm_live_production_boot_live
                or args.virtio_wasm_live_production_device_boot_gate is not None
                or args.virtio_wasm_live_production_device_boot_live is not None
            )
            else (
                32
                if (
                    args.virtio_wasm_live_subsystem_runtime
                    or args.virtio_wasm_live_production_subsystem_source
                    or args.virtio_wasm_live_production_subsystem_source_live
                    or args.virtio_wasm_live_production_subsystem_source_live_soak
                    or args.virtio_wasm_live_production_real_subsystem_source_live
                    or args.virtio_wasm_live_production_real_subsystem_source_live_soak
                    or args.virtio_wasm_live_production_normal_subsystem_source_live
                    or args.virtio_wasm_live_production_sandbox_source_live_soak
                    or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
                    or args.virtio_wasm_live_production_normal_boot_live
                    or args.virtio_wasm_live_production_normal_boot_live_soak
                    or args.virtio_wasm_live_production_sandbox_source_live_device_soak is not None
                    or args.virtio_wasm_live_production_real_subsystem_source_live_device is not None
                    or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
                )
                else (
                    8
                    if args.virtio_wasm_live_production_queue
                    else (
                        16
                        if (
                            args.virtio_wasm_live_os_pump_long
                            or args.virtio_wasm_live_os_subsystem_bridge
                        )
                        else 4
                    )
                )
            )
            )
        )
        device_gate = (
            args.virtio_wasm_live_production_device_boot_gate
            or args.virtio_wasm_live_production_device_boot_live
            or args.virtio_wasm_live_production_real_subsystem_source_live_device
            or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak
            or args.virtio_wasm_live_production_hardware_payload_source_live_device
            or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak
            or args.virtio_wasm_live_production_sandbox_source_live_device
            or args.virtio_wasm_live_production_sandbox_source_live_device_soak
        )
        expected_items_per_tick = (
            {"block": 2, "gpu": 1, "net": 2, "input": 1}[device_gate]
            if device_gate is not None
            else 6
        )
        expected_items = expected_ticks * expected_items_per_tick
        if args.virtio_wasm_live_production_sandbox_source_live or args.virtio_wasm_live_production_sandbox_source_live_device:
            expected_ticks = 1
            expected_items_per_tick = (
                {"block": 2, "gpu": 1, "net": 2, "input": 1}[device_gate]
                if device_gate is not None
                else 6
            )
            expected_items = expected_ticks * expected_items_per_tick
        if (
            args.virtio_wasm_live_production_hardware_payload_source_live
            or args.virtio_wasm_live_production_hardware_payload_source_live_device
            or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak
        ):
            expected_ticks = (
                32
                if args.virtio_wasm_live_production_hardware_payload_source_live_device_soak
                else 8
            )
            expected_items_per_tick = 6
            if device_gate is not None:
                expected_items_per_tick = {"block": 2, "gpu": 1, "net": 2, "input": 1}[device_gate]
            expected_items = expected_ticks * expected_items_per_tick
        if (
            args.virtio_wasm_live_production_external_normal_source_live
            or args.virtio_wasm_live_production_external_normal_device_submit_live
            or args.virtio_wasm_live_production_external_normal_device_submit_soak
        ):
            expected_ticks = (
                args.external_normal_device_submit_batches
                if args.virtio_wasm_live_production_external_normal_device_submit_soak
                else 1
            )
            expected_items_per_tick = 6
            expected_items = expected_ticks * expected_items_per_tick
        if "virtio live OS pump selftest complete" not in output:
            return validation_failed("missing live OS pump completion")
        if "virtio live OS pump selftest passed=1" not in output:
            return validation_failed("missing live OS pump pass marker")
        armed_marker = (
            "live OS runtime selftest armed=1"
            if args.virtio_wasm_live_os_runtime
            else (
                "subsystem bridge selftest armed=1"
                if args.virtio_wasm_live_os_subsystem_bridge
                else (
                        "production subsystem queue selftest armed=1"
                        if args.virtio_wasm_live_production_queue
                        else (
                            "external normal subsystem source active=1"
                            if (
                                args.virtio_wasm_live_production_external_normal_source_live
                                or args.virtio_wasm_live_production_external_normal_device_submit_live
                                or args.virtio_wasm_live_production_external_normal_device_submit_soak
                            )
                            else (
                            "hardware payload proof owner mask="
                            if (
                                args.virtio_wasm_live_production_hardware_payload_source_live
                                or args.virtio_wasm_live_production_hardware_payload_source_live_device is not None
                                or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is not None
                            )
                            else (
                            "production subsystem-source runtime armed=1"
                            if (
                                args.virtio_wasm_live_production_subsystem_source
                                or args.virtio_wasm_live_production_subsystem_source_live
                                or args.virtio_wasm_live_production_subsystem_source_live_soak
                                or args.virtio_wasm_live_production_real_subsystem_source_live
                                or args.virtio_wasm_live_production_real_subsystem_source_live_soak
                                or args.virtio_wasm_live_production_normal_subsystem_source_live
                                or args.virtio_wasm_live_production_sandbox_source_live
                                or args.virtio_wasm_live_production_sandbox_source_live_soak
                                or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
                                or args.virtio_wasm_live_production_normal_boot_live
                                or args.virtio_wasm_live_production_normal_boot_live_soak
                                or args.virtio_wasm_live_production_sandbox_source_live_device is not None
                                or args.virtio_wasm_live_production_sandbox_source_live_device_soak is not None
                                or args.virtio_wasm_live_production_real_subsystem_source_live_device is not None
                                or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
                            )
                            else (
                        "production boot gate armed=1"
                        if args.virtio_wasm_live_production_boot_gate
                        else (
                        "production boot-live armed=1"
                        if args.virtio_wasm_live_production_boot_live
                        else (
                            "production boot-live soak armed=1"
                            if args.virtio_wasm_live_production_boot_live_soak
                            else (
                                "production device boot-live armed=1"
                                if args.virtio_wasm_live_production_device_boot_live is not None
                                else (
                                    "production device boot gate armed=1"
                                    if args.virtio_wasm_live_production_device_boot_gate is not None
                                    else (
                                        "manual subsystem selftest armed=1"
                                        if args.virtio_wasm_live_manual_subsystem
                                        else (
                                            "subsystem runtime soak armed=1"
                                            if args.virtio_wasm_live_subsystem_runtime
                                            else (
                                                "OS producer API selftest armed=1"
                                                if args.virtio_wasm_live_os_request_api
                                                else "live OS pump selftest armed=1"
                        )
                        )
                        )
                        )
                        )
                    )
                )
                        )
                        )
                        )
                    )
                )
            )
        )
        if armed_marker not in output:
            return validation_failed(f"missing armed marker: {armed_marker}")
        if args.virtio_wasm_live_os_pump_long and "live OS pump profile=long target_ticks=16" not in output:
            return 1
        if args.virtio_wasm_live_os_request_api and f"OS producer API prequeued items={expected_items}" not in output:
            return 1
        if args.virtio_wasm_live_os_runtime and f"live OS runtime selftest prequeued items={expected_items}" not in output:
            return 1
        if args.virtio_wasm_live_os_subsystem_bridge and f"subsystem bridge prequeued items={expected_items}" not in output:
            return 1
        if args.virtio_wasm_live_subsystem_runtime and f"subsystem runtime soak armed=1 target_ticks={expected_ticks}" not in output:
            return 1
        if args.virtio_wasm_live_production_queue and f"production subsystem queue selftest armed=1 target_ticks={expected_ticks}" not in output:
            return 1
        if (
            args.virtio_wasm_live_production_subsystem_source
            or args.virtio_wasm_live_production_subsystem_source_live
            or args.virtio_wasm_live_production_subsystem_source_live_soak
            or args.virtio_wasm_live_production_real_subsystem_source_live
            or args.virtio_wasm_live_production_real_subsystem_source_live_soak
            or args.virtio_wasm_live_production_normal_subsystem_source_live
            or args.virtio_wasm_live_production_sandbox_source_live
            or args.virtio_wasm_live_production_sandbox_source_live_soak
            or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
            or args.virtio_wasm_live_production_normal_boot_live
            or args.virtio_wasm_live_production_normal_boot_live_soak
            or args.virtio_wasm_live_production_sandbox_source_live_device is not None
            or args.virtio_wasm_live_production_sandbox_source_live_device_soak is not None
            or args.virtio_wasm_live_production_real_subsystem_source_live_device is not None
            or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
        ) and f"production subsystem-source runtime armed=1 target_ticks={expected_ticks}" not in output:
            return 1
        if args.virtio_wasm_live_production_boot_gate and f"production boot gate armed=1 target_ticks={expected_ticks}" not in output:
            return 1
        if args.virtio_wasm_live_production_boot_live and f"production boot-live armed=1 target_ticks={expected_ticks}" not in output:
            return 1
        if args.virtio_wasm_live_production_boot_live_soak and f"production boot-live soak armed=1 target_ticks={expected_ticks}" not in output:
            return 1
        if args.virtio_wasm_live_production_device_boot_gate and f"production device boot gate armed=1 target_ticks={expected_ticks}" not in output:
            return 1
        if args.virtio_wasm_live_production_device_boot_live and f"production device boot-live armed=1 target_ticks={expected_ticks}" not in output:
            return 1
        if args.virtio_wasm_live_manual_subsystem and f"manual subsystem selftest armed=1 required_submit_all={expected_ticks}" not in output:
            return 1
        if args.virtio_wasm_live_os_request_api:
            if "command returned; ARM64 main loop must drain producer-owned OS work." not in output:
                return 1
        elif args.virtio_wasm_live_os_runtime:
            if "command returned; ARM64 main loop must drain live runtime OS work." not in output:
                return 1
            if "virtio live OS runtime selftest complete" not in output:
                return 1
            if "virtio live OS runtime selftest passed=1" not in output:
                return 1
        elif args.virtio_wasm_live_os_subsystem_bridge:
            if "command returned; ARM64 main loop must drain subsystem-backed OS work." not in output:
                return 1
            if "virtio live OS subsystem bridge selftest complete" not in output:
                return 1
            if "virtio live OS subsystem bridge selftest passed=1" not in output:
                return 1
            expected_subsystem = (
                f"subsystem producer counters block_read={expected_ticks} "
                f"block_write={expected_ticks} gpu_control={expected_ticks} "
                f"net_rx={expected_ticks} net_tx={expected_ticks} "
                f"input_events={expected_ticks} rejected=0"
            )
            if expected_subsystem not in output:
                return 1
        elif args.virtio_wasm_live_subsystem_runtime:
            if "command returned; subsystem producer will feed sandbox owners from the main loop." not in output:
                return 1
            if "virtio live subsystem runtime soak complete" not in output:
                return 1
            if "virtio live subsystem runtime soak passed=1" not in output:
                return 1
            expected_subsystem = (
                f"subsystem producer counters block_read={expected_ticks} "
                f"block_write={expected_ticks} gpu_control={expected_ticks} "
                f"net_rx={expected_ticks} net_tx={expected_ticks} "
                f"input_events={expected_ticks} rejected=0"
            )
            if expected_subsystem not in output:
                return 1
        elif args.virtio_wasm_live_production_queue:
            if "command returned; normal subsystem inbox must feed sandbox owners from the main loop." not in output:
                return 1
            if "virtio live production subsystem queue complete" not in output:
                return 1
            if "virtio live production subsystem queue passed=1" not in output:
                return 1
            expected_subsystem = (
                f"subsystem producer counters block_read={expected_ticks} "
                f"block_write={expected_ticks} gpu_control={expected_ticks} "
                f"net_rx={expected_ticks} net_tx={expected_ticks} "
                f"input_events={expected_ticks} rejected=0"
            )
            expected_inbox = (
                "production subsystem inbox block_read_pending=0 block_write_pending=0 "
                "gpu_control_pending=0 net_rx_pending=0 net_tx_pending=0 "
                f"input_event_pending=0 enqueued={expected_items} "
                f"forwarded={expected_items} rejected=0"
            )
            if expected_subsystem not in output or expected_inbox not in output:
                return 1
        elif (
            args.virtio_wasm_live_production_external_normal_source_live
            or args.virtio_wasm_live_production_external_normal_device_submit_live
            or args.virtio_wasm_live_production_external_normal_device_submit_soak
        ):
            if "command returned; external normal subsystem submit commands must feed sandbox owners." not in output:
                return validation_failed("missing external-normal command-return marker")
            expected_mask = virtio_device_mask(None)
            if f"external normal subsystem source active=1 target_ticks=1 device_mask={expected_mask}" not in output:
                return validation_failed("missing external-normal source mask")
            if "external_normal_source=1 generated_normal_source=0" not in output:
                return validation_failed("missing external-normal boundary marker")
            if "external normal source uses proof owner mask; live device promotion remains closed." not in output:
                return validation_failed("missing external-normal proof-owner marker")
            if " normal_source=1" in output:
                return validation_failed("generated normal-source path was used")
            if args.virtio_wasm_live_production_external_normal_device_submit_soak:
                marker = "external normal subsystem submit-device-round block=2 gpu=1 net=2 input=1 accepted=6"
                if output.count(marker) < args.external_normal_device_submit_batches:
                    return validation_failed("missing repeated external-normal device-round submit markers")
            elif args.virtio_wasm_live_production_external_normal_device_submit_live:
                for marker in (
                    "external normal subsystem submit-block accepted=2",
                    "external normal subsystem submit-gpu accepted=1",
                    "external normal subsystem submit-net accepted=2",
                    "external normal subsystem submit-input accepted=1",
                ):
                    if marker not in output:
                        return validation_failed(f"missing external-normal device submit marker: {marker}")
            elif "external normal subsystem submit-all accepted=6" not in output:
                return validation_failed("external normal submit-all did not enqueue every work item")
            expected_normal_api = production_expected_subsystem(
                expected_ticks, None
            ).replace(
                "subsystem producer counters", "ARM64 normal subsystem API"
            )
            expected_normal_health = (
                f"ARM64 normal producer health storage_ticks={expected_ticks} "
                f"display_ticks={expected_ticks} network_ticks={expected_ticks} "
                f"input_ticks={expected_ticks} rejected=0"
            )
            expected_normal_subsystem_queues = (
                f"ARM64 normal subsystem queues len=0 enqueued={expected_items} "
                f"dispatched={expected_items} rejected=0"
            )
            expected_normal_device_queues = (
                f"ARM64 normal device queues len=0 enqueued={expected_items} "
                f"submitted={expected_items} rejected=0"
            )
            expected_normal_ingress_queue = (
                f"ARM64 normal ingress queue len=0 enqueued={expected_items} "
                f"submitted={expected_items} rejected=0"
            )
            expected_normal_source_queue = (
                f"ARM64 normal source queue len=0 enqueued={expected_items} "
                f"forwarded={expected_items} rejected=0"
            )
            expected_normal_descriptor_queue = (
                f"ARM64 normal descriptor queue len=0 enqueued={expected_items} "
                f"dispatched={expected_items} rejected=0"
            )
            for marker in (
                expected_normal_subsystem_queues,
                expected_normal_api,
                expected_normal_device_queues,
                expected_normal_ingress_queue,
                expected_normal_source_queue,
                expected_normal_descriptor_queue,
                expected_normal_health,
                production_expected_inbox(expected_items),
            ):
                if marker not in output:
                    return validation_failed(f"missing external-normal exact counter: {marker}")
            if "virtio live production subsystem queue complete" not in output:
                return validation_failed("missing external-normal production completion")
            if "virtio live production subsystem queue passed=1" not in output:
                return validation_failed("missing external-normal production pass marker")
            if "production external-normal keepalive active=1" not in output:
                return validation_failed("missing external-normal keepalive marker")
            if "production queue runtime running=1" not in output:
                return validation_failed("missing external-normal runtime marker")
            if "source_mode=validated-keepalive boot_default_live=0" not in output:
                return validation_failed("missing external-normal keepalive/source boundary")
            if "live service running after stop=0" not in output:
                return validation_failed("missing external-normal stopped service marker")
        elif (
            args.virtio_wasm_live_production_sandbox_source_live
            or args.virtio_wasm_live_production_sandbox_source_live_soak
            or args.virtio_wasm_live_production_sandbox_source_live_device is not None
            or args.virtio_wasm_live_production_sandbox_source_live_device_soak is not None
        ):
            if "command returned; ARM64 subsystem sources must produce work through virtio_production_request_*." not in output:
                return validation_failed("missing sandbox-source command-return marker")
            sandbox_device = (
                args.virtio_wasm_live_production_sandbox_source_live_device
                or args.virtio_wasm_live_production_sandbox_source_live_device_soak
            )
            expected_mask = virtio_device_mask(sandbox_device)
            if f"ARM64 sandbox-driver source device_mask={expected_mask}" not in output:
                return validation_failed(f"missing sandbox-source device mask {expected_mask}")
            if "sandbox_source=1 generated_normal_source=0" not in output:
                return validation_failed("missing sandbox-source boundary marker")
            if "normal_source=1" in output:
                return validation_failed("generated normal-source path was used")
            if output.count("sandbox source submitted kind=") < expected_items:
                return validation_failed("sandbox drivers did not submit every production work item")
            expected_normal_api = production_expected_subsystem(
                expected_ticks, sandbox_device
            ).replace(
                "subsystem producer counters", "ARM64 normal subsystem API"
            )
            expected_normal_subsystem_queues = (
                f"ARM64 normal subsystem queues len=0 enqueued={expected_items} "
                f"dispatched={expected_items} rejected=0"
            )
            expected_normal_device_queues = (
                f"ARM64 normal device queues len=0 enqueued={expected_items} "
                f"submitted={expected_items} rejected=0"
            )
            expected_normal_ingress_queue = (
                f"ARM64 normal ingress queue len=0 enqueued={expected_items} "
                f"submitted={expected_items} rejected=0"
            )
            expected_normal_source_queue = (
                f"ARM64 normal source queue len=0 enqueued={expected_items} "
                f"forwarded={expected_items} rejected=0"
            )
            expected_normal_descriptor_queue = (
                f"ARM64 normal descriptor queue len=0 enqueued={expected_items} "
                f"dispatched={expected_items} rejected=0"
            )
            for marker in (
                expected_normal_subsystem_queues,
                expected_normal_api,
                expected_normal_device_queues,
                expected_normal_ingress_queue,
                expected_normal_source_queue,
                expected_normal_descriptor_queue,
                production_expected_inbox(expected_items),
            ):
                if marker not in output:
                    return validation_failed(f"missing sandbox-source exact counter: {marker}")
            if "virtio live production subsystem queue complete" not in output:
                return validation_failed("missing sandbox-source production completion")
            if "virtio live production subsystem queue passed=1" not in output:
                return validation_failed("missing sandbox-source production pass marker")
            if "production source-live keepalive active=1" not in output:
                return validation_failed("missing sandbox-source keepalive marker")
            if "production queue runtime running=1" not in output:
                return validation_failed("missing sandbox-source runtime marker")
            if "source_mode=validated-keepalive boot_default_live=0" not in output:
                return validation_failed("missing sandbox-source keepalive/source boundary")
            if "live service running after stop=0" not in output:
                return validation_failed("missing sandbox-source stopped service marker")
        elif (
            args.virtio_wasm_live_production_subsystem_source
            or args.virtio_wasm_live_production_subsystem_source_live
            or args.virtio_wasm_live_production_subsystem_source_live_soak
            or args.virtio_wasm_live_production_real_subsystem_source_live
            or args.virtio_wasm_live_production_real_subsystem_source_live_soak
            or args.virtio_wasm_live_production_normal_subsystem_source_live
            or args.virtio_wasm_live_production_sandbox_source_live
            or args.virtio_wasm_live_production_hardware_payload_source_live
            or args.virtio_wasm_live_production_hardware_payload_source_live_device is not None
            or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is not None
            or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
            or args.virtio_wasm_live_production_normal_boot_live
            or args.virtio_wasm_live_production_normal_boot_live_soak
            or args.virtio_wasm_live_production_real_subsystem_source_live_device is not None
            or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
        ):
            hardware_payload_source = (
                args.virtio_wasm_live_production_hardware_payload_source_live
                or args.virtio_wasm_live_production_hardware_payload_source_live_device is not None
                or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is not None
            )
            expected_command_return = (
                "command returned; ARM64 descriptor producers must feed hardware payload dispatch."
                if hardware_payload_source
                else "command returned; ARM64 subsystem sources must produce work through virtio_production_request_*."
            )
            if expected_command_return not in output:
                return validation_failed("missing source command-return marker")
            if (
                args.virtio_wasm_live_production_real_subsystem_source_live
                or args.virtio_wasm_live_production_real_subsystem_source_live_soak
                or args.virtio_wasm_live_production_normal_subsystem_source_live
                or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
                or args.virtio_wasm_live_production_normal_boot_live
                or args.virtio_wasm_live_production_normal_boot_live_soak
                or args.virtio_wasm_live_production_real_subsystem_source_live_device is not None
                or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
            ):
                expected_mask = virtio_device_mask(
                    args.virtio_wasm_live_production_real_subsystem_source_live_device
                    or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak
                )
                if f"ARM64 real subsystem source detected device_mask={expected_mask}" not in output:
                    return validation_failed(f"missing detected real-source mask {expected_mask}")
            if (
                args.virtio_wasm_live_production_normal_subsystem_source_live
                or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
                or args.virtio_wasm_live_production_normal_boot_live
                or args.virtio_wasm_live_production_normal_boot_live_soak
            ):
                expected_mask = virtio_device_mask(device_gate)
                if f"ARM64 normal subsystem producer source device_mask={expected_mask}" not in output:
                    return validation_failed(f"missing normal-source mask {expected_mask}")
                if "normal_source=1" not in output:
                    return validation_failed("missing normal-source arming marker")
                expected_normal_health = (
                    f"ARM64 normal producer health storage_ticks={expected_ticks} "
                    f"display_ticks={expected_ticks} network_ticks={expected_ticks} "
                    f"input_ticks={expected_ticks} rejected=0"
                )
                expected_normal_api = (
                    f"ARM64 normal subsystem API block_read={expected_ticks} "
                    f"block_write={expected_ticks} gpu_control={expected_ticks} "
                    f"net_rx={expected_ticks} net_tx={expected_ticks} "
                    f"input_events={expected_ticks} rejected=0"
                )
                expected_normal_subsystem_queues = (
                    f"ARM64 normal subsystem queues len=0 enqueued={expected_items} "
                    f"dispatched={expected_items} rejected=0"
                )
                expected_normal_device_queues = (
                    f"ARM64 normal device queues len=0 enqueued={expected_items} "
                    f"submitted={expected_items} rejected=0"
                )
                expected_normal_ingress_queue = (
                    f"ARM64 normal ingress queue len=0 enqueued={expected_items} "
                    f"submitted={expected_items} rejected=0"
                )
                expected_normal_source_queue = (
                    f"ARM64 normal source queue len=0 enqueued={expected_items} "
                    f"forwarded={expected_items} rejected=0"
                )
                expected_normal_descriptor_queue = (
                    f"ARM64 normal descriptor queue len=0 enqueued={expected_items} "
                    f"dispatched={expected_items} rejected=0"
                )
                if expected_normal_subsystem_queues not in output:
                    return validation_failed("missing normal subsystem queues exact counters")
                if expected_normal_api not in output:
                    return validation_failed("missing normal subsystem API exact counters")
                if expected_normal_device_queues not in output:
                    return validation_failed("missing normal device queues exact counters")
                if expected_normal_ingress_queue not in output:
                    return validation_failed("missing normal ingress queue exact counters")
                if expected_normal_source_queue not in output:
                    return validation_failed("missing normal source queue exact counters")
                if expected_normal_descriptor_queue not in output:
                    return validation_failed("missing normal descriptor queue exact counters")
                if expected_normal_health not in output:
                    return validation_failed("missing normal producer source health counters")
            if "virtio live production subsystem queue complete" not in output:
                return validation_failed("missing production subsystem queue completion")
            if "virtio live production subsystem queue passed=1" not in output:
                return validation_failed("missing production subsystem queue pass marker")
            expected_source = production_expected_subsystem(expected_ticks, device_gate).replace(
                "subsystem producer", "production subsystem source"
            )
            expected_subsystem = production_expected_subsystem(expected_ticks, device_gate)
            expected_inbox = production_expected_inbox(expected_items)
            expected_source_queue = (
                "ARM64 subsystem queue pending block_read=0 block_write=0 "
                "gpu_control=0 net_rx=0 net_tx=0 "
                f"input_events=0 enqueued={expected_items} "
                f"dispatched={expected_items} rejected=0"
            )
            if (
                expected_source not in output
                or expected_subsystem not in output
                or expected_inbox not in output
                or expected_source_queue not in output
            ):
                return validation_failed("missing source/subsystem/inbox exact counters")
            if (
                args.virtio_wasm_live_production_hardware_payload_source_live
                or args.virtio_wasm_live_production_hardware_payload_source_live_device
                or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak
            ):
                expected_payload = production_expected_hardware_payload(expected_ticks, device_gate)
                if expected_payload not in output:
                    return validation_failed("missing hardware payload dispatch exact counters")
                activation_marker = (
                    "hardware payload dispatch source active=1 boot_default_live=0"
                    if args.virtio_wasm_live_production_hardware_payload_source_live
                    else "hardware payload dispatch device source active=1"
                )
                if activation_marker not in output:
                    return validation_failed("missing hardware payload dispatch activation marker")
                if args.virtio_wasm_live_production_hardware_payload_promote_rollback_device:
                    promoted = args.virtio_wasm_live_production_hardware_payload_promote_rollback_device
                    for marker in (
                        "hardware payload evidence accepted mask=",
                        f"virtio {promoted} hardware-payload evidence promotion begins.",
                        f"virtio {promoted} hardware-payload sandbox-active owner recorded.",
                        f"{promoted} live dispatch rounds=4 passed=1",
                        f"{promoted} rollback proof=1",
                        "owner_mask=0x00000000",
                    ):
                        if marker not in output:
                            return validation_failed(
                                f"missing hardware payload promote/rollback marker: {marker}"
                            )
                if args.virtio_wasm_live_production_hardware_payload_policy_candidate_device:
                    policy_device = (
                        args.virtio_wasm_live_production_hardware_payload_policy_candidate_device
                    )
                    expected_policy_mask = virtio_device_mask(policy_device)
                    for marker in (
                        "hardware payload evidence accepted mask=",
                        f"virtio {policy_device} hardware-payload boot-policy candidate request begins.",
                        f"virtio {policy_device} hardware-payload boot-policy candidate recorded.",
                        f"hardware-payload boot-policy candidate mask={expected_policy_mask} boot_default_live=0 persisted=0",
                        "hardware-payload boot-policy candidates cleared; boot_default_live=0.",
                        "hardware payload boot-policy candidate mask=0x00000000 boot_default_live=0 persisted=0",
                    ):
                        if marker not in output:
                            return validation_failed(
                                f"missing hardware payload policy marker: {marker}"
                            )
                if args.virtio_wasm_live_production_hardware_payload_policy_capsule_device:
                    capsule_device = (
                        args.virtio_wasm_live_production_hardware_payload_policy_capsule_device
                    )
                    expected_capsule_mask = virtio_device_mask(capsule_device)
                    for marker in (
                        "hardware payload evidence accepted mask=",
                        f"virtio {capsule_device} hardware-payload boot-policy candidate request begins.",
                        f"virtio {capsule_device} hardware-payload boot-policy candidate recorded.",
                        f"virtio {capsule_device} hardware-payload boot-policy capsule seal begins.",
                        f"hardware-payload boot-policy capsule sealed device={capsule_device} mask={expected_capsule_mask}",
                        "hardware-payload boot-policy capsule verify active=1 ok=1",
                        "hardware-payload boot-policy capsule cleared; boot_default_live=0 persisted=0.",
                        "hardware payload boot-policy capsule active=0 mask=0x00000000",
                    ):
                        if marker not in output:
                            return validation_failed(
                                f"missing hardware payload capsule marker: {marker}"
                            )
                if args.virtio_wasm_live_production_hardware_payload_policy_storage_device:
                    storage_device = (
                        args.virtio_wasm_live_production_hardware_payload_policy_storage_device
                    )
                    expected_storage_mask = virtio_device_mask(storage_device)
                    for marker in (
                        "hardware payload evidence accepted mask=",
                        f"virtio {storage_device} hardware-payload boot-policy candidate recorded.",
                        f"hardware-payload boot-policy capsule sealed device={storage_device} mask={expected_storage_mask}",
                        "hardware-payload boot-policy capsule verify active=1 ok=1",
                        f"hardware-payload policy storage write-intent staged mask={expected_storage_mask}",
                        f"policy storage commit denied; storage capability missing mask={expected_storage_mask}",
                        "hardware-payload policy storage write-intent rolled back; persisted=0.",
                        "hardware payload policy storage intent active=0 mask=0x00000000",
                    ):
                        if marker not in output:
                            return validation_failed(
                                f"missing hardware payload storage marker: {marker}"
                            )
                if args.virtio_wasm_live_production_hardware_payload_policy_storage_proof_device:
                    storage_device = (
                        args.virtio_wasm_live_production_hardware_payload_policy_storage_proof_device
                    )
                    expected_storage_mask = virtio_device_mask(storage_device)
                    for marker in (
                        "hardware payload evidence accepted mask=",
                        f"virtio {storage_device} hardware-payload boot-policy candidate recorded.",
                        f"hardware-payload boot-policy capsule sealed device={storage_device} mask={expected_storage_mask}",
                        "hardware-payload boot-policy capsule verify active=1 ok=1",
                        f"hardware-payload policy storage write-intent staged mask={expected_storage_mask}",
                        "sandbox policy storage proof capability attached lease=",
                        f"policy storage commit accepted into volatile proof slot mask={expected_storage_mask}",
                        "policy storage volatile proof verify active=1 ok=1",
                        "policy storage volatile proof slot cleared; persisted=0 boot_default_live=0.",
                        "storage proof capability active=0",
                        "volatile_committed=0",
                    ):
                        if marker not in output:
                            return validation_failed(
                                f"missing hardware payload storage proof marker: {marker}"
                            )
                if args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_proof_device:
                    storage_device = (
                        args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_proof_device
                    )
                    expected_storage_mask = virtio_device_mask(storage_device)
                    for marker in (
                        "hardware payload evidence accepted mask=",
                        f"virtio {storage_device} hardware-payload boot-policy candidate recorded.",
                        f"hardware-payload boot-policy capsule sealed device={storage_device} mask={expected_storage_mask}",
                        "hardware-payload boot-policy capsule verify active=1 ok=1",
                        f"hardware-payload policy storage write-intent staged mask={expected_storage_mask}",
                        "sandbox policy storage provider proof attached lease=",
                        f"policy storage provider seal accepted mask={expected_storage_mask}",
                        "policy storage provider readback active=1 ok=1",
                        "policy storage provider proof cleared; provider_abi=proof persisted=0 boot_default_live=0.",
                        "storage provider active=0",
                        "slot=0",
                    ):
                        if marker not in output:
                            return validation_failed(
                                f"missing hardware payload storage provider proof marker: {marker}"
                            )
                if args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_restore_proof_device:
                    storage_device = (
                        args.virtio_wasm_live_production_hardware_payload_policy_storage_provider_restore_proof_device
                    )
                    expected_storage_mask = virtio_device_mask(storage_device)
                    for marker in (
                        "hardware payload evidence accepted mask=",
                        f"virtio {storage_device} hardware-payload boot-policy candidate recorded.",
                        f"hardware-payload boot-policy capsule sealed device={storage_device} mask={expected_storage_mask}",
                        "hardware-payload boot-policy capsule verify active=1 ok=1",
                        f"hardware-payload policy storage write-intent staged mask={expected_storage_mask}",
                        "sandbox policy storage provider proof attached lease=",
                        f"policy storage provider seal accepted mask={expected_storage_mask}",
                        "hardware-payload boot-policy capsule cleared; boot_default_live=0 persisted=0.",
                        f"policy storage provider restore active=1 ok=1 restored=1 mask={expected_storage_mask}",
                        "policy storage provider readback active=1 ok=1",
                        "policy storage provider proof cleared; provider_abi=proof persisted=0 boot_default_live=0.",
                        "storage provider active=0",
                        "slot=0",
                    ):
                        if marker not in output:
                            return validation_failed(
                                f"missing hardware payload storage provider restore proof marker: {marker}"
                            )
            if (
                args.virtio_wasm_live_production_subsystem_source_live
                or args.virtio_wasm_live_production_subsystem_source_live_soak
                or args.virtio_wasm_live_production_real_subsystem_source_live
                or args.virtio_wasm_live_production_real_subsystem_source_live_soak
                or args.virtio_wasm_live_production_normal_subsystem_source_live
                or args.virtio_wasm_live_production_normal_subsystem_source_live_soak
                or args.virtio_wasm_live_production_hardware_payload_source_live
                or args.virtio_wasm_live_production_hardware_payload_source_live_device is not None
                or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is not None
                or args.virtio_wasm_live_production_normal_boot_live
                or args.virtio_wasm_live_production_normal_boot_live_soak
                or args.virtio_wasm_live_production_real_subsystem_source_live_device is not None
                or args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is not None
            ):
                if "production source-live keepalive active=1" not in output:
                    return validation_failed("missing source-live keepalive marker")
                if "production queue runtime running=1" not in output:
                    return validation_failed("missing production runtime marker")
                if (
                    args.virtio_wasm_live_production_hardware_payload_source_live
                    or args.virtio_wasm_live_production_hardware_payload_source_live_device
                    or args.virtio_wasm_live_production_hardware_payload_source_live_device_soak
                ):
                    if "source_mode=detected-normal-hardware-payload boot_default_live=0" not in output:
                        return validation_failed("missing hardware payload source boundary marker")
                elif "source_mode=validated-keepalive boot_default_live=0" not in output:
                    return validation_failed("missing explicit validated keepalive/source boundary marker")
                expected_owner_mask = virtio_device_mask(device_gate)
                if (
                    f"live owner mask={expected_owner_mask}" not in output
                    and f"owner_mask={expected_owner_mask}" not in output
                ):
                    return validation_failed(f"missing live owner mask {expected_owner_mask}")
                if (
                    args.virtio_wasm_live_production_normal_boot_live
                    or args.virtio_wasm_live_production_normal_boot_live_soak
                ):
                    if "virtio live production boot gate passed=1" not in output:
                        return validation_failed("missing normal boot-live pass marker")
                    if "production boot-live keepalive active=1" not in output:
                        return validation_failed("missing normal boot-live keepalive marker")
                    if "live service running after stop=0" in output:
                        return validation_failed("normal boot-live unexpectedly stopped service")
                else:
                    if "virtio live service stop complete" not in output:
                        return validation_failed("missing service stop completion")
                    if "live service running after stop=0" not in output:
                        return validation_failed("missing stopped service marker")
            else:
                if "live service running after stop=0" not in output:
                    return validation_failed("missing non-live stopped service marker")
        elif (
            args.virtio_wasm_live_production_boot_gate
            or args.virtio_wasm_live_production_boot_live
            or args.virtio_wasm_live_production_boot_live_soak
            or args.virtio_wasm_live_production_device_boot_gate
            or args.virtio_wasm_live_production_device_boot_live
        ):
            if "virtio live production boot gate complete" not in output:
                return 1
            if "virtio live production boot gate passed=1" not in output:
                return 1
            if (
                args.virtio_wasm_live_production_boot_live
                or args.virtio_wasm_live_production_boot_live_soak
                or args.virtio_wasm_live_production_device_boot_live
            ):
                if "production boot-live keepalive active=1" not in output:
                    return 1
                if "live service running after stop=0" in output:
                    return 1
            expected_subsystem = production_expected_subsystem(
                expected_ticks,
                args.virtio_wasm_live_production_device_boot_gate
                or args.virtio_wasm_live_production_device_boot_live,
            )
            expected_inbox = production_expected_inbox(expected_items)
            if expected_subsystem not in output or expected_inbox not in output:
                return 1
        elif args.virtio_wasm_live_manual_subsystem:
            if "command returned; external subsystem submit commands must feed sandbox owners." not in output:
                return 1
            if output.count("subsystem manual submit-all queued=6") < expected_ticks:
                return 1
            if "virtio live manual subsystem selftest complete" not in output:
                return 1
            if "virtio live manual subsystem selftest passed=1" not in output:
                return 1
            expected_subsystem = (
                f"subsystem producer counters block_read={expected_ticks} "
                f"block_write={expected_ticks} gpu_control={expected_ticks} "
                f"net_rx={expected_ticks} net_tx={expected_ticks} "
                f"input_events={expected_ticks} rejected=0"
            )
            if expected_subsystem not in output:
                return 1
        elif "command returned; ARM64 main loop must pump semantic OS request/event work." not in output:
            return 1
        if f"live OS pump target ticks={expected_ticks} queue_ok=1 counters_ok=1" not in output:
            return validation_failed("missing live OS pump target success")
        if (
            not args.virtio_wasm_live_production_boot_live
            and not args.virtio_wasm_live_production_boot_live_soak
            and not args.virtio_wasm_live_production_normal_boot_live
            and not args.virtio_wasm_live_production_normal_boot_live_soak
            and not args.virtio_wasm_live_production_device_boot_live
            and "live service running after stop=0" not in output
        ):
            return validation_failed("missing final stopped service marker")
        if f"live OS pump queue block_read_pending=0 block_write_pending=0 gpu_control_pending=0 net_rx_pending=0 net_tx_pending=0 input_event_pending=0 enqueued={expected_items} drained={expected_items}" not in output:
            return validation_failed("missing final live OS pump queue counters")
        if live_pump_expected_counters(expected_ticks, device_gate) not in output:
            return validation_failed("missing final live OS pump counters")
        if args.virtio_wasm_live_production_boot_live_soak:
            return 0
        if args.virtio_wasm_live_production_normal_boot_live_soak:
            return 0
    if args.virtio_wasm_rust_probe and "rust payload probe complete" not in output:
        return 1
    if args.virtio_wasm_rust_probe and "run_skill returned 1" not in output:
        return 1
    if (
        args.inject_input
        and not args.virtio_wasm_promotion_gate
        and args.virtio_wasm_device_enable_gate is None
        and args.virtio_wasm_device_live_cutover is None
        and not args.virtio_wasm_live_routing
        and not args.virtio_wasm_live_route_all
        and not args.virtio_wasm_live_boot_service
        and not args.virtio_wasm_live_service_loop
        and not args.virtio_wasm_live_os_pump
        and not args.virtio_wasm_live_os_pump_long
        and not args.virtio_wasm_live_os_request_api
        and not args.virtio_wasm_live_os_runtime
        and not args.virtio_wasm_live_os_subsystem_bridge
        and not args.virtio_wasm_live_subsystem_runtime
        and not args.virtio_wasm_live_production_queue
        and not args.virtio_wasm_live_production_runtime
        and not args.virtio_wasm_live_production_subsystem_source
        and not args.virtio_wasm_live_production_subsystem_source_live
        and not args.virtio_wasm_live_production_subsystem_source_live_soak
        and not args.virtio_wasm_live_production_real_subsystem_source_live
        and not args.virtio_wasm_live_production_real_subsystem_source_live_soak
        and not args.virtio_wasm_live_production_normal_subsystem_source_live
        and not args.virtio_wasm_live_production_sandbox_source_live
        and not args.virtio_wasm_live_production_sandbox_source_live_soak
        and not args.virtio_wasm_live_production_external_normal_source_live
        and not args.virtio_wasm_live_production_hardware_payload_source_live
        and args.virtio_wasm_live_production_hardware_payload_source_live_device is None
        and args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is None
        and not args.virtio_wasm_live_production_external_normal_device_submit_live
        and not args.virtio_wasm_live_production_external_normal_device_submit_soak
        and not args.virtio_wasm_live_production_normal_subsystem_source_live_soak
        and not args.virtio_wasm_live_production_normal_boot_live
        and not args.virtio_wasm_live_production_normal_boot_live_soak
        and args.virtio_wasm_live_production_sandbox_source_live_device is None
        and args.virtio_wasm_live_production_sandbox_source_live_device_soak is None
        and args.virtio_wasm_live_production_real_subsystem_source_live_device is None
        and args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is None
        and not args.virtio_wasm_live_production_boot_gate
        and not args.virtio_wasm_live_production_boot_live
        and not args.virtio_wasm_live_production_boot_live_soak
        and args.virtio_wasm_live_production_device_boot_gate is None
        and args.virtio_wasm_live_production_device_boot_live is None
        and not args.virtio_wasm_live_manual_subsystem
        and "virtio-input event_read=1" not in output
    ):
        return 1
    if (
        args.inject_net_rx
        and not args.virtio_wasm_promotion_gate
        and args.virtio_wasm_device_enable_gate is None
        and args.virtio_wasm_device_live_cutover is None
        and not args.virtio_wasm_live_routing
        and not args.virtio_wasm_live_route_all
        and not args.virtio_wasm_live_boot_service
        and not args.virtio_wasm_live_service_loop
        and not args.virtio_wasm_live_os_pump
        and not args.virtio_wasm_live_os_pump_long
        and not args.virtio_wasm_live_os_request_api
        and not args.virtio_wasm_live_os_runtime
        and not args.virtio_wasm_live_os_subsystem_bridge
        and not args.virtio_wasm_live_subsystem_runtime
        and not args.virtio_wasm_live_production_queue
        and not args.virtio_wasm_live_production_runtime
        and not args.virtio_wasm_live_production_subsystem_source
        and not args.virtio_wasm_live_production_subsystem_source_live
        and not args.virtio_wasm_live_production_subsystem_source_live_soak
        and not args.virtio_wasm_live_production_real_subsystem_source_live
        and not args.virtio_wasm_live_production_real_subsystem_source_live_soak
        and not args.virtio_wasm_live_production_normal_subsystem_source_live
        and not args.virtio_wasm_live_production_sandbox_source_live
        and not args.virtio_wasm_live_production_sandbox_source_live_soak
        and not args.virtio_wasm_live_production_external_normal_source_live
        and not args.virtio_wasm_live_production_hardware_payload_source_live
        and args.virtio_wasm_live_production_hardware_payload_source_live_device is None
        and args.virtio_wasm_live_production_hardware_payload_source_live_device_soak is None
        and not args.virtio_wasm_live_production_external_normal_device_submit_live
        and not args.virtio_wasm_live_production_external_normal_device_submit_soak
        and not args.virtio_wasm_live_production_normal_subsystem_source_live_soak
        and not args.virtio_wasm_live_production_normal_boot_live
        and not args.virtio_wasm_live_production_normal_boot_live_soak
        and args.virtio_wasm_live_production_sandbox_source_live_device is None
        and args.virtio_wasm_live_production_sandbox_source_live_device_soak is None
        and args.virtio_wasm_live_production_real_subsystem_source_live_device is None
        and args.virtio_wasm_live_production_real_subsystem_source_live_device_soak is None
        and not args.virtio_wasm_live_production_boot_gate
        and not args.virtio_wasm_live_production_boot_live
        and not args.virtio_wasm_live_production_boot_live_soak
        and args.virtio_wasm_live_production_device_boot_gate is None
        and args.virtio_wasm_live_production_device_boot_live is None
        and not args.virtio_wasm_live_manual_subsystem
        and "virtio-net rx_event_read=1" not in output
    ):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
