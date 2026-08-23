import json, glob
bad = 0
files = glob.glob("assets/animations/**/*.gltf", recursive=True)
with_ext = 0
for f in files:
    try:
        d = json.load(open(f, encoding="utf-8"))
    except Exception as e:
        print("BAD JSON", f, e); bad += 1; continue
    anims = d.get("animations") or []
    if len(anims) != 1:
        print("BAD anims", f, len(anims)); bad += 1; continue
    ext = anims[0].get("extensions", {}).get("ALS_float_curves")
    if ext:
        with_ext += 1
        for c in ext["curves"]:
            n = len(c["times"])
            assert n == len(c["values"]) == len(c["interps"]), (f, c["name"])
            assert all(t2 >= t1 for t1, t2 in zip(c["times"], c["times"][1:])), (f, c["name"])
print("files=%d with_ext=%d bad=%d" % (len(files), with_ext, bad))
