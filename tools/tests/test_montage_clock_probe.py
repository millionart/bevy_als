import json
import math
import sys
from pathlib import Path


def check(path):
    data=json.loads(Path(path).read_text(encoding='utf-8-sig'))
    samples={sample['name']:sample['frames'] for sample in data['montage_clock_probe']['samples']}
    zero=samples['zero']
    assert len(zero)==10 and all(frame['position']==0 and frame['weight']==1 and frame['valid'] for frame in zero)
    for frames in samples.values():
        assert all(math.isfinite(frame['position']) and 0<=frame['weight']<=1 for frame in frames)
        assert all(not frame['playing'] for frame in frames if not frame['valid'])
        assert all(a['position']<=b['position'] or
                   (abs(b['position']-1.49995)<1e-7 and not b['playing'])
                   for a,b in zip(frames,frames[1:]))
    regular=samples['regular']
    assert regular[0]['valid'] and regular[0]['weight']==1
    assert any(0<frame['weight']<1 for frame in regular)
    assert not regular[-1]['valid'] and regular[-1]['weight']==0
    interrupted=samples['interrupted']
    assert all(frame['weight']==1 for frame in interrupted[:4])
    assert abs(interrupted[4]['weight']-.84375)<1e-6
    assert abs(interrupted[5]['weight']-.5)<1e-6
    assert not interrupted[7]['valid'] and interrupted[7]['weight']==0
    shortened=samples['shortened']
    assert shortened[4]['valid'] and not shortened[5]['valid']
    print('Montage clock probe self-test: PASS',flush=True)


if __name__=='__main__':
    check(sys.argv[1])
