from pathlib import Path

previous = Path(__file__).resolve().parent.parent/'495/observer.py'
text = previous.read_text()
assert text.count('tools/tmp/bf144/495')==1
exec(compile(text.replace('tools/tmp/bf144/495','tools/tmp/bf144/496'),previous.name,'exec'))
