import json
import math
import re

extra = json.load(open("tmp/export/playground/scene_full.json", encoding="utf-8"))
ron = open("assets/models/playground/playground_parity.ron", encoding="utf-8").read()

text_by_xy = {}
for t in extra.get("text_render_actors", []):
    loc = t["location"]
    text_by_xy[(round(loc["x"]), round(loc["y"]))] = t

lines = ron.split("\n")
out = []
text_count = 0

for line in lines:
    if "TextRenderActor" in line and "location:" in line:
        xm = re.search(r"location: \((-?\d+(?:\.\d+)?)", line)
        ym = re.search(r", (-?\d+(?:\.\d+)?), -?\d+(?:\.\d+)?\)", line)
        if xm and ym:
            key = (round(float(xm.group(1))), round(float(ym.group(1))))
            t = text_by_xy.get(key)
            if t:
                txt = t["text"].replace("\\", "\\\\").replace('"', "'")
                txt = txt.replace("\r\n", "\\n").replace("\n", "\\n")
                col = t.get("color", [1.0, 1.0, 1.0, 1.0])
                while len(col) < 4:
                    col.append(1.0)
                ws = int(t.get("world_size", 30))
                stripped = line.rstrip()
                if stripped.endswith(")),"):
                    ins = ', text: Some("%s"), text_color: Some([%s]), text_world_size: Some(%d)' % (
                        txt,
                        ", ".join(str(round(float(c), 3)) for c in col[:4]),
                        ws,
                    )
                    # line ends like: ...bounds_extent: (...37.5)),
                    # We must insert between the two closing parens:
                    #   ...(37.5) <- closes bounds_extent
                    #             <- INSERT HERE
                    #          ) <- closes actor entry
                    # Find the second-to-last ')' position
                    close_actor_pos = stripped.rfind(")")       # last ')' = actor close
                    close_bounds_pos = stripped.rfind(")", 0, close_actor_pos)  # second-to-last = bounds close
                    if close_bounds_pos > 0:
                        line = (
                            stripped[:close_bounds_pos + 1]
                            + ins
                            + stripped[close_bounds_pos + 1:]
                        )
                    else:
                        line = stripped
                    text_count += 1
    out.append(line)

ron = "\n".join(out)

light_data = (extra.get("directional_lights") or [{}])[0]
sr = light_data.get("rotation", {})
pr = math.radians(sr.get("pitch", -45))
yr = math.radians(sr.get("yaw", 45))
sd = [
    round(-math.sin(yr) * math.cos(pr), 4),
    round(math.sin(pr), 4),
    round(math.cos(yr) * math.cos(pr), 4),
]
intensity = round(light_data.get("intensity", 3.0) * 10000.0, 1)

lighting_line = (
    "lighting: Some((sun_direction: Some([%s, %s, %s]), "
    "sun_intensity: Some(%s), ambient_brightness: Some(400.0))),"
    % (sd[0], sd[1], sd[2], intensity)
)

ron = ron.replace(
    "actors: [",
    lighting_line + "\n    actors: [",
    1,
)

open("assets/models/playground/playground_parity.ron", "w", encoding="utf-8", newline="\n").write(ron)
print(f"text_count={text_count}")
