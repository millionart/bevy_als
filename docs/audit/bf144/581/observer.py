from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'580'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/580','tools/tmp/bf144/581')
script=script.replace('580/initial.json','581/initial.json')
exec(compile(script,previous.name,'exec'))
driver=path.read_text()
a='"normal":normal,"hit_distance":hit.distance,"second_hit":second.map(|h|(name(h.entity),h.distance)),'
assert driver.count(a)==1
path.write_text(driver.replace(a,'''"normal":normal,"hit_distance":hit.distance,
            "second_normal":second.as_ref().map(|h|h.normal1),
            "second_fraction":second.as_ref().map(|h|
                bevy_als::als::motion::movement_sweep_travel(slide_length,h.distance)/slide_length),
            "second_hit":second.map(|h|(name(h.entity),h.distance)),'''))
print('Prepared actual repeated wall path and second-contact normal',flush=True)
