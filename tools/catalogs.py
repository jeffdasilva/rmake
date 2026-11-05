#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Verify the attributed, project-owned compatibility catalog assets."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = {"COPYING", "fr.po", "fr/LC_MESSAGES/make.mo"}


def verify(directory):
    directory = directory.resolve()
    manifest = json.loads((directory / "provenance.json").read_bytes())
    if set(manifest["files"]) != REQUIRED:
        raise ValueError("owned catalog/notice inventory differs")
    for name, expected in manifest["files"].items():
        path = directory / name
        if path.is_symlink() or not path.resolve().is_relative_to(directory):
            raise ValueError("catalog asset is outside its owned directory: " + name)
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise ValueError("catalog asset hash differs: " + name)
    header = manifest["translator_header"].encode()
    if not header or not (directory / "fr.po").read_bytes().startswith(header):
        raise ValueError("catalog translator/package notice differs")
    return len(REQUIRED)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=ROOT / "catalogs")
    arguments = parser.parse_args()
    try:
        count = verify(arguments.directory)
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f"catalogs: {error}\n")
    print(f"Verified {count} owned catalog and notice files.")


if __name__ == "__main__":
    main()
