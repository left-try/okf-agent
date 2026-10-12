from __future__ import annotations

import logging
import threading
from pathlib import Path

from .discovery import build_index, files


logger = logging.getLogger(__name__)


def _snapshot(root: Path) -> dict[Path, int]:
    snapshot: dict[Path, int] = {}
    for path in files(root):
        try:
            snapshot[path] = path.stat().st_mtime_ns
        except OSError:
            # Files may be deleted or replaced between discovery and stat.
            continue
    return snapshot


class Watcher:
    def __init__(self, stop_event: threading.Event, thread: threading.Thread):
        self._stop_event = stop_event
        self._thread = thread

    @property
    def is_alive(self) -> bool:
        return self._thread.is_alive()

    def stop(self, timeout: float = 2.0) -> None:
        self._stop_event.set()
        self._thread.join(timeout)


def start(root: Path, interval: float = 2.0) -> Watcher:
    """Poll source mtimes and rebuild the disposable index after changes."""
    stop_event = threading.Event()

    def loop() -> None:
        snapshot: dict[Path, int] | None = None
        while not stop_event.is_set():
            current = _snapshot(root)
            if snapshot is not None and current != snapshot:
                try:
                    build_index(root)
                except Exception:
                    logger.exception("Could not rebuild the OKF index for %s", root)
            snapshot = current
            stop_event.wait(interval)

    thread = threading.Thread(target=loop, name="okf-index-watcher", daemon=True)
    thread.start()
    return Watcher(stop_event, thread)
