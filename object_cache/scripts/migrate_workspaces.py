#!/usr/bin/env python3
"""Move a stopped object cache's legacy app/session layout into default workspace.

Take a volume snapshot before --apply. If migration is interrupted, restore that
snapshot before retrying; this tool intentionally refuses a partial staging dir.
"""

import argparse
import json
import os
from pathlib import Path
import re


MARKER = ".workspace-layout-v1"
STAGE = ".workspace-migration-stage"
SAFE_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*\Z")


def _name(value: str) -> str:
    if not SAFE_NAME.fullmatch(value) or ".." in value:
        raise ValueError(f"unsafe legacy path component: {value!r}")
    return value


def inventory(root: Path) -> list[str]:
    if (root / STAGE).exists():
        raise ValueError(f"partial migration at {root / STAGE}; restore the volume snapshot")
    if (root / MARKER).exists():
        raise ValueError(f"workspace migration marker already exists at {root / MARKER}")
    if not root.is_dir() or root.is_symlink():
        raise ValueError(f"cache root is not a directory: {root}")

    apps = []
    objects = 0
    for app in sorted(root.iterdir()):
        _name(app.name)
        if not app.is_dir() or app.is_symlink():
            raise ValueError(f"unexpected cache root entry: {app}")
        apps.append(app.name)
        for session in app.iterdir():
            _name(session.name)
            if not session.is_dir() or session.is_symlink():
                raise ValueError(f"unexpected legacy session entry: {session}")
            for entry in session.iterdir():
                if entry.is_symlink():
                    raise ValueError(f"symlink in cache data: {entry}")
                if entry.is_file() and entry.suffix == ".bin":
                    _name(entry.stem)
                    objects += 1
                elif entry.is_dir() and entry.name.endswith(".deltas"):
                    _name(entry.name.removesuffix(".deltas"))
                else:
                    raise ValueError(f"unexpected legacy object entry: {entry}")
    if apps and not objects:
        raise ValueError("no legacy object files found; layout is empty or already workspace-scoped")
    return apps


def migrate(root: Path, apply: bool = False) -> list[str]:
    apps = inventory(root)
    if not apply or not apps:
        return apps

    stage = root / STAGE
    stage.mkdir()
    for app in apps:
        os.replace(root / app, stage / app)

    destination = root / "default"
    destination.mkdir()
    for app in apps:
        os.replace(stage / app, destination / app)
    stage.rmdir()

    marker_tmp = root / f"{MARKER}.tmp"
    marker_tmp.write_text(json.dumps({"version": 1, "workspace": "default"}) + "\n")
    os.replace(marker_tmp, root / MARKER)
    return apps


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path, help="object cache disk storage root")
    parser.add_argument("--apply", action="store_true", help="perform the offline move after inventory")
    args = parser.parse_args()
    apps = migrate(args.root, args.apply)
    print(json.dumps({"mode": "apply" if args.apply else "dry-run", "applications": apps}))


if __name__ == "__main__":
    main()
