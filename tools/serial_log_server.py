from __future__ import annotations

import queue
import socket
import threading
from pathlib import Path
import tkinter as tk


HOST = "127.0.0.1"
PORT = 4444
WINDOW_TITLE = "OpenRhiza Serial Log"
FONT_FAMILY = "Consolas"
FONT_SIZE = 14

REPO_ROOT = Path(__file__).resolve().parents[1]
LOG_DIR = REPO_ROOT / "logs"
LOG_FILE = LOG_DIR / "serial.log"


class SerialLogViewer:
    def __init__(self) -> None:
        self.root = tk.Tk()
        self.root.title(WINDOW_TITLE)
        self.root.configure(bg="black")
        self.root.geometry("1200x720")
        self.root.minsize(900, 420)
        self.root.tk.call("tk", "scaling", 0.8)

        frame = tk.Frame(self.root, bg="black")
        frame.pack(fill="both", expand=True)

        self.text = tk.Text(
            frame,
            bg="black",
            fg="#ffd400",
            insertbackground="#ffd400",
            wrap="none",
            font=(FONT_FAMILY, FONT_SIZE),
            borderwidth=0,
            highlightthickness=0,
            padx=8,
            pady=8,
        )
        self.text.pack(side="left", fill="both", expand=True)

        scrollbar = tk.Scrollbar(frame, command=self.text.yview)
        scrollbar.pack(side="right", fill="y")
        self.text.configure(yscrollcommand=scrollbar.set)

        self.text.tag_configure("meta_info", foreground="#69d2ff")
        self.text.tag_configure("meta_wait", foreground="#6a6a6a")
        self.text.tag_configure("meta_connected", foreground="#79ff79")
        self.text.tag_configure("meta_disconnected", foreground="#ffd400")
        self.text.tag_configure("debug", foreground="#6a6a6a")
        self.text.tag_configure("payload", foreground="#ffd400")

        self.queue: queue.Queue[str] = queue.Queue()
        self.stop_event = threading.Event()
        self.listener_thread = threading.Thread(target=self._listen_loop, daemon=True)

        LOG_DIR.mkdir(exist_ok=True)
        LOG_FILE.write_text("", encoding="utf-8")

        self.root.protocol("WM_DELETE_WINDOW", self._handle_close)

    def start(self) -> None:
        self.listener_thread.start()
        self.root.after(50, self._drain_queue)
        self.root.mainloop()

    def _handle_close(self) -> None:
        self.stop_event.set()
        self.root.destroy()

    def _append_line(self, line: str) -> None:
        tag = self._tag_for_line(line)
        self.text.insert("end", f"{line}\n", tag)
        self.text.see("end")
        with LOG_FILE.open("a", encoding="utf-8") as handle:
            handle.write(f"{line}\n")

    def _tag_for_line(self, line: str) -> str:
        if line.startswith("[OpenRhiza Serial]"):
            if "Connected" in line:
                return "meta_connected"
            if "Disconnected" in line:
                return "meta_disconnected"
            if "Waiting" in line:
                return "meta_wait"
            return "meta_info"

        if (
            line.startswith("QEMU_LOG:")
            or line.startswith("[TLS]")
            or line.startswith("[HTTPS")
            or line.startswith("[HTTP]")
        ):
            return "debug"

        return "payload"

    def _drain_queue(self) -> None:
        while True:
            try:
                line = self.queue.get_nowait()
            except queue.Empty:
                break
            self._append_line(line)

        if not self.stop_event.is_set():
            self.root.after(50, self._drain_queue)

    def _emit(self, line: str) -> None:
        self.queue.put(line)

    def _listen_loop(self) -> None:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
            server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            server.bind((HOST, PORT))
            server.listen(1)
            server.settimeout(0.5)
            self._emit(f"[OpenRhiza Serial] Listening on {HOST}:{PORT}")

            while not self.stop_event.is_set():
                self._emit("[OpenRhiza Serial] Waiting for QEMU...")

                try:
                    client, _ = server.accept()
                except socket.timeout:
                    continue
                except OSError:
                    break

                self._emit("[OpenRhiza Serial] Connected.")
                try:
                    client.settimeout(0.5)
                    pending = ""
                    while not self.stop_event.is_set():
                        try:
                            data = client.recv(4096)
                        except socket.timeout:
                            continue
                        except OSError:
                            break

                        if not data:
                            break

                        pending += data.decode("ascii", errors="replace").replace("\r", "")
                        while "\n" in pending:
                            line, pending = pending.split("\n", 1)
                            self._emit(line)

                    if pending:
                        self._emit(pending)
                finally:
                    try:
                        client.close()
                    except OSError:
                        pass
                    self._emit("")
                    self._emit("[OpenRhiza Serial] Disconnected.")


if __name__ == "__main__":
    SerialLogViewer().start()
