from __future__ import annotations

import argparse
import base64
import json
import os
import socket
import sys
import time
import urllib.request
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
VOICE_INPUT_FILE = REPO_ROOT / "rhiza_drivers" / "VOICEIN.TXT"
QEMU_MONITOR_HOST = "127.0.0.1"
QEMU_MONITOR_PORT = 55555
VOICE_INPUT_CAPACITY = 8192
MAX_AUDIO_BYTES = 1_048_576
VOICE_ROUTES = ("text-first", "direct-audio", "hybrid")


KEY_MAP = {
    "/": "slash",
    "-": "minus",
    " ": "spc",
}


def hmp_send(command: str) -> None:
    hmp_send_many([command])


def hmp_send_many(commands: list[str], pause: float = 0.05) -> None:
    with socket.create_connection((QEMU_MONITOR_HOST, QEMU_MONITOR_PORT), timeout=3.0) as client:
        client.settimeout(0.5)
        try:
            client.recv(4096)
        except OSError:
            pass
        for command in commands:
            client.sendall((command + "\r\n").encode("ascii"))
            time.sleep(pause)


def send_ascii_keys(text: str) -> None:
    commands: list[str] = []
    for ch in text:
        if "a" <= ch <= "z" or "0" <= ch <= "9":
            key = ch
        elif "A" <= ch <= "Z":
            key = "shift-" + ch.lower()
        elif ch in KEY_MAP:
            key = KEY_MAP[ch]
        else:
            raise ValueError(f"Cannot inject character through QEMU HMP: {ch!r}")
        commands.append("sendkey " + key)
    hmp_send_many(commands, pause=0.12)


def inject_guest_command(command: str, *, clear_first: bool = True) -> None:
    if clear_first:
        clear_guest_input()
    send_ascii_keys(command)
    hmp_send("sendkey ret")


def clear_guest_input(max_chars: int = 160) -> None:
    hmp_send_many(["sendkey backspace"] * max_chars, pause=0.01)


def write_voice_input(text: str) -> None:
    VOICE_INPUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    data = text.encode("utf-8")
    if len(data) > VOICE_INPUT_CAPACITY:
        raise ValueError(f"Transcript is too large for VOICEIN.TXT: {len(data)} bytes")
    VOICE_INPUT_FILE.write_bytes(data + b"\0" * (VOICE_INPUT_CAPACITY - len(data)))


def audio_prompt_for_route(route: str) -> str:
    if route == "direct-audio":
        return (
            "Analyze this bounded voice clip directly. Return concise plain text only. "
            "Include what was said if speech is present. Mention audio cues only when they change intent. "
            "Do not execute commands and do not wrap the result in markdown."
        )
    if route == "hybrid":
        return (
            "Transcribe this bounded voice clip first. If confidence is low or tone/noise materially changes "
            "the user's intent, include one short audio-context note. Return concise plain text only. "
            "Do not execute commands and do not wrap the result in markdown."
        )
    return (
        "Transcribe this audio as plain text only. "
        "Do not execute commands. Do not wrap the result in markdown."
    )


def process_audio_with_gemini(
    audio_path: Path,
    model: str,
    api_key: str,
    route: str,
    mime_type: str,
    max_audio_bytes: int,
) -> str:
    audio = audio_path.read_bytes()
    if len(audio) > max_audio_bytes:
        raise ValueError(
            f"Audio file is too large for this bounded voice bridge: {len(audio)} bytes > {max_audio_bytes}"
        )
    payload = {
        "contents": [
            {
                "role": "user",
                "parts": [
                    {
                        "text": audio_prompt_for_route(route)
                    },
                    {
                        "inline_data": {
                            "mime_type": mime_type,
                            "data": base64.b64encode(audio).decode("ascii"),
                        }
                    },
                ],
            }
        ],
        "generationConfig": {"temperature": 0},
    }
    url = f"https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}"
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        result = json.loads(response.read().decode("utf-8"))

    parts = (
        result.get("candidates", [{}])[0]
        .get("content", {})
        .get("parts", [])
    )
    text = "".join(part.get("text", "") for part in parts).strip()
    if not text:
        raise RuntimeError("Gemini returned an empty voice result")
    return text


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Write a voice transcript to OpenRhiza and optionally import it through QEMU."
    )
    parser.add_argument("--text", help="Transcript text to import.")
    parser.add_argument("--audio", type=Path, help="WAV file to transcribe with Gemini.")
    parser.add_argument("--model", default="gemini-3.1-pro-preview")
    parser.add_argument(
        "--route",
        choices=VOICE_ROUTES,
        default="text-first",
        help="Voice routing policy to mirror inside OpenRhiza before import.",
    )
    parser.add_argument("--mime-type", default="audio/wav", help="MIME type for --audio inline upload.")
    parser.add_argument(
        "--max-audio-bytes",
        type=int,
        default=MAX_AUDIO_BYTES,
        help="Safety cap for direct audio uploads.",
    )
    parser.add_argument(
        "--api-key",
        default=os.environ.get("GEMINI_API_KEY") or os.environ.get("OPENRHIZA_GEMINI_API_KEY"),
    )
    parser.add_argument("--inject", action="store_true", help="Send /voice-import to the running QEMU monitor.")
    parser.add_argument(
        "--preserve-input",
        action="store_true",
        help="Do not clear the current guest composer before injecting /voice-import.",
    )
    parser.add_argument("--submit", action="store_true", help="After importing, press Enter to submit the composer.")
    args = parser.parse_args()

    if args.text:
        transcript = args.text
    elif args.audio:
        if not args.api_key:
            raise SystemExit("GEMINI_API_KEY or OPENRHIZA_GEMINI_API_KEY is required for --audio")
        transcript = process_audio_with_gemini(
            args.audio,
            args.model,
            args.api_key,
            args.route,
            args.mime_type,
            args.max_audio_bytes,
        )
    else:
        transcript = sys.stdin.read().strip()

    if not transcript:
        raise SystemExit("No transcript text was provided")

    write_voice_input(transcript)
    print(f"Wrote transcript to {VOICE_INPUT_FILE} ({len(transcript.encode('utf-8'))} bytes)")

    if args.inject:
        if not args.preserve_input:
            inject_guest_command(f"/voice-route {args.route}", clear_first=True)
        inject_guest_command("/voice-import", clear_first=not args.preserve_input)
        print(f"Injected /voice-import into QEMU with route={args.route}.")
        if args.submit:
            hmp_send("sendkey ret")
            print("Submitted imported transcript.")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
