import gzip
import json
from pathlib import Path


directory = Path(__file__).resolve().parent
with gzip.open(directory / "roll.native.json.gz") as stream:
    probe = json.load(stream)["notify_dispatch_probe"]
assert not probe["tick_even_when_paused"]
rows = []
for row in probe["visibility_tick_options"]:
    recent = row["recent"]
    delta = row["delta_seconds"]
    for montage in [False, True]:
        source = (recent or montage) and not row["only_tick_montages"]
        actual = recent or (montage and delta == 0.0)
        assert actual == source
        rows.append({**row, "active_montage": montage, "legacy_updates_graph": recent,
                     "updates_graph": actual, "refresh_bones": recent})
assert len(rows) == 16
assert sum(row["legacy_updates_graph"] != row["updates_graph"] for row in rows) == 1

owner_delta = 1 / 60 * 0.0
assert any(not row["recent"] and row["delta_seconds"] == owner_delta and row["active_montage"]
           and row["updates_graph"] for row in rows)
print(json.dumps({"cases": rows, "hidden_owner_zero_updates_graph": True,
                  "hidden_paused_updates_graph": probe["tick_even_when_paused"]}, indent=2))
