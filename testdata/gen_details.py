#!/usr/bin/env python3
"""Generate the track-details conformance files: small MP3/FLAC files with Xing/LAME headers, ReplayGain tags,
embedded pictures and odd values, plus the records tau-alpha's reference extractor (tools/tau_details.py) reads
from them. Run from the Tau Omega directory with TAU_REFERENCE pointing at a tau-alpha checkout at main.
The outputs under testdata/details/ are checked in; never edit them by hand."""
import json, os, struct, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REF = Path(os.environ.get("TAU_REFERENCE") or HERE.parent.parent / "tau-alpha")
sys.path.insert(0, str(REF / "tools"))
import tau_details  # noqa: E402

ROOT_DIR = HERE / "details"
OUT = ROOT_DIR / "common" / "Details"
import tau_library  # noqa: E402


def ss(n):
    return bytes([(n >> 21) & 127, (n >> 14) & 127, (n >> 7) & 127, n & 127])


def frame(fid, body, ver):
    size = ss(len(body)) if ver == 4 else struct.pack(">I", len(body))
    return fid.encode() + size + b"\0\0" + body


def txxx(desc, val, enc=0):
    if enc == 1:
        return bytes([1]) + (desc + "\0" + val).encode("utf-16")
    if enc == 3:
        return bytes([3]) + (desc + "\0" + val).encode("utf-8")
    return bytes([0]) + (desc + "\0" + val).encode("latin-1")


def id3(frames, ver=3):
    body = b"".join(frames)
    return b"ID3" + bytes([ver, 0, 0]) + ss(len(body)) + body


def mpeg_header(version, bitrate_index, rate_index, mono):
    b1 = 0xE0 | (version << 3) | (1 << 1) | 1
    return bytes([0xFF, b1, (bitrate_index << 4) | (rate_index << 2), (3 << 6) if mono else 0])


def xing(kind, frames, nbytes, delay, pad, version=3, mono=False, name=b"LAME3.99r"):
    side = (17 if mono else 32) if version == 3 else (9 if mono else 17)
    h = mpeg_header(version, 9, 0, mono)
    body = b"\0" * side + kind + struct.pack(">I", 3) + struct.pack(">II", frames, nbytes)
    ext = name.ljust(9, b" ") + b"\0" * 12 + bytes([(delay >> 4) & 255, ((delay & 15) << 4) | (pad >> 8), pad & 255])
    return h + body + ext + b"\0" * 400


def plain(version=3, br=9, ri=0, mono=False):
    return (mpeg_header(version, br, ri, mono) + b"\0" * 400) * 3


def flac(rate, ch, bits, total, rg=None, vendor="reference libFLAC 1.4.3", pic=0):
    si = bytearray(34)
    si[10] = (rate >> 12) & 255
    si[11] = (rate >> 4) & 255
    si[12] = ((rate & 15) << 4) | (((ch - 1) & 7) << 1) | (((bits - 1) >> 4) & 1)
    si[13] = ((((bits - 1) & 15) << 4) | ((total >> 32) & 15))
    si[14:18] = struct.pack(">I", total & 0xFFFFFFFF)
    blocks = [(0, bytes(si))]
    comments = [f"{k}={v}" for k, v in (rg or {}).items()] + ["TITLE=x"]
    v = vendor.encode()
    vc = struct.pack("<I", len(v)) + v + struct.pack("<I", len(comments))
    for c in comments:
        cb = c.encode()
        vc += struct.pack("<I", len(cb)) + cb
    blocks.append((4, vc))
    if pic:
        blocks.append((6, b"\0" * pic))
    out = b"fLaC"
    for i, (t, b) in enumerate(blocks):
        out += bytes([(0x80 if i == len(blocks) - 1 else 0) | t]) + len(b).to_bytes(3, "big") + b
    return out + b"\xff\xf8" + b"\0" * 3000


CASES = {
    "mp3_cbr_plain.mp3": plain(),
    "mp3_mono_22k.mp3": plain(2, 8, 1, True),
    "mp3_lame_vbr.mp3": id3([frame("TIT2", b"\0x", 3)]) + xing(b"Xing", 5000, 6_000_000, 1105, 1232),
    "mp3_lame_cbr_info.mp3": xing(b"Info", 2000, 0, 576, 1000),
    "mp3_lame_mono_v2.mp3": xing(b"Xing", 900, 400_000, 576, 100, version=2, mono=True),
    "mp3_rg_latin1.mp3": id3([frame("TXXX", txxx("REPLAYGAIN_TRACK_GAIN", "-6.54 dB"), 3),
                              frame("TXXX", txxx("REPLAYGAIN_TRACK_PEAK", "0.987654"), 3),
                              frame("TXXX", txxx("replaygain_album_gain", "+1.005 dB"), 3),
                              frame("TXXX", txxx("REPLAYGAIN_ALBUM_PEAK", "1.9999"), 3)]) + plain(),
    "mp3_rg_utf16_v24.mp3": id3([frame("TXXX", txxx("REPLAYGAIN_TRACK_GAIN", "-3.20 dB", 1), 4),
                                 frame("TXXX", txxx("REPLAYGAIN_TRACK_PEAK", "1.0", 1), 4)], 4) + plain(),
    "mp3_rg_utf8.mp3": id3([frame("TXXX", txxx("REPLAYGAIN_ALBUM_GAIN", "-18.5", 3), 3)]) + plain(),
    "mp3_rg_odd.mp3": id3([frame("TXXX", txxx("REPLAYGAIN_TRACK_GAIN", "-0.005 dB"), 3),
                           frame("TXXX", txxx("REPLAYGAIN_TRACK_PEAK", "1.5"), 3),
                           frame("TXXX", txxx("REPLAYGAIN_ALBUM_GAIN", "junk"), 3),
                           frame("TXXX", txxx("REPLAYGAIN_ALBUM_PEAK", ".5"), 3)]) + plain(),
    "mp3_cover.mp3": id3([frame("APIC", b"\0image/jpeg\0\3\0" + b"\xff" * 5000, 3)]) + plain(),
    "mp3_big_gain.mp3": id3([frame("TXXX", txxx("REPLAYGAIN_TRACK_GAIN", "+99 dB"), 3),
                             frame("TXXX", txxx("REPLAYGAIN_ALBUM_GAIN", "-99 dB"), 3)]) + plain(),
    "extra/mp3_peak_overflow.mp3": id3([frame("TXXX", txxx("REPLAYGAIN_TRACK_PEAK", "12.5"), 3)]) + plain(),  # the reference cannot pack this
    "mp3_no_frames.mp3": b"\0" * 5000,
    "flac_44k_16.flac": flac(44100, 2, 16, 44100 * 60),
    "flac_96k_24.flac": flac(96000, 2, 24, 96000 * 30, {"REPLAYGAIN_TRACK_GAIN": "-4.1 dB", "REPLAYGAIN_TRACK_PEAK": "0.5",
                                                      "REPLAYGAIN_ALBUM_GAIN": "-5.0 dB", "REPLAYGAIN_ALBUM_PEAK": "0.99"}),
    "flac_mono_48k.flac": flac(48000, 1, 16, 48000 * 10, vendor="Lavf60", pic=20000),
    "flac_6ch.flac": flac(48000, 6, 24, 48000 * 5),
    "flac_unknown_length.flac": flac(44100, 2, 16, 0),
}


def main():
    import shutil
    shutil.rmtree(ROOT_DIR, ignore_errors=True)
    OUT.mkdir(parents=True)
    expected = {}
    for name, data in CASES.items():
        path = ROOT_DIR / name if name.startswith("extra/") else OUT / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        d = tau_details.extract(path)
        for key in ("tg", "ag", "tp", "ap", "delay", "pad"):
            d.setdefault(key, 0)
        # the reference's pack() would raise on a peak above 65535 (the Rust side clamps); record the clamp
        for key in ("tp", "ap"):
            d[key] = min(d[key], 65535)
        d["enc"] = d.get("enc", "")
        expected[name.removeprefix('extra/')] = {k: d[k] for k in ("codec", "ch", "bits", "flags", "rate", "size", "kbps", "cover_kib", "tg", "ag", "tp", "ap", "delay", "pad", "enc")}
    (ROOT_DIR / "common" / "Details" / "tau-art").mkdir()
    entries, playlists, _ = tau_library.scan(ROOT_DIR / "common", playlists=False)
    (ROOT_DIR / "index.tdb").write_bytes(tau_library.build_index(entries, playlists, []))
    (ROOT_DIR / "expected.json").write_text(json.dumps(expected, indent=1, sort_keys=True))
    print(f"wrote {len(CASES)} files and expected.json (reference {REF})")


main()
