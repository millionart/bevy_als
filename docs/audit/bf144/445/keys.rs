fn c445_updated_keys(caches:&bevy_animation_graph::core::context::node_caches::NodeCaches)->Vec<(bevy_animation_graph::core::animation_graph::NodeId,bevy_animation_graph::core::context::node_states::StateKey)>{
    use bevy::reflect::{PartialReflect,ReflectRef};
    use bevy_animation_graph::core::animation_graph::NodeId;
    use bevy_animation_graph::core::context::node_caches::NodeCache;
    let ReflectRef::Struct(fields)=caches.reflect_ref()else{panic!("Node caches are not a reflected struct");};
    let ReflectRef::Map(map)=fields.field("caches").unwrap().reflect_ref()else{panic!("Node cache entries are not a reflected map");};
    let mut output=Vec::new();
    for (node,value) in map.iter(){
        let node=*node.try_downcast_ref::<NodeId>().unwrap();
        let cache=value.try_downcast_ref::<std::sync::Arc<NodeCache>>().unwrap();
        output.extend(cache.updated.iter().map(|key|(node,*key)));
    }
    output.sort_by_key(|(node,key)|format!("{node:?}:{key:?}"));
    output
}
