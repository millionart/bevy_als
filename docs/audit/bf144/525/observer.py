from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'524'/'observer.py'
script=previous.read_text().replace('tools/tmp/bf144/524','tools/tmp/bf144/525')
exec(compile(script,previous.name,'exec'))
