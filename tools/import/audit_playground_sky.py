import argparse
import colorsys
import json
import sys
import subprocess
from functools import lru_cache
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'animgraph_authoring'))
from ue_animation_asset_resolver import object_property, source_object_path
from ue_exported_text import parse_struct, parse_bool


def sample_curve(keys,time):
    if not keys:return 1.0
    if time<=keys[0]['t']:return keys[0]['v']
    if time>=keys[-1]['t']:return keys[-1]['v']
    left,right=next((a,b) for a,b in zip(keys,keys[1:]) if a['t']<=time<b['t'])
    alpha=(time-left['t'])/(right['t']-left['t'])
    if left['interpolation']=='Constant':return left['v']
    if left['interpolation']=='Linear':return left['v']+(right['v']-left['v'])*alpha
    assert left['tangent_weight_mode']=='None' and right['tangent_weight_mode']=='None'
    span=right['t']-left['t']
    return (2*alpha**3-3*alpha**2+1)*left['v']+(alpha**3-2*alpha**2+alpha)*span*left['leave_tangent']+(-2*alpha**3+3*alpha**2)*right['v']+(alpha**3-alpha**2)*span*right['arrive_tangent']


def refresh(definition):
    height=definition['sun_height']
    if definition['sun'] is not None:
        height=definition['sun']['pitch']/-90
        pitch,yaw=definition['sun']['pitch'],definition['sun']['yaw']
        sun_color=definition['sun']['color'][:3]
    else:
        pitch,yaw=-90*height,definition['actor_yaw']
        t=np.clip(height+.2,0,1)
        sun_color=np.array([1,.221,.04])*(1-t)+np.array([.954,.901,.74412])*t
    pitch,yaw=np.radians([pitch,yaw])
    direction=np.array([np.cos(pitch)*np.cos(yaw),np.cos(pitch)*np.sin(yaw),np.sin(pitch)])
    values={'Light direction':direction,'Sun color':sun_color,'Overall Color':definition['overall'][:3]}
    for field,name in [('zenith','Zenith Color'),('horizon','Horizon color'),('cloud','Cloud color')]:
        if definition['colors_by_sun']:
            rgb=np.array([sample_curve(keys,height) for keys in definition['curves'][field][:3]])
            hsv=colorsys.rgb_to_hsv(*rgb) if rgb.max()>0 else (0,0,0)
            rgb=colorsys.hsv_to_rgb(hsv[0],np.clip(hsv[1],0,1),np.clip(hsv[2],0,1))
        else:rgb=definition[field][:3]
        values[name]=np.array(rgb)
    values.update({'Horizon Falloff':3+4*abs(height) if definition['colors_by_sun'] else definition['horizon_falloff'],
        'Sun height':abs(height) if height<0 else 0,'Sun brightness':definition['sun_brightness'],
        'Cloud speed':definition['cloud_speed'],'Cloud opacity':definition['cloud_opacity'],'Stars brightness':definition['stars_brightness']})
    return values


class SourceMaterial:
    def __init__(self,source,directory,node_executable='node',pngjs_module='pngjs'):
        self.objects={o['object_path']:o for o in source['asset_objects']}
        editor=next(o for o in self.objects.values() if o['class_path'].endswith('MaterialEditorOnlyData'))
        self.output=source_object_path(parse_struct(object_property(editor,'EmissiveColor'))['Expression'])
        self.directory=directory
        self.node_executable=node_executable
        self.pngjs_module=pngjs_module

    @lru_cache
    def pixels(self,path):
        name=path.rsplit('.',1)[-1]
        metadata=json.loads((self.directory/(name+'.ue.json')).read_text(encoding='utf-8-sig'))
        if metadata['source_pixel_format']=='RGBA16':
            script="const p=require(process.argv[1]).PNG.sync.read(require('fs').readFileSync(process.argv[2]),{skipRescale:true});process.stdout.write(Buffer.from(p.data.buffer,p.data.byteOffset,p.data.byteLength));"
            decoded=subprocess.run([self.node_executable,'-e',script,self.pngjs_module,str(self.directory/metadata['source_image'])],capture_output=True,check=True,timeout=30).stdout
            image=np.frombuffer(decoded,dtype='<u2').reshape(metadata['source_height'],metadata['source_width'],4).astype(np.float64)/65535
        else:image=np.asarray(Image.open(self.directory/metadata['source_image']).convert('RGBA'),dtype=np.float64)/255
        if metadata['source_pixel_format']!='RGBA16' and parse_bool(object_property({'properties':metadata['asset_properties']},'SRGB')):
            rgb=image[:,:,:3]
            image[:,:,:3]=np.where(rgb<=.04045,rgb/12.92,((rgb+.055)/1.055)**2.4)
        return image

    def sample(self,path,uv):
        image=self.pixels(path)
        h,w=image.shape[:2]
        p=np.asarray(uv)*[w,h]-.5
        x,y=np.moveaxis(np.floor(p).astype(int),-1,0)
        a,b=np.moveaxis(p-np.floor(p),-1,0)
        a,b=np.asarray(a)[...,None],np.asarray(b)[...,None]
        return (image[y%h,x%w]*(1-a)+image[y%h,(x+1)%w]*a)*(1-b)+(image[(y+1)%h,x%w]*(1-a)+image[(y+1)%h,(x+1)%w]*a)*b

    def evaluate(self,context,parameters):
        @lru_cache
        def node(path):
            obj=self.objects[path]
            kind=obj['class_path'].rsplit('.',1)[-1].removeprefix('MaterialExpression')
            props={p['name']:p['exported_text'] for p in obj['properties']}
            def number(name,default=0):return float(props.get(name,default))
            def arg(name,default=0):
                data=parse_struct(props.get(name,'()'))
                if data.get('Expression','None')=='None':return np.asarray(default)
                result=node(source_object_path(data['Expression']))
                if kind!='TextureSample' and self.objects[source_object_path(data['Expression'])]['class_path'].endswith('TextureSample'):
                    index=int(data.get('OutputIndex',0))
                    result=result[...,:3] if index==0 else result if index==5 else result[...,index-1:index]
                if data.get('Mask','0') not in ['False','0'] and np.ndim(result)>0:
                    mask=[i for i,c in enumerate('RGBA') if data.get('Mask'+c,'0') not in ['False','0'] and i<result.shape[-1]]
                    result=result[...,mask]
                return result
            if kind=='ScalarParameter':return np.asarray(parameters.get(props['ParameterName'],number('DefaultValue')))
            if kind=='VectorParameter':return np.asarray(parameters.get(props['ParameterName'],[float(parse_struct(props['DefaultValue'])[c]) for c in 'RGB']))
            if kind=='Constant':return np.asarray(number('R'))
            if kind=='Constant3Vector':return np.array([float(parse_struct(props['Constant'])[c]) for c in 'RGB'])
            if kind=='CameraVectorWS':return context['camera_vector']
            if kind=='ObjectPositionWS':return context['origin']
            if kind=='WorldPosition':return context['position']
            if kind=='ObjectRadius':return context['radius']
            if kind=='Time':return context['time']
            if kind=='TextureCoordinate':return context['uv'+str(int(number('CoordinateIndex')))]*[number('UTiling',1),number('VTiling',1)]
            if kind=='Panner':return arg('Coordinate',context['uv'+str(int(number('ConstCoordinate')))])+arg('Time',context['time'])*np.array([number('SpeedX'),number('SpeedY')])
            if kind=='TextureSample':return self.sample(source_object_path(props['Texture']),arg('Coordinates',context['uv0']))
            if kind in ['Add','Subtract','Multiply','Divide']:
                a,b=arg('A',number('ConstA')),arg('B',number('ConstB',1))
                return {'Add':np.add,'Subtract':np.subtract,'Multiply':np.multiply,'Divide':np.divide}[kind](a,b)
            if kind=='LinearInterpolate':
                a,b,t=arg('A',number('ConstA')),arg('B',number('ConstB',1)),arg('Alpha',number('ConstAlpha',.5))
                return a*(1-t)+b*t
            if kind=='Power':
                value=arg('Base')
                return np.where(value<=2.980233e-8,0,np.maximum(value,0)**arg('Exponent',number('ConstExponent',2)))
            if kind=='Clamp':return np.clip(arg('Input'),arg('Min',number('MinDefault')),arg('Max',number('MaxDefault',1)))
            if kind=='OneMinus':return 1-arg('Input')
            if kind=='DotProduct':return np.sum(arg('A')*arg('B'),axis=-1,keepdims=True)
            if kind=='Normalize':
                value=arg('VectorInput');return value/np.linalg.norm(value,axis=-1,keepdims=True)
            if kind=='ComponentMask':return arg('Input')[...,[i for i,c in enumerate('RGBA') if props.get(c,'False')=='True']]
            if kind=='SphereMask':return np.clip((1-np.linalg.norm(arg('A')-arg('B'),axis=-1,keepdims=True)/max(.00001,arg('Radius',number('AttenuationRadius'))))/max(.00001,1-arg('Hardness',number('HardnessPercent')*.01)),0,1)
            raise ValueError('Unsupported source sky expression: '+kind)
        return node(self.output)


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--source',type=Path,required=True)
    parser.add_argument('--definition',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--node',default='node')
    parser.add_argument('--pngjs-module',default='pngjs')
    parser.add_argument('--sun-pitch',type=float)
    args=parser.parse_args()
    source=json.loads((args.source/'M_Sky_Panning_Clouds2.ue.json').read_text(encoding='utf-8-sig'))
    material=SourceMaterial(source,args.source,args.node,args.pngjs_module)
    definition=json.loads(args.definition.read_text(encoding='utf-8'))
    if args.sun_pitch is not None:definition['sun']['pitch']=args.sun_pitch
    params=refresh(definition)
    radius=1638400.0
    uv0=np.array([.31,.47])
    uv1=np.array([.67,.29])
    if params['Sun height']>0:
        texture=material.pixels('/Engine/EngineSky/T_Sky_Stars.T_Sky_Stars')
        y,x=np.unravel_index(texture[:,:,:3].max(axis=2).argmax(),texture.shape[:2])
        uv1=np.array([(x+.5)/texture.shape[1],(y+.5)/texture.shape[0]])/12
    rows=[]
    for pitch in [-90,-45,0,45,90]:
        for time in [0,1,60,3601]:
            angle=np.radians(pitch)
            position=np.array([np.cos(angle),0,np.sin(angle)])*radius
            context=dict(camera_vector=-position/radius,origin=np.zeros(3),position=position,radius=radius,time=time,uv0=uv0,uv1=uv1)
            rows.append(dict(pitch=pitch,time=time,color=material.evaluate(context,params).tolist()))
    args.output.write_text(json.dumps(dict(success=True,sun_pitch=definition['sun']['pitch'],uv0=uv0.tolist(),uv1=uv1.tolist(),samples=rows),indent=2)+'\n',encoding='utf-8')
    print('Source sky output samples:',len(rows))


if __name__=='__main__':main()
