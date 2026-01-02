pub mod character;
pub mod collision;
pub mod joints;
pub mod queries;
pub mod rigidbody;

pub use character::*;
pub use collision::*;
pub use joints::*;
pub use queries::*;
pub use rigidbody::*;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

static PHYSICS_HANDLE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicsHandle(u64);

impl PhysicsHandle {
    pub fn new() -> Self {
        Self(PHYSICS_HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    pub fn id(&self) -> u64 {
        self.0
    }
}

impl Default for PhysicsHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum ColliderShape {
    Box { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
    Cylinder { half_height: f32, radius: f32 },
    Cone { half_height: f32, radius: f32 },
    Mesh { vertices: Vec<Vec3>, indices: Vec<[u32; 3]> },
    ConvexHull { points: Vec<Vec3> },
    Compound { shapes: Vec<(Vec3, Quat, ColliderShape)> },
    HeightField { heights: Vec<Vec<f32>>, scale: Vec3 },
}

impl ColliderShape {
    pub fn cuboid(half_x: f32, half_y: f32, half_z: f32) -> Self {
        Self::Box {
            half_extents: Vec3::new(half_x, half_y, half_z),
        }
    }

    pub fn sphere(radius: f32) -> Self {
        Self::Sphere { radius }
    }

    pub fn capsule(half_height: f32, radius: f32) -> Self {
        Self::Capsule { half_height, radius }
    }

    pub fn capsule_y(total_height: f32, radius: f32) -> Self {
        let half_height = (total_height - 2.0 * radius).max(0.0) / 2.0;
        Self::Capsule { half_height, radius }
    }

    pub fn cylinder(half_height: f32, radius: f32) -> Self {
        Self::Cylinder { half_height, radius }
    }

    pub fn to_rapier_collider(&self) -> Collider {
        match self {
            ColliderShape::Box { half_extents } => {
                Collider::cuboid(half_extents.x, half_extents.y, half_extents.z)
            }
            ColliderShape::Sphere { radius } => Collider::ball(*radius),
            ColliderShape::Capsule { half_height, radius } => {
                Collider::capsule_y(*half_height, *radius)
            }
            ColliderShape::Cylinder { half_height, radius } => {
                Collider::cylinder(*half_height, *radius)
            }
            ColliderShape::Cone { half_height, radius } => {
                Collider::cone(*half_height, *radius)
            }
            ColliderShape::ConvexHull { points } => {
                Collider::convex_hull(points).unwrap_or_else(|| Collider::ball(0.5))
            }
            ColliderShape::Mesh { vertices, indices } => {
                Collider::trimesh(vertices.clone(), indices.clone())
            }
            ColliderShape::Compound { shapes } => {
                let rapier_shapes: Vec<(Vec3, Quat, Collider)> = shapes
                    .iter()
                    .map(|(pos, rot, shape)| (*pos, *rot, shape.to_rapier_collider()))
                    .collect();
                Collider::compound(rapier_shapes)
            }
            ColliderShape::HeightField { heights, scale } => {
                let rows = heights.len();
                let cols = if rows > 0 { heights[0].len() } else { 0 };
                let flat_heights: Vec<f32> = heights.iter().flatten().copied().collect();
                Collider::heightfield(
                    flat_heights,
                    rows,
                    cols,
                    Vec3::new(scale.x, 1.0, scale.z),
                )
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RigidBodyType {
    #[default]
    Static,
    Dynamic,
    KinematicPositionBased,
    KinematicVelocityBased,
}

impl RigidBodyType {
    pub fn to_rapier(&self) -> RigidBody {
        match self {
            RigidBodyType::Static => RigidBody::Fixed,
            RigidBodyType::Dynamic => RigidBody::Dynamic,
            RigidBodyType::KinematicPositionBased => RigidBody::KinematicPositionBased,
            RigidBodyType::KinematicVelocityBased => RigidBody::KinematicVelocityBased,
        }
    }

    pub fn is_static(&self) -> bool {
        matches!(self, RigidBodyType::Static)
    }

    pub fn is_dynamic(&self) -> bool {
        matches!(self, RigidBodyType::Dynamic)
    }

    pub fn is_kinematic(&self) -> bool {
        matches!(
            self,
            RigidBodyType::KinematicPositionBased | RigidBodyType::KinematicVelocityBased
        )
    }
}

#[derive(Debug, Clone)]
pub struct ColliderConfig {
    pub shape: ColliderShape,
    pub offset: Vec3,
    pub rotation: Quat,
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
    pub is_sensor: bool,
    pub collision_groups: CollisionGroups,
    pub solver_groups: SolverGroups,
    pub active_events: ActiveEvents,
    pub active_collision_types: ActiveCollisionTypes,
}

impl Default for ColliderConfig {
    fn default() -> Self {
        Self {
            shape: ColliderShape::Sphere { radius: 0.5 },
            offset: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            friction: 0.5,
            restitution: 0.0,
            density: 1.0,
            is_sensor: false,
            collision_groups: CollisionGroups::default(),
            solver_groups: SolverGroups::default(),
            active_events: ActiveEvents::COLLISION_EVENTS,
            active_collision_types: ActiveCollisionTypes::default(),
        }
    }
}

impl ColliderConfig {
    pub fn sensor(shape: ColliderShape) -> Self {
        Self {
            shape,
            is_sensor: true,
            active_events: ActiveEvents::COLLISION_EVENTS,
            ..Default::default()
        }
    }

    pub fn with_friction(mut self, friction: f32) -> Self {
        self.friction = friction;
        self
    }

    pub fn with_restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution;
        self
    }

    pub fn with_density(mut self, density: f32) -> Self {
        self.density = density;
        self
    }

    pub fn with_offset(mut self, offset: Vec3) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_collision_groups(mut self, groups: CollisionGroups) -> Self {
        self.collision_groups = groups;
        self
    }
}

#[derive(Debug, Clone)]
pub struct RigidBodyConfig {
    pub body_type: RigidBodyType,
    pub position: Vec3,
    pub rotation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub can_sleep: bool,
    pub ccd_enabled: bool,
    pub dominance: i8,
    pub additional_mass: f32,
    pub locked_axes: LockedAxes,
}

impl Default for RigidBodyConfig {
    fn default() -> Self {
        Self {
            body_type: RigidBodyType::Dynamic,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.05,
            can_sleep: true,
            ccd_enabled: false,
            dominance: 0,
            additional_mass: 0.0,
            locked_axes: LockedAxes::empty(),
        }
    }
}

impl RigidBodyConfig {
    pub fn dynamic() -> Self {
        Self {
            body_type: RigidBodyType::Dynamic,
            ..Default::default()
        }
    }

    pub fn kinematic() -> Self {
        Self {
            body_type: RigidBodyType::KinematicPositionBased,
            ..Default::default()
        }
    }

    pub fn fixed() -> Self {
        Self {
            body_type: RigidBodyType::Static,
            ..Default::default()
        }
    }

    pub fn with_position(mut self, position: Vec3) -> Self {
        self.position = position;
        self
    }

    pub fn with_gravity_scale(mut self, scale: f32) -> Self {
        self.gravity_scale = scale;
        self
    }

    pub fn with_damping(mut self, linear: f32, angular: f32) -> Self {
        self.linear_damping = linear;
        self.angular_damping = angular;
        self
    }

    pub fn with_ccd(mut self, enabled: bool) -> Self {
        self.ccd_enabled = enabled;
        self
    }

    pub fn with_locked_axes(mut self, axes: LockedAxes) -> Self {
        self.locked_axes = axes;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PhysicsSettings {
    pub gravity: Vec3,
    pub timestep_mode: TimestepMode,
    pub max_velocity_iterations: usize,
    pub max_velocity_friction_iterations: usize,
    pub max_stabilization_iterations: usize,
    pub prediction_distance: f32,
    pub length_unit: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum TimestepMode {
    #[default]
    Fixed,
    Variable,
    Interpolated,
}

impl Default for PhysicsSettings {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            timestep_mode: TimestepMode::Fixed,
            max_velocity_iterations: 4,
            max_velocity_friction_iterations: 8,
            max_stabilization_iterations: 1,
            prediction_distance: 0.002,
            length_unit: 1.0,
        }
    }
}

impl PhysicsSettings {
    pub fn mmorpg_defaults() -> Self {
        Self {
            gravity: Vec3::new(0.0, -20.0, 0.0),
            timestep_mode: TimestepMode::Fixed,
            max_velocity_iterations: 8,
            max_velocity_friction_iterations: 8,
            max_stabilization_iterations: 2,
            prediction_distance: 0.002,
            length_unit: 1.0,
        }
    }
}

#[derive(Resource)]
pub struct PhysicsFabric {
    pub collision_manager: CollisionManager,
    pub query_pipeline: PhysicsQueryPipeline,
    pub settings: PhysicsSettings,
    tracked_bodies: HashMap<PhysicsHandle, Entity>,
    tracked_colliders: HashMap<PhysicsHandle, Entity>,
    enabled: bool,
    paused: bool,
    time_scale: f32,
    step_count: u64,
}

impl Default for PhysicsFabric {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsFabric {
    pub fn new() -> Self {
        Self {
            collision_manager: CollisionManager::new(),
            query_pipeline: PhysicsQueryPipeline::new(),
            settings: PhysicsSettings::default(),
            tracked_bodies: HashMap::new(),
            tracked_colliders: HashMap::new(),
            enabled: true,
            paused: false,
            time_scale: 1.0,
            step_count: 0,
        }
    }

    pub fn with_settings(settings: PhysicsSettings) -> Self {
        Self {
            settings,
            ..Self::new()
        }
    }

    pub fn create_rigidbody(
        &mut self,
        commands: &mut Commands,
        config: RigidBodyConfig,
    ) -> PhysicsHandle {
        let handle = PhysicsHandle::new();

        let entity = commands
            .spawn((
                config.body_type.to_rapier(),
                Transform::from_translation(config.position).with_rotation(config.rotation),
                Velocity {
                    linvel: config.linear_velocity,
                    angvel: config.angular_velocity,
                },
                GravityScale(config.gravity_scale),
                Damping {
                    linear_damping: config.linear_damping,
                    angular_damping: config.angular_damping,
                },
                config.locked_axes,
                Sleeping::default(),
                ExternalForce::default(),
                ExternalImpulse::default(),
                PhysicsBody { handle },
            ))
            .id();

        if config.ccd_enabled {
            commands.entity(entity).insert(Ccd::enabled());
        }

        if config.dominance != 0 {
            commands.entity(entity).insert(Dominance::group(config.dominance));
        }

        if config.additional_mass > 0.0 {
            commands.entity(entity).insert(AdditionalMassProperties::Mass(config.additional_mass));
        }

        self.tracked_bodies.insert(handle, entity);
        log::debug!("PhysicsFabric: Created rigidbody {:?} -> entity {:?}", handle, entity);

        handle
    }

    pub fn create_collider(
        &mut self,
        commands: &mut Commands,
        parent: Option<Entity>,
        config: ColliderConfig,
    ) -> PhysicsHandle {
        let handle = PhysicsHandle::new();

        let collider = config.shape.to_rapier_collider();
        
        let mut entity_commands = if let Some(parent_entity) = parent {
            commands.entity(parent_entity)
        } else {
            commands.spawn(Transform::default())
        };

        let collider_entity = if parent.is_some() {
            entity_commands.with_children(|parent| {
                parent.spawn((
                    collider,
                    Transform::from_translation(config.offset).with_rotation(config.rotation),
                    Friction::coefficient(config.friction),
                    Restitution::coefficient(config.restitution),
                    ColliderMassProperties::Density(config.density),
                    config.collision_groups,
                    config.solver_groups,
                    config.active_events,
                    config.active_collision_types,
                    PhysicsCollider { handle },
                ));
            });
            entity_commands.id()
        } else {
            let entity = commands
                .spawn((
                    collider,
                    Transform::from_translation(config.offset).with_rotation(config.rotation),
                    Friction::coefficient(config.friction),
                    Restitution::coefficient(config.restitution),
                    ColliderMassProperties::Density(config.density),
                    config.collision_groups,
                    config.solver_groups,
                    config.active_events,
                    config.active_collision_types,
                    PhysicsCollider { handle },
                ))
                .id();
            entity
        };

        if config.is_sensor {
            commands.entity(collider_entity).insert(Sensor);
        }

        self.tracked_colliders.insert(handle, collider_entity);
        log::debug!("PhysicsFabric: Created collider {:?}", handle);

        handle
    }

    pub fn create_static_collider(
        &mut self,
        commands: &mut Commands,
        position: Vec3,
        shape: ColliderShape,
    ) -> PhysicsHandle {
        self.create_collider(
            commands,
            None,
            ColliderConfig {
                shape,
                offset: position,
                ..Default::default()
            },
        )
    }

    pub fn create_trigger_volume(
        &mut self,
        commands: &mut Commands,
        position: Vec3,
        shape: ColliderShape,
    ) -> PhysicsHandle {
        self.create_collider(
            commands,
            None,
            ColliderConfig::sensor(shape).with_offset(position),
        )
    }

    pub fn remove_rigidbody(&mut self, commands: &mut Commands, handle: PhysicsHandle) {
        if let Some(entity) = self.tracked_bodies.remove(&handle) {
            commands.entity(entity).despawn_recursive();
            log::debug!("PhysicsFabric: Removed rigidbody {:?}", handle);
        }
    }

    pub fn remove_collider(&mut self, commands: &mut Commands, handle: PhysicsHandle) {
        if let Some(entity) = self.tracked_colliders.remove(&handle) {
            commands.entity(entity).despawn_recursive();
            log::debug!("PhysicsFabric: Removed collider {:?}", handle);
        }
    }

    pub fn get_rigidbody_entity(&self, handle: PhysicsHandle) -> Option<Entity> {
        self.tracked_bodies.get(&handle).copied()
    }

    pub fn get_collider_entity(&self, handle: PhysicsHandle) -> Option<Entity> {
        self.tracked_colliders.get(&handle).copied()
    }

    pub fn raycast(
        &self,
        rapier_context: &RapierContext,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        filter: QueryFilter,
    ) -> Option<RaycastResult> {
        self.query_pipeline.raycast(rapier_context, origin, direction, max_distance, filter)
    }

    pub fn raycast_all(
        &self,
        rapier_context: &RapierContext,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        filter: QueryFilter,
    ) -> Vec<RaycastResult> {
        self.query_pipeline.raycast_all(rapier_context, origin, direction, max_distance, filter)
    }

    pub fn spherecast(
        &self,
        rapier_context: &RapierContext,
        origin: Vec3,
        direction: Vec3,
        radius: f32,
        max_distance: f32,
        filter: QueryFilter,
    ) -> Option<ShapecastResult> {
        self.query_pipeline.spherecast(rapier_context, origin, direction, radius, max_distance, filter)
    }

    pub fn overlap_sphere(
        &self,
        rapier_context: &RapierContext,
        center: Vec3,
        radius: f32,
        filter: QueryFilter,
    ) -> Vec<Entity> {
        self.query_pipeline.overlap_sphere(rapier_context, center, radius, filter)
    }

    pub fn overlap_box(
        &self,
        rapier_context: &RapierContext,
        center: Vec3,
        half_extents: Vec3,
        rotation: Quat,
        filter: QueryFilter,
    ) -> Vec<Entity> {
        self.query_pipeline.overlap_box(rapier_context, center, half_extents, rotation, filter)
    }

    pub fn set_gravity(&mut self, gravity: Vec3) {
        self.settings.gravity = gravity;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn set_time_scale(&mut self, scale: f32) {
        self.time_scale = scale.clamp(0.0, 4.0);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn time_scale(&self) -> f32 {
        self.time_scale
    }

    pub fn step_count(&self) -> u64 {
        self.step_count
    }

    pub fn rigidbody_count(&self) -> usize {
        self.tracked_bodies.len()
    }

    pub fn collider_count(&self) -> usize {
        self.tracked_colliders.len()
    }

    pub(crate) fn update(&mut self, _delta_time: f32) {
        if !self.enabled || self.paused {
            return;
        }
        self.step_count += 1;
    }
}

#[derive(Component)]
pub struct PhysicsBody {
    pub handle: PhysicsHandle,
}

#[derive(Component)]
pub struct PhysicsCollider {
    pub handle: PhysicsHandle,
}

#[derive(Event, Debug, Clone)]
pub enum PhysicsEvent {
    CollisionStarted {
        entity_a: Entity,
        entity_b: Entity,
        contact_point: Vec3,
        normal: Vec3,
    },
    CollisionEnded {
        entity_a: Entity,
        entity_b: Entity,
    },
    TriggerEntered {
        trigger: Entity,
        other: Entity,
    },
    TriggerExited {
        trigger: Entity,
        other: Entity,
    },
    BodySleep {
        entity: Entity,
    },
    BodyWake {
        entity: Entity,
    },
}

pub struct PhysicsPlugin {
    pub settings: PhysicsSettings,
}

impl Default for PhysicsPlugin {
    fn default() -> Self {
        Self {
            settings: PhysicsSettings::default(),
        }
    }
}

impl PhysicsPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_settings(settings: PhysicsSettings) -> Self {
        Self { settings }
    }

    pub fn mmorpg() -> Self {
        Self {
            settings: PhysicsSettings::mmorpg_defaults(),
        }
    }
}

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        let rapier_config = RapierConfiguration {
            gravity: self.settings.gravity,
            physics_pipeline_active: true,
            query_pipeline_active: true,
            timestep_mode: bevy_rapier3d::plugin::TimestepMode::Fixed {
                dt: 1.0 / 60.0,
                substeps: 1,
            },
            scaled_shape_subdivision: 10,
            force_update_from_transform_changes: false,
        };

        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
            .insert_resource(rapier_config)
            .insert_resource(PhysicsFabric::with_settings(self.settings))
            .add_event::<PhysicsEvent>()
            .add_systems(
                Update,
                (
                    update_physics_fabric,
                    process_collision_events,
                    sync_collision_manager,
                )
                    .chain(),
            )
            .add_systems(PostUpdate, update_character_controllers);

        log::info!(
            "PhysicsPlugin initialized with gravity {:?}",
            self.settings.gravity
        );
    }
}

fn update_physics_fabric(mut physics: ResMut<PhysicsFabric>, time: Res<Time>) {
    physics.update(time.delta_secs());
}

fn process_collision_events(
    mut collision_events: EventReader<CollisionEvent>,
    mut physics_events: EventWriter<PhysicsEvent>,
    rapier_context: ReadRapierContext,
) {
    let Ok(rapier_context) = rapier_context.single() else {
        return;
    };
    
    for event in collision_events.read() {
        match event {
            CollisionEvent::Started(entity_a, entity_b, flags) => {
                let is_sensor = flags.is_empty();
                
                if is_sensor {
                    physics_events.send(PhysicsEvent::TriggerEntered {
                        trigger: *entity_a,
                        other: *entity_b,
                    });
                } else {
                    let contact_point = rapier_context
                        .contact_pair(*entity_a, *entity_b)
                        .and_then(|pair| {
                            pair.manifolds().next().and_then(|manifold| {
                                manifold.points().next().map(|point| {
                                    Vec3::new(
                                        point.local_p1().x,
                                        point.local_p1().y,
                                        point.local_p1().z,
                                    )
                                })
                            })
                        })
                        .unwrap_or(Vec3::ZERO);

                    physics_events.send(PhysicsEvent::CollisionStarted {
                        entity_a: *entity_a,
                        entity_b: *entity_b,
                        contact_point,
                        normal: Vec3::Y,
                    });
                }
            }
            CollisionEvent::Stopped(entity_a, entity_b, flags) => {
                let is_sensor = flags.is_empty();
                
                if is_sensor {
                    physics_events.send(PhysicsEvent::TriggerExited {
                        trigger: *entity_a,
                        other: *entity_b,
                    });
                } else {
                    physics_events.send(PhysicsEvent::CollisionEnded {
                        entity_a: *entity_a,
                        entity_b: *entity_b,
                    });
                }
            }
        }
    }
}

fn sync_collision_manager(
    mut physics: ResMut<PhysicsFabric>,
    mut physics_events: EventReader<PhysicsEvent>,
) {
    for event in physics_events.read() {
        physics.collision_manager.handle_event(event);
    }
}

fn update_character_controllers(
    time: Res<Time>,
    physics: Res<PhysicsFabric>,
    rapier_context: ReadRapierContext,
    mut controllers: Query<(
        Entity,
        &mut CharacterController,
        &mut KinematicCharacterController,
        &Transform,
    )>,
    mut outputs: Query<&KinematicCharacterControllerOutput>,
) {
    if !physics.is_enabled() || physics.is_paused() {
        return;
    }
    
    let Ok(rapier_context) = rapier_context.single() else {
        return;
    };

    let dt = time.delta_secs() * physics.time_scale();

    for (entity, mut controller, mut kinematic, transform) in controllers.iter_mut() {
        if let Ok(output) = outputs.get(entity) {
            controller.update_ground_state(output, &rapier_context, transform.translation);
        }

        let movement = controller.compute_movement(dt, transform);
        kinematic.translation = Some(movement);
    }
}
