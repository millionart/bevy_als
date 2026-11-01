import hashlib
import json
import sys
from pathlib import Path

r = Path(__file__).resolve().parent
p = Path("tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp")
edits = [
    ("Controller->GetPlayerViewPoint(Location, ViewRotation);", "Location = FVector::ZeroVector; ViewRotation = FRotator::ZeroRotator;"),
    ('Request->TryGetField(TEXT("capture_player_view"))->Type != EJson::Boolean || ', ""),
    ("if (bCapturePlayerView)", "if (true)"),
]
s = p.read_text()
if sys.argv[1] == "restore":
    for original, replacement in edits:
        if replacement:
            assert s.count(replacement) == 1
            s = s.replace(replacement, original)
        else:
            anchor = '(!Request->TryGetBoolField(TEXT("capture_player_view"), bCapturePlayerView)))'
            assert s.count(anchor) == 1
            s = s.replace(anchor, '(' + original + anchor[1:])
else:
    assert hashlib.sha256(p.read_bytes()).hexdigest() == json.loads((r / "reader-final-sha256.json").read_text())["sha256"]
    for original, replacement in edits:
        assert s.count(original) == 1
        s = s.replace(original, replacement)
p.write_text(s)
if sys.argv[1] == "restore":
    assert hashlib.sha256(p.read_bytes()).hexdigest() == json.loads((r / "reader-final-sha256.json").read_text())["sha256"]
print("Reader variant prepared: " + sys.argv[1], flush=True)
