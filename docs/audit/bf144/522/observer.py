from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'521'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/521','tools/tmp/bf144/522')
script=script.replace("(root/'trace.rs')","(root.parent/'521'/'trace.rs')")
exec(compile(script,previous.name,'exec'))
root=Path(__file__).resolve().parent
path=Path('tools/tmp/bf144/522/playground_observed.rs')
driver=path.read_text()
key='    c520_install(&mut app);'
assert driver.count(key)==1
driver=driver.replace(key,key+'\n    c522_install(&mut app);')
path.write_text(driver+'\n'+(root/'contact.rs').read_text())
print('Prepared live sweep contact lifecycle observer',flush=True)
