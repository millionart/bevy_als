import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');q=Path('target/ue_graph_source_reader_package/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp');assert p.read_bytes()==q.read_bytes()
(r/(sys.argv[1]+'-package-source-check.json')).write_text(json.dumps(dict(exact=True,sha256=hashlib.sha256(p.read_bytes()).hexdigest()),indent=2)+'\n')
print('Verified native package and project reader source bytes '+sys.argv[1],flush=True)
