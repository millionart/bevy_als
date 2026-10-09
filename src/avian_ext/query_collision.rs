use avian3d::prelude::Collider;
use bevy::prelude::Component;

#[derive(Component, Clone)]
pub struct ComplexCollider(pub Collider);
