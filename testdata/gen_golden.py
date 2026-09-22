#!/usr/bin/env python3
"""Generate T1 conformance files only from the read-only Python oracle.

Run from the Tau Omega directory.  The resulting binary files are checked
in; do not edit them by hand.  The script writes exclusively under testdata/.
"""
from __future__ import annotations

import importlib.util
import json
import shutil
import struct
import sys
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
REFERENCE = ROOT.parent / "tau-alpha"
sys.path.insert(0, str(REFERENCE / "tools"))
import tau_library as library  # noqa: E402


def load_fixture_tree():
    spec = importlib.util.spec_from_file_location("tau_index_sim", REFERENCE / "sim" / "test_library_index.py")
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.tree


def reseal(data: bytes) -> bytes:
    out = bytearray(data)
    crc = zlib.crc32(out[128:]) & 0xFFFFFFFF
    struct.pack_into("<II", out, 16, crc, len(out))
    struct.pack_into("<I", out, 24, crc)
    struct.pack_into("<I", out, 124, zlib.crc32(out[:124]) & 0xFFFFFFFF)
    return bytes(out)


def corruption_matrix(data: bytes) -> dict[str, dict[str, object]]:
    ix = library.parse(data)
    tracks = ix.sec["tracks"][0]
    strings = ix.sec["strings"][0]
    cases = {
        "bad_magic": bytes(bytearray(data[:1]) + bytes([data[0] ^ 0xFF]) + data[2:]),
        "truncated": data[:-16],
        "appended": data + b"\0" * 16,
        "body_flip": data[: tracks + 3] + bytes([data[tracks + 3] ^ 0xFF]) + data[tracks + 4 :],
        "string_flip": data[: strings + 5] + bytes([data[strings + 5] ^ 0xFF]) + data[strings + 6 :],
    }
    malformed = bytearray(data)
    struct.pack_into("<I", malformed, tracks, 0x7FFFFF)
    cases["track_string_offset"] = reseal(bytes(malformed))
    output = {}
    for name, blob in cases.items():
        (HERE / "corruption" / f"{name}.tdb").write_bytes(blob)
        try:
            library.parse(blob)
            code = 0
        except library.LibError as error:
            code = error.code
        output[name] = {"file": f"corruption/{name}.tdb", "code": code}
    return output


def main() -> None:
    for path in (HERE / "fixture", HERE / "corruption"):
        if path.exists():
            shutil.rmtree(path)
    (HERE / "fixture").mkdir(parents=True)
    (HERE / "corruption").mkdir(parents=True)
    common = HERE / "fixture" / "common"
    load_fixture_tree()(common)
    entries, playlists, warnings = library.scan(common, playlists=True)
    fixture = library.build_index(entries, playlists, [])
    (HERE / "fixture.tdb").write_bytes(fixture)
    for tracks, albums, artists in ((400, 40, 20), (7180, 800, 300), (16384, 2048, 1024)):
        entries = library.synth(tracks, albums, artists)
        # The Rust conformance test receives these oracle inputs rather than
        # duplicating Python's random-number generator in the fixture layer.
        # It must still independently reproduce the resulting index bytes.
        (HERE / f"synth-{tracks}.json").write_text(json.dumps(entries, separators=(",", ":")))
        (HERE / f"synth-{tracks}.tdb").write_bytes(library.build_index(entries))
        (HERE / f"synth-{tracks}-playlists.tdb").write_bytes(
            library.build_index(entries, [{"name": "Golden list", "rel_ids": list(range(min(tracks, 16)))}])
        )
    vectors = {
        "ascii": {"Nausicaä – Mönch": "Nausicaa Monch", "  A\tName  ": "A Name", "東京": ""},
        "sort": ["The Beta", "A 10", "A 2", "9 Lives", "Zed"],
        "fixture_warnings": warnings,
        "corruption": corruption_matrix(fixture),
    }
    (HERE / "vectors.json").write_text(json.dumps(vectors, indent=2) + "\n")
    print("generated golden files from", REFERENCE)


if __name__ == "__main__":
    main()
