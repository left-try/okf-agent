from __future__ import annotations

import threading
from pathlib import Path

from .discovery import build_index, files


def start(root: Path, interval: float = 2.0) -> threading.Event:
    """Poll source mtimes and rebuild the disposable index after changes."""
    stop = threading.Event()

    def loop() -> None:
        snapshot: dict[Path, int] = {}
        while not stop.is_set():
            current = {path: path.stat().st_mtime_ns for path in files(root) if path.exists()}
            if snapshot and current != snapshot:
                try:
                    build_index(root)
                except OSError:
                    pass
            snapshot = current
            stop.wait(interval)

    threading.Thread(target=loop, name="okf-index-watcher", daemon=True).start()
    return stop
