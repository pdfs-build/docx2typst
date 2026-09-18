#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
import tomllib
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run the pinned public DOCX corpus through the CLI."
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("tests/corpus/public/manifest.toml"),
        help="Path to the public corpus manifest.",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(".artifacts/public-corpus"),
        help="Directory for conversion artifacts and summary output.",
    )
    parser.add_argument(
        "--ids",
        nargs="*",
        help="Optional fixture ids to run. Default runs the full manifest.",
    )
    parser.add_argument(
        "--keep-artifacts",
        action="store_true",
        help="Keep existing per-fixture output directories instead of deleting them first.",
    )
    return parser.parse_args()


def load_manifest(path: Path) -> list[dict]:
    return tomllib.loads(path.read_text()).get("fixture", [])


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    manifest_path = (repo_root / args.manifest).resolve()
    manifest_dir = manifest_path.parent
    fixtures = load_manifest(manifest_path)
    selected_ids = set(args.ids or [])
    if selected_ids:
        fixtures = [fixture for fixture in fixtures if fixture["id"] in selected_ids]
    out_dir = (repo_root / args.out).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    summary = {
        "manifest": str(manifest_path.relative_to(repo_root)),
        "out_dir": str(out_dir.relative_to(repo_root)),
        "fixtures": [],
    }
    failures = []

    for fixture in fixtures:
        fixture_id = fixture["id"]
        src = manifest_dir / fixture["path"]
        artifact_dir = out_dir / fixture_id
        if artifact_dir.exists() and not args.keep_artifacts:
            shutil.rmtree(artifact_dir)
        started = time.time()
        process = subprocess.run(
            [
                "cargo",
                "run",
                "-q",
                "-p",
                "docx2typst-cli",
                "--",
                "convert",
                str(src),
                "--out",
                str(artifact_dir),
            ],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        elapsed_ms = round((time.time() - started) * 1000)
        validation_path = artifact_dir / "validation.json"
        report_path = artifact_dir / "report.json"
        validation = {}
        report = {}
        if validation_path.exists():
            validation = json.loads(validation_path.read_text())
        if report_path.exists():
            report = json.loads(report_path.read_text())
        diagnostics = validation.get("diagnostics", [])
        report_diagnostics = report.get("diagnostics", [])
        compile_ok = validation.get("compile_ok", False)
        fallback_count = sum(
            1
            for diagnostic in report_diagnostics
            if diagnostic.get("code") == "DOCX_FALLBACK_RENDERED_ASSET"
        )
        baseline = fixture["baseline"]
        success = process.returncode == 0 and (
            baseline == "exploratory"
            or compile_ok
            and (baseline != "fallback" or fallback_count > 0)
        )
        if not success:
            failures.append(fixture_id)
        summary["fixtures"].append(
            {
                "id": fixture_id,
                "baseline": baseline,
                "source_path": str(src.relative_to(repo_root)),
                "compile_ok": compile_ok,
                "page_count": validation.get("page_count"),
                "diagnostic_count": len(diagnostics),
                "fallback_count": fallback_count,
                "elapsed_ms": elapsed_ms,
                "success": success,
                "stdout": process.stdout.strip(),
                "stderr": process.stderr.strip(),
            }
        )
        status = "ok" if success else "fail"
        print(
            f"{status:4} {fixture_id} "
            f"baseline={baseline} compile_ok={compile_ok} "
            f"fallbacks={fallback_count} elapsed_ms={elapsed_ms}"
        )

    summary_path = out_dir / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2))
    print(f"summary {summary_path.relative_to(repo_root)}")
    if failures:
        print(f"failed fixtures: {', '.join(failures)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
