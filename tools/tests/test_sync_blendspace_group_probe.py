import json
import math
import struct
import sys
from pathlib import Path

request=json.loads(Path(__file__).with_name('fixtures').joinpath('sync_blendspace_group.request.json').read_text())
probe=json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['sync_group_clock_probe']
f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]
reference={row['asset_path']:row for row in request['expected_sequences']}
def marker_time(asset,position):
    times={marker['name']:marker['time'] for marker in asset['markers']}
    start=times[position['previous']]
    end=times[position['next']]
    if end<=start:
        end+=asset['duration']
    return (start+(end-start)*position['alpha'])%asset['duration']

assert len(probe['players'])==len(request['players'])==2
assert all(player['time_unit']=='normalized' for player in probe['players'])
assert len(probe['samples'])==len(request['samples'])
count=0
for expected,actual in zip(request['samples'],probe['samples']):
    assert expected['name']==actual['name'] and len(expected['steps'])==len(actual['frames'])
    previous=[f32(player['start']) for player in request['players']]
    for step,frame in zip(expected['steps'],actual['frames']):
        assert frame['leader']==0 and frame['marker_start']['valid'] and frame['marker_end']['valid']
        assert frame['order']==[i for i,value in enumerate(step['weights']) if value is not None]
        for index,clock in enumerate(frame['players']):
            active=step['weights'][index] is not None
            assert clock['ticked']==active
            if not active:
                assert clock['position']==previous[index] and not clock['samples']
                continue
            assert clock['has_previous'] and clock['previous']==previous[index]
            assert clock['playback_delta']==f32(step['delta_seconds'])
            assert 0<=clock['position']<=1
            assert [clock['filtered_position'][axis] for axis in ['x','y']]==request['players'][index]['blend_position']
            assert abs(sum(sample['weight'] for sample in clock['samples'])-1)<1e-6
            ratios=[]
            for sample in clock['samples']:
                asset=reference[sample['animation']]
                assert sample['weight']>0 and sample['play_rate']==1
                assert math.isfinite(sample['time']) and math.isfinite(sample['previous_time'])
                assert abs(sample['time']-marker_time(asset,frame['marker_end']))<2e-6
                assert abs(sample['previous_time']-marker_time(asset,frame['marker_start']))<2e-6
                ratios.append(f32(sample['time']/asset['duration']))
            assert clock['position'] in ratios
            previous[index]=clock['position']
        count+=1
assert count==160
print(f'Native BlendSpace group units, independent inputs, sample clocks and markers PASS: {count} frames')
