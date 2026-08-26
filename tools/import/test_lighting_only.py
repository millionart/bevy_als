import json
import math

extra = json.load(open("tmp/export/playground/scene_full.json", encoding="utf-8"))
ron = open("assets/models/playground/playground.ron", encoding="utf-8").read()
light_data = (extra.get("directional_lights") or [{}])[0]
sr = light_data.get("rotation", {})
pr = math.radians(sr.get("pitch", -45))
yr = math.radians(sr.get("yaw", 45))
sd = [round(-math.sin(yr) * math.cos(pr), 4), round(math.sin(pr), 4), round(math.cos(yr) * math.cos(pr), 4)]
intensity = round(float(light_data.get("intensity", 10.0)), 1)
ron = ron.replace(
    "actors: [",
    "lighting: Some((sun_direction: Some((%s, %s, %s)), sun_intensity: Some(%s), ambient_brightness: Some(2.0), camera_exposure_ev100: Some(2.0), camera_contrast: Some(1.05))),\n    actors: [" % (
        sd[0], sd[1], sd[2], intensity), 1)
open("assets/models/playground/playground_parity.ron", "w", encoding="utf-8", newline="\n").write(ron)
print("lighting only added")
