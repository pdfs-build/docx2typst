#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
import sys
import tomllib
import urllib.request
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Download the pinned public DOCX corpus fixtures."
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("tests/corpus/public/manifest.toml"),
        help="Path to the public corpus manifest.",
    )
    parser.add_argument(
        "--refresh",
        action="store_true",
        help="Re-download fixtures even if they already exist.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    manifest_path = (repo_root / args.manifest).resolve()
    manifest_dir = manifest_path.parent
    manifest = tomllib.loads(manifest_path.read_text())
    fixtures = manifest.get("fixture", [])
    downloaded = 0
    skipped = 0

    for fixture in fixtures:
        destination = manifest_dir / fixture["path"]
        source = fixture["source"]
        url = source["url"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        if destination.exists() and not args.refresh:
            skipped += 1
            print(f"skip  {fixture['id']}: {destination.relative_to(repo_root)}")
            continue

        print(f"fetch {fixture['id']}: {url}")
        with urllib.request.urlopen(url) as response:
            payload = response.read()
        destination.write_bytes(payload)
        sha256 = hashlib.sha256(payload).hexdigest()
        downloaded += 1
        print(
            f"ok    {fixture['id']}: {destination.relative_to(repo_root)} "
            f"({len(payload)} bytes, sha256={sha256})"
        )

    print(
        f"done  downloaded={downloaded} skipped={skipped} "
        f"fixtures={len(fixtures)} manifest={manifest_path.relative_to(repo_root)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
