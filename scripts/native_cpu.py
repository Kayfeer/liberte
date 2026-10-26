"""Read-only Linux CPU observation for a native client and its child processes."""
from __future__ import annotations

import os
from pathlib import Path
import time


def process_snapshot(root_pid: int) -> dict[tuple[int, int], int]:
    records = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            # comm may contain spaces or parentheses; fields begin after its final ') '.
            fields = (entry / "stat").read_text().rsplit(") ", 1)[1].split()
            records[int(entry.name)] = (int(fields[1]), int(fields[19]), int(fields[11]) + int(fields[12]))
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue  # A child can exit while /proc is being observed.
    if root_pid not in records:
        raise RuntimeError("Native application exited during CPU observation")
    included = {root_pid}
    while True:
        descendants = {pid for pid, (parent, _, _) in records.items() if parent in included}
        if descendants.issubset(included):
            break
        included.update(descendants)
    return {(pid, records[pid][1]): records[pid][2] for pid in included}


def observe_cpu_tree(root_pid: int, duration: float = 10.0) -> dict:
    if duration < 1:
        raise ValueError("CPU observation needs at least one second")
    ticks = os.sysconf("SC_CLK_TCK")
    before = process_snapshot(root_pid)
    started = time.monotonic()
    time.sleep(duration)
    after = process_snapshot(root_pid)
    elapsed = time.monotonic() - started
    # PID + kernel start tick prevents PID reuse from corrupting the delta.
    consumed = sum(max(0, total - before.get(key, 0)) for key, total in after.items()) / ticks
    return {"elapsed_seconds": round(elapsed, 3), "cpu_seconds": round(consumed, 3),
            "core_usage": round(consumed / elapsed, 4), "process_count": len(after),
            "scope": "native application and surviving child processes; one core equals 1.0"}


def assert_idle_cpu(report: dict) -> None:
    # Upstream issue16189 reports a permanently busy core at idle. This threshold
    # detects that regression while allowing startup work on a shared CI runner.
    if report["core_usage"] >= 0.8:
        raise AssertionError(f"Native client consumes a CPU core while idle: {report}")
