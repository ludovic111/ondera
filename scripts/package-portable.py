#!/usr/bin/env python3
"""Build the exact portable update archive or the verified stock-plugin demo."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile


def package(source, output, kind):
    if kind == "demo":
        names = ["Afterglow.ondera", "Afterglow.wav", "Afterglow.mid", "verification.json"]
        report = json.loads((source / "verification.json").read_text())
        if report.get("instrument") != "stock" or report.get("effect") != "stock":
            raise ValueError("The release demo must use only stock plugins")
        if report.get("mode") != "headless" or not report.get("validation", "").startswith("Valid session:"):
            raise ValueError("The release demo must have completed isolated session verification")
        if hashlib.sha256((source / "Afterglow.wav").read_bytes()).hexdigest() != report.get("sha256"):
            raise ValueError("The demo mix no longer matches its verification report")
    else:
        extension = ".exe" if kind == "windows" else ""
        names = [name + extension for name in ["ondera", "ondera-cli", "ondera-mcp"]]
    for name in names:
        path = source / name
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError("Missing or invalid package file: " + str(path))
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED) as archive:
        for name in names:
            archive.write(source / name, arcname=name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=["linux", "windows", "demo"])
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    package(args.source, args.output, args.kind)


if __name__ == "__main__":
    main()
