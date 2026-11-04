from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'537'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/537','tools/tmp/bf144/538')
script=script.replace("(root/'target_fin.inc')","(root.parent/'537'/'target_fin.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/538/playground_observed.rs')
driver=path.read_text()
anchor='    c533_install(&mut app);'
assert driver.count(anchor)==1
driver=driver.replace(anchor,anchor+'\n    c538_install(&mut app);')
path.write_text(driver+'\n'+(root/'solver_probe.rs').read_text())
print('Prepared live post-CMC collision masking and solver phase observation',flush=True)
