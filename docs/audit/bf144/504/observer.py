from pathlib import Path

previous=Path(__file__).resolve().parent.parent/'503/observer.py'
text=previous.read_text()
assert text.count('tools/tmp/bf144/503')==2
exec(compile(text.replace('tools/tmp/bf144/503','tools/tmp/bf144/504'),previous.name,'exec'))
root=Path(__file__).resolve().parent
path=Path('tools/tmp/bf144/504/playground_observed.rs')
driver=path.read_text()
driver=driver.replace('probe.frame >= 80','probe.frame >= 120').replace('probe.frame==80','probe.frame==120')
start=driver.index('        let rows=(95..=110).map')
end=driver.index('        exit.write(AppExit::Success);',start)
driver=driver[:start]+'''        c504_write(output,&probe);
'''+driver[end:]
path.write_text(driver+'\n'+(root/'tail_observer.rs').read_text())
print('Prepared the same one-frame control through first movement and stationary tail',flush=True)
