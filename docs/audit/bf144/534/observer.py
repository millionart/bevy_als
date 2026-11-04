from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'533'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/533','tools/tmp/bf144/534')
script=script.replace("(root/'walking_probe.rs')","(root.parent/'533'/'walking_probe.rs')")
exec(compile(script,previous.name,'exec'))
