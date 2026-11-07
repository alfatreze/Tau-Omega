#!/usr/bin/env python3
"""The preset set for the real-card test (docs/REAL_CARD_PRESETS_TEST.md), made from what a card holds: Winamp Bars (meter id 12) with
one preset renamed (id 1), one deleted (id 3) and one custom added (id 5, values changed from the defaults). (The first real-card run also set
the index's hidden bit on id 4; hidden is Omega-only now (D-022), so this script no longer does.)

  tau meter-presets-show <media-root> --registry <registry> --json > card.json
  python3 tools/preset_scenario.py card.json > sets.json            (prints a JSON list of PresetSet for `meter-presets-plan --sets`)
  python3 tools/preset_scenario.py card.json --without 5 > sets2.json   (the set on the card minus preset id 5: the "chosen preset vanishes" step)
  python3 tools/preset_scenario.py card.json --undo > undo.json     (the set as card.json had it: writing it puts the card back)

Reads only the JSON file it is given; never touches a card."""
import json, sys

METER = 12
argv = sys.argv[1:]
without = int(argv[argv.index("--without") + 1]) if "--without" in argv else None
args = [a for i, a in enumerate(argv) if not a.startswith("--") and not (i and argv[i - 1] == "--without")]
rows = json.load(open(args[0]))
row = next(r for r in rows if r["meter_id"] == METER)
if row["on_card"] is None:
    sys.exit("meter 12 has no presets on this card: install the factory set first (tools/install_factory_presets.py in Tau Alpha)")
presets = row["on_card"]["presets"]
if "--undo" in sys.argv:
    print(json.dumps([row["on_card"]], indent=1)); sys.exit(0)
if without is not None:
    if without not in {p["id"] for p in presets}:
        sys.exit("preset id %d is not on the card" % without)
    print(json.dumps([{"meter_id": METER, "presets": [p for p in presets if p["id"] != without]}], indent=1)); sys.exit(0)
by = {p["id"]: p for p in presets}
need = {0, 1, 2, 3, 4}
if not need <= set(by) or 5 in by:
    sys.exit("expected the factory ids 0..4 and no id 5; this card differs (%s): stop and ask" % sorted(by))
out = []
for p in presets:
    p = json.loads(json.dumps(p))
    if p["id"] == 3:            # deleted
        continue
    if p["id"] == 1:            # renamed
        p["name"] = "CLASSIC 2"
    out.append(p)
# a custom preset of its own: a copy of FLUID's values with the bands changed (to a legal value that differs from FLUID's)
custom = json.loads(json.dumps(by[0]))
custom.update({"id": 5, "name": "TEST ONE", "hidden": False})
custom["values"]["bands"] = 8 if custom["values"].get("bands") != 8 else 12      # the legal range is 4..16
out.append(custom)
print(json.dumps([{"meter_id": METER, "presets": out}], indent=1))
