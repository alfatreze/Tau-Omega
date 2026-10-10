#!/usr/bin/env python3
"""Builds the updater's `latest.json` from a folder of release assets: python3 tools/make_latest_json.py <assets-dir> <tag> <repo> [notes-file]
Every updater bundle needs its `.sig` beside it. Platforms: darwin-aarch64 / darwin-x86_64 (`*aarch64*.app.tar.gz` / `*x64*.app.tar.gz`),
windows-x86_64 (`*-setup.exe`), linux-x86_64 (`*.AppImage`). Refuses a missing platform or signature: a half-published update
must not exist. The file must be uploaded to the same release as the bundles (the app reads releases/latest/download/latest.json)."""
import datetime, json, pathlib, sys

PLATFORMS = {
    "darwin-aarch64": lambda n: n.endswith(".app.tar.gz") and "aarch64" in n,
    "darwin-x86_64": lambda n: n.endswith(".app.tar.gz") and ("x64" in n or "x86_64" in n),
    "windows-x86_64": lambda n: n.endswith(".exe") and "setup" in n,
    "linux-x86_64": lambda n: n.endswith(".AppImage"),
}


def build(assets: pathlib.Path, tag: str, repo: str, notes: str = "") -> dict:
    files = sorted(p for p in assets.iterdir() if p.is_file() and not p.name.endswith(".sig"))
    platforms = {}
    for key, match in PLATFORMS.items():
        hits = [p for p in files if match(p.name)]
        if len(hits) != 1:
            raise SystemExit(f"{key}: expected exactly one bundle, found {[p.name for p in hits]}")
        sig = hits[0].with_name(hits[0].name + ".sig")
        if not sig.is_file():
            raise SystemExit(f"{key}: {hits[0].name} has no .sig (was the signing key set during the build?)")
        platforms[key] = {"signature": sig.read_text().strip(), "url": f"https://github.com/{repo}/releases/download/{tag}/{hits[0].name}"}
    return {"version": tag.lstrip("v"), "notes": notes, "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), "platforms": platforms}


if __name__ == "__main__":
    if len(sys.argv) < 4:
        raise SystemExit(__doc__)
    notes = pathlib.Path(sys.argv[4]).read_text() if len(sys.argv) > 4 else ""
    out = build(pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3], notes)
    (pathlib.Path(sys.argv[1]) / "latest.json").write_text(json.dumps(out, indent=2) + "\n")
    print(json.dumps(out, indent=2))
