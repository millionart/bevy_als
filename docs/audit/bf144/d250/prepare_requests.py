import json
from pathlib import Path

root = Path("docs/audit/bf144")
output = root / "d250"
for boundary in ("before", "after"):
    data = json.loads((root / "476" / f"weights-{boundary}.request.json").read_text())
    data["cases"] = [case for case in data["cases"] if case["name"] == "setter_weight_0.25"]
    (output / f"weights-{boundary}.request.json").write_text(json.dumps(data, separators=(",", ":")) + "\n")

rows = json.loads((root / "480" / "negative-inputs-slerp.json").read_text())["source"]
samples = json.loads((root / "480" / "core.request.json").read_text())["samples"]
request = {"samples": [samples[row["core_sample"]] for row in rows]}
(output / "core.request.json").write_text(json.dumps(request, separators=(",", ":")) + "\n")
