import argparse
import json
import sys
from copy import deepcopy
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'animgraph_authoring'))
from coordinate_basis import source_point_to_runtime, source_direction_to_runtime, source_rotator_to_runtime_rotation
from import_moving_actors import vector, transform, compose
from ue_animation_asset_resolver import object_property, source_object_path, rich_curve_key
from ue_exported_text import parse_bool, parse_number, parse_struct, parse_list


def color_curves(source):
    properties = {'properties': source['asset_properties']}
    for name, expected in [('AdjustBrightness',1), ('AdjustBrightnessCurve',1), ('AdjustVibrance',0), ('AdjustSaturation',1), ('AdjustHue',0), ('AdjustMinAlpha',0), ('AdjustMaxAlpha',1)]:
        if parse_number(object_property(properties,name)) != expected:
            raise ValueError('Source sky color adjustment requires an explicit port: '+name)
    channels = []
    for row in source['asset_properties']:
        if row['name'] != 'FloatCurves':
            continue
        curve = parse_struct(row['exported_text'])
        if any(curve.get(name,'RCCE_Constant') != 'RCCE_Constant' for name in ['PreInfinityExtrap','PostInfinityExtrap']):
            raise ValueError('Source sky curve requires constant extrapolation')
        channels.append([rich_curve_key(key) for key in parse_list(curve.get('Keys','()'))])
    if len(channels) != 4:
        raise ValueError('Source sky requires four color channels')
    return channels[:3]


def compile_definition(world, blueprint, material, curves):
    objects = {item['object_path']:item for item in world['asset_objects']}
    actors = [item for item in objects.values() if item['class_path'] == blueprint['asset_path']+'_C']
    if len(actors) != 1:
        raise ValueError('Source world requires one sky actor')
    actor = actors[0]
    def component(owner, name):
        ref=object_property(owner,name)
        return objects[source_object_path(ref) if "'" in ref else owner['object_path']+'.'+ref]
    root, mesh = component(actor,'RootComponent'), component(actor,'SkySphereMesh')
    if object_property(root,'AttachParent')!='None' or object_property(mesh,'AttachParent')!=root['object_path'].rsplit('.',1)[-1]:
        raise ValueError('Source sky attachment changed')
    frame=compose(transform(root),transform(mesh))
    source_light=object_property(actor,'Directional light actor')
    light=None
    if source_light!='None':
        light_actor=objects[source_object_path(source_light)]
        light_root=component(light_actor,'RootComponent')
        light_component=component(light_actor,'LightComponent')
        rotation=vector(object_property(light_root,'RelativeRotation'),['Pitch','Yaw','Roll'])
        light=dict(pitch=rotation[0],yaw=rotation[1],color=vector(object_property(light_component,'LightColor'),'RGBA'))
        light['color']=[value/255 if index==3 else value/3294.6 if value<=10.31475 else ((value/255+.055)/1.055)**2.4 for index,value in enumerate(light['color'])]
    defaults={}
    expressions={}
    for obj in material['asset_objects']:
        expressions[obj['object_path'].rsplit(':',1)[-1]]=obj
        if obj['class_path'].endswith('MaterialExpressionScalarParameter'):
            defaults[object_property(obj,'ParameterName')]=parse_number(object_property(obj,'DefaultValue'))
        elif obj['class_path'].endswith('MaterialExpressionVectorParameter'):
            defaults[object_property(obj,'ParameterName')]=vector(object_property(obj,'DefaultValue'),'RGBA')
    result=dict(colors_by_sun=parse_bool(object_property(actor,'Colors determined by sun position')),
        sun_height=parse_number(object_property(actor,'Sun height')),
        actor_yaw=vector(object_property(root,'RelativeRotation'),['Pitch','Yaw','Roll'])[1],
        sun=light, transform=frame, mesh_source=source_object_path(object_property(mesh,'StaticMesh')),
        zenith=vector(object_property(actor,'Zenith Color'),'RGBA'),
        horizon=vector(object_property(actor,'Horizon color'),'RGBA'),
        cloud=vector(object_property(actor,'Cloud color'),'RGBA'),
        overall=vector(object_property(actor,'Overall Color'),'RGBA'),
        sun_brightness=parse_number(object_property(actor,'Sun brightness')),
        horizon_falloff=parse_number(object_property(actor,'Horizon Falloff')),
        cloud_speed=parse_number(object_property(actor,'Cloud speed')),
        cloud_opacity=parse_number(object_property(actor,'Cloud opacity')),
        stars_brightness=parse_number(object_property(actor,'Stars brightness')),
        material_constants=dict(noise_power_1=defaults['NoisePower1'],noise_power_2=defaults['NoisePower2'],sun_radius=defaults['Sun Radius']), curves={})
    for field,node,prop in [('blue_speed','Panner_2','SpeedX'),('cloud_speed','Panner_4','SpeedX'),
        ('noise_tiling','TextureCoordinate_1','UTiling'),('stars_tiling','TextureCoordinate_5','UTiling'),
        ('emissive_scale','Multiply_1','ConstB'),('rim_scale','Multiply_11','ConstB'),
        ('height_radius_scale','Multiply_3','ConstB'),('glow_radius','Constant_13','R'),('glow_exponent','Power_8','ConstExponent')]:
        result['material_constants'][field]=parse_number(object_property(expressions['MaterialExpression'+node],prop))
    for name,source in zip(['zenith','horizon','cloud'],curves):
        field={'zenith':'Zenith','horizon':'Horizon','cloud':'Cloud'}[name]+' color curve'
        if source_object_path(object_property(actor,field))!=source['asset_path']:
            raise ValueError('Source sky curve reference differs')
        result['curves'][name]=color_curves(source)
    return result


def compile_geometry(source):
    geometry=source.get('static_mesh_geometry')
    if not geometry or not geometry['vertices_source'] or len(geometry['uv_channels'])<2:
        raise ValueError('Source sky render geometry and both UV channels are required')
    return dict(positions=[source_point_to_runtime([v[axis] for axis in 'xyz']) for v in geometry['vertices_source']],
        normals=[source_direction_to_runtime([v[axis] for axis in 'xyz']) for v in geometry['normals_source']],
        uv0=geometry['uv_channels'][0],uv1=geometry['uv_channels'][1],
        indices=[value for triangle in geometry['triangle_indices'] for value in triangle],
        bounds_origin=source_point_to_runtime([geometry['bounds_origin_source'][axis] for axis in 'xyz']),
        bounds_radius=geometry['bounds_radius_source']*.01)


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--source',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--self-test',action='store_true')
    parser.add_argument('--check',action='store_true')
    args=parser.parse_args()
    def load(name):return json.loads((args.source/(name+'.ue.json')).read_text(encoding='utf-8-sig'))
    sources=[load(name) for name in ['L_Als_Playground','BP_Sky_Sphere','M_Sky_Panning_Clouds2']]
    curves=[load('C_Sky_'+name+'_Color') for name in ['Zenith','Horizon','Cloud']]
    result=compile_definition(*sources,curves)
    geometry=compile_geometry(load(result['mesh_source'].rsplit('.',1)[-1]))
    if args.self_test:
        changed=deepcopy(sources)
        actor=next(o for o in changed[0]['asset_objects'] if o['class_path']==changed[1]['asset_path']+'_C')
        next(p for p in actor['properties'] if p['name']=='Cloud speed')['exported_text']='3.5'
        assert compile_definition(*changed,curves)['cloud_speed']==3.5
        changed_curve=deepcopy(curves)
        next(p for p in changed_curve[0]['asset_properties'] if p['name']=='AdjustBrightness')['exported_text']='2.0'
        try:compile_definition(*sources,changed_curve)
        except ValueError:pass
        else:raise AssertionError('Unsupported color adjustment was accepted')
        try:compile_geometry({})
        except ValueError:pass
        else:raise AssertionError('Missing source UV geometry was accepted')
    args.output.mkdir(parents=True,exist_ok=True)
    for name,data in [('playground_sky.runtime.json',result),('sky_mesh.runtime.json',geometry)]:
        target=args.output/name
        text=json.dumps(data,indent=2)+'\n'
        if args.check:
            if target.read_text(encoding='utf-8')!=text:raise ValueError('Generated sky data differs: '+name)
        else:target.write_text(text,encoding='utf-8')
    for name in ['T_Sky_Clouds_M','T_Sky_Blue','T_Sky_Stars']:
        texture=load(name)
        from PIL import Image
        source=args.source/texture['source_image']
        image=Image.open(source)
        expected='PNG' if texture['source_pixel_format']=='RGBA16' else {4:'TGA',3:'PNG'}[texture['source_channels']]
        if not texture['source_image_pixel_exact'] or len(image.getbands())!=texture['source_channels'] or image.format!=expected:
            raise ValueError('Source sky texture channels or pixel evidence differ')
        if texture['source_pixel_format']=='RGBA16' and source.read_bytes()[24:26]!=bytes([16,6]):
            raise ValueError('Source RGBA16 PNG lost its bit depth or alpha')
        target=args.output/source.name.replace('.ue','')
        if args.check:
            if target.read_bytes()!=source.read_bytes():raise ValueError('Runtime sky texture differs')
        else:target.write_bytes(source.read_bytes())
        assert Image.open(target).tobytes()==image.tobytes()
    print(json.dumps(dict(success=True,vertices=len(geometry['positions']),uv_channels=2,cloud_speed=result['cloud_speed'],self_test=args.self_test,check=args.check)))


if __name__=='__main__':
    main()
