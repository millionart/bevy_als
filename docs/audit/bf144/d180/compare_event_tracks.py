import difflib
import hashlib
import json
import subprocess
import sys
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
sys.path.insert(0, str(root / "tools/import"))
from compile_als_event_tracks import remove_event_tracks, render_tracks, tracks, update_graph_clip

output = Path(sys.argv[1])
baseline = sys.argv[2] if len(sys.argv) > 2 else None
output.mkdir(parents=True, exist_ok=True)
assets = root / "assets"
manifest = json.loads((assets / "ALS/Animations/als_notify_manifest.json").read_bytes())
compiled = tracks(manifest)
changes = []
with (output / "tracks-full.diff").open("w", encoding="utf-8") as diff:
    for clip, track_map in sorted(compiled.items()):
        before = (subprocess.check_output(["git", "show", f"{baseline}:assets/{clip}"], cwd=root)
                  if baseline else (assets / clip).read_bytes())
        staged = output / "assets" / clip
        staged.parent.mkdir(parents=True, exist_ok=True)
        staged.write_bytes(before)
        block = render_tracks(track_map)
        update_graph_clip(staged, block, check=False)
        update_graph_clip(staged, block, check=True)
        after = staged.read_bytes()
        old_text, new_text = (data.decode().replace("\r\n", "\n") for data in (before, after))
        assert remove_event_tracks(old_text) == remove_event_tracks(new_text), clip
        if before != after:
            changes.append({"file": clip, "before_sha256": hashlib.sha256(before).hexdigest(),
                            "after_sha256": hashlib.sha256(after).hexdigest()})
            lines = list(difflib.unified_diff(old_text.splitlines(keepends=True), new_text.splitlines(keepends=True),
                                              fromfile="a/assets/" + clip, tofile="b/assets/" + clip))
            assert all(line[1:].strip().startswith(("start_time:", "end_time:")) for line in lines[2:]
                       if line.startswith(("+", "-"))), clip
            diff.writelines(lines)

result = {"clips": len(compiled), "tracks": sum(len(track_map) for track_map in compiled.values()),
          "events": sum(len(events) for track_map in compiled.values() for events in track_map.values()),
          "changed_fields": ["start_time", "end_time"], "changes": changes}
(output / "track-diff.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps({**{key: value for key, value in result.items() if key != "changes"}, "changed_clips": len(changes)}))
