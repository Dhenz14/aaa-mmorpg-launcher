use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use super::{CollisionFilter, PhysicsHandle, LAYER_PLAYER};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundState {
    Grounded,
    Airborne,
    Sliding,
    Climbing,
}

impl Default for GroundState {
    fn default() -> Self {
        Self::Airborne
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GroundInfo {
    pub state: GroundState,
    pub ground_normal: Vec3,
    pub ground_point: Vec3,
    pub ground_entity: Option<Entity>,
    pub slope_angle: f32,
    pub is_on_step: bool,
    pub step_height: f32,
}

impl Default for GroundInfo {
    fn default() -> Self {
        Self {
            state: GroundState::Airborne,
            ground_normal: Vec3::Y,
            ground_point: Vec3::ZERO,
            ground_entity: None,
            slope_angle: 0.0,
            is_on_step: false,
            step_height: 0.0,
        }
    }
}

impl GroundInfo {
    pub fn is_grounded(&self) -> bool {
        matches!(self.state, GroundState::Grounded | GroundState::Climbing)
    }

    pub fn is_on_slope(&self) -> bool {
        self.slope_angle > 5.0
    }

    pub fn is_steep_slope(&self, max_slope: f32) -> bool {
        self.slope_angle > max_slope
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CharacterMovementConfig {
    pub max_speed: f32,
    pub acceleration: f32,
    pub deceleration: f32,
    pub air_control: f32,
    pub jump_height: f32,
    pub jump_count: u32,
    pub max_slope_angle: f32,
    pub step_height: f32,
    pub step_offset: f32,
    pub skin_width: f32,
    pub snap_to_ground: f32,
    pub push_power: f32,
    pub mass: f32,
}

impl Default for CharacterMovementConfig {
    fn default() -> Self {
        Self {
            max_speed: 10.0,
            acceleration: 50.0,
            deceleration: 30.0,
            air_control: 0.3,
            jump_height: 2.0,
            jump_count: 1,
            max_slope_angle: 45.0,
            step_height: 0.35,
            step_offset: 0.1,
            skin_width: 0.01,
            snap_to_ground: 0.3,
            push_power: 2.0,
            mass: 80.0,
        }
    }
}

impl CharacterMovementConfig {
    pub fn mmorpg_player() -> Self {
        Self {
            max_speed: 7.0,
            acceleration: 40.0,
            deceleration: 25.0,
            air_control: 0.2,
            jump_height: 1.5,
            jump_count: 1,
            max_slope_angle: 50.0,
            step_height: 0.4,
            step_offset: 0.1,
            skin_width: 0.02,
            snap_to_ground: 0.4,
            push_power: 1.5,
            mass: 75.0,
        }
    }

    pub fn npc() -> Self {
        Self {
            max_speed: 4.0,
            acceleration: 20.0,
            deceleration: 15.0,
            air_control: 0.0,
            jump_height: 0.0,
            jump_count: 0,
            max_slope_angle: 35.0,
            step_height: 0.3,
            step_offset: 0.1,
            skin_width: 0.02,
            snap_to_ground: 0.3,
            push_power: 0.5,
            mass: 70.0,
        }
    }

    pub fn calculate_jump_velocity(&self, gravity: f32) -> f32 {
        (2.0 * gravity.abs() * self.jump_height).sqrt()
    }
}

#[derive(Component, Debug, Clone)]
pub struct CharacterController {
    pub handle: PhysicsHandle,
    pub config: CharacterMovementConfig,
    pub collision_filter: CollisionFilter,
    
    pub ground_info: GroundInfo,
    pub velocity: Vec3,
    pub input_direction: Vec3,
    pub look_direction: Vec3,
    
    pub jump_count_remaining: u32,
    pub coyote_time: f32,
    pub jump_buffer_time: f32,
    
    pub is_crouching: bool,
    pub is_sprinting: bool,
    pub is_climbing: bool,
    pub is_swimming: bool,
    
    pub external_velocity: Vec3,
    pub platform_velocity: Vec3,
    pub last_ground_position: Vec3,
    
    enabled: bool,
}

impl Default for CharacterController {
    fn default() -> Self {
        Self {
            handle: PhysicsHandle::default(),
            config: CharacterMovementConfig::default(),
            collision_filter: CollisionFilter {
                membership: LAYER_PLAYER,
                mask: u32::MAX & !LAYER_PLAYER,
            },
            ground_info: GroundInfo::default(),
            velocity: Vec3::ZERO,
            input_direction: Vec3::ZERO,
            look_direction: Vec3::NEG_Z,
            jump_count_remaining: 0,
            coyote_time: 0.0,
            jump_buffer_time: 0.0,
            is_crouching: false,
            is_sprinting: false,
            is_climbing: false,
            is_swimming: false,
            external_velocity: Vec3::ZERO,
            platform_velocity: Vec3::ZERO,
            last_ground_position: Vec3::ZERO,
            enabled: true,
        }
    }
}

impl CharacterController {
    pub fn new(config: CharacterMovementConfig) -> Self {
        Self {
            config,
            jump_count_remaining: config.jump_count,
            ..Default::default()
        }
    }

    pub fn player() -> Self {
        let config = CharacterMovementConfig::mmorpg_player();
        Self::new(config)
    }

    pub fn npc() -> Self {
        let config = CharacterMovementConfig::npc();
        Self::new(config)
    }

    pub fn set_input(&mut self, direction: Vec3) {
        self.input_direction = direction.normalize_or_zero();
    }

    pub fn set_look_direction(&mut self, direction: Vec3) {
        let horizontal = Vec3::new(direction.x, 0.0, direction.z);
        if horizontal.length_squared() > 0.001 {
            self.look_direction = horizontal.normalize();
        }
    }

    pub fn jump(&mut self, gravity: f32) {
        if self.can_jump() {
            let jump_vel = self.config.calculate_jump_velocity(gravity);
            self.velocity.y = jump_vel;
            self.jump_count_remaining = self.jump_count_remaining.saturating_sub(1);
            self.ground_info.state = GroundState::Airborne;
            self.coyote_time = 0.0;
            log::debug!("CharacterController: Jump with velocity {}", jump_vel);
        } else {
            self.jump_buffer_time = 0.1;
        }
    }

    pub fn can_jump(&self) -> bool {
        if !self.enabled {
            return false;
        }
        
        if self.is_swimming || self.is_climbing {
            return true;
        }
        
        self.ground_info.is_grounded() 
            || self.coyote_time > 0.0 
            || self.jump_count_remaining > 0
    }

    pub fn set_crouching(&mut self, crouching: bool) {
        self.is_crouching = crouching;
    }

    pub fn set_sprinting(&mut self, sprinting: bool) {
        self.is_sprinting = sprinting && !self.is_crouching;
    }

    pub fn add_external_velocity(&mut self, velocity: Vec3) {
        self.external_velocity += velocity;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn get_current_speed(&self) -> f32 {
        Vec3::new(self.velocity.x, 0.0, self.velocity.z).length()
    }

    pub fn get_effective_max_speed(&self) -> f32 {
        let base_speed = self.config.max_speed;
        if self.is_crouching {
            base_speed * 0.5
        } else if self.is_sprinting {
            base_speed * 1.5
        } else {
            base_speed
        }
    }

    pub fn update_ground_state(
        &mut self,
        output: &KinematicCharacterControllerOutput,
        _rapier_context: &RapierContext,
        current_position: Vec3,
    ) {
        let was_grounded = self.ground_info.is_grounded();
        
        if output.grounded {
            self.ground_info.state = GroundState::Grounded;
            self.ground_info.ground_point = current_position - Vec3::Y * 0.1;
            self.jump_count_remaining = self.config.jump_count;
            self.coyote_time = 0.15;
            self.last_ground_position = current_position;

            if let Some(collision) = output.collisions.first() {
                self.ground_info.ground_normal = collision.hit.normal;
                self.ground_info.slope_angle = collision.hit.normal.dot(Vec3::Y).acos().to_degrees();
                
                if self.ground_info.slope_angle > self.config.max_slope_angle {
                    self.ground_info.state = GroundState::Sliding;
                }
            }

            if self.jump_buffer_time > 0.0 {
                self.jump_buffer_time = 0.0;
            }

            if !was_grounded {
                log::debug!("CharacterController: Landed");
            }
        } else {
            self.ground_info.state = GroundState::Airborne;
            self.ground_info.ground_entity = None;
            
            if was_grounded {
                log::debug!("CharacterController: Left ground");
            }
        }
    }

    pub fn compute_movement(&mut self, dt: f32, transform: &Transform) -> Vec3 {
        if !self.enabled {
            return Vec3::ZERO;
        }

        if self.coyote_time > 0.0 {
            self.coyote_time -= dt;
        }

        if self.jump_buffer_time > 0.0 {
            self.jump_buffer_time -= dt;
        }

        let is_grounded = self.ground_info.is_grounded();
        let control = if is_grounded { 1.0 } else { self.config.air_control };
        let max_speed = self.get_effective_max_speed();

        let target_velocity = self.input_direction * max_speed;
        let current_horizontal = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
        
        let velocity_diff = target_velocity - current_horizontal;
        let accel = if velocity_diff.dot(self.input_direction) > 0.0 {
            self.config.acceleration
        } else {
            self.config.deceleration
        };

        let new_horizontal = current_horizontal + velocity_diff.normalize_or_zero() * accel * control * dt;
        let new_horizontal = if new_horizontal.length() > max_speed {
            new_horizontal.normalize() * max_speed
        } else {
            new_horizontal
        };

        self.velocity.x = new_horizontal.x;
        self.velocity.z = new_horizontal.z;

        if !is_grounded && !self.is_swimming {
            self.velocity.y -= 20.0 * dt;
        }

        if self.ground_info.state == GroundState::Sliding {
            let slide_dir = Vec3::new(
                self.ground_info.ground_normal.x,
                0.0,
                self.ground_info.ground_normal.z,
            ).normalize_or_zero();
            self.velocity += slide_dir * 5.0 * dt;
        }

        let total_velocity = self.velocity + self.external_velocity + self.platform_velocity;
        
        self.external_velocity *= 0.9_f32.powf(dt * 60.0);
        if self.external_velocity.length() < 0.01 {
            self.external_velocity = Vec3::ZERO;
        }

        let movement = total_velocity * dt;

        if is_grounded && self.input_direction.length_squared() < 0.01 {
            let snap = Vec3::new(0.0, -self.config.snap_to_ground, 0.0);
            return movement + snap * dt;
        }

        movement
    }

    pub fn handle_step(
        &mut self,
        rapier_context: &RapierContext,
        position: Vec3,
        movement: Vec3,
        collider_height: f32,
    ) -> Option<Vec3> {
        if !self.ground_info.is_grounded() || movement.length_squared() < 0.001 {
            return None;
        }

        let step_check_pos = position + Vec3::Y * self.config.step_height;
        let horizontal_move = Vec3::new(movement.x, 0.0, movement.z).normalize_or_zero();
        
        let ray_origin = step_check_pos;
        let ray_dir = horizontal_move;
        let max_dist = 0.5;

        if let Some((_, toi)) = rapier_context.cast_ray(
            ray_origin,
            ray_dir,
            max_dist,
            true,
            QueryFilter::default(),
        ) {
            if toi > self.config.step_offset {
                let step_forward = position + horizontal_move * (toi + 0.1);
                let down_ray_origin = step_forward + Vec3::Y * self.config.step_height;
                
                if let Some((_, down_toi)) = rapier_context.cast_ray(
                    down_ray_origin,
                    Vec3::NEG_Y,
                    self.config.step_height * 2.0,
                    true,
                    QueryFilter::default(),
                ) {
                    let step_height = self.config.step_height - down_toi;
                    if step_height > 0.01 && step_height <= self.config.step_height {
                        self.ground_info.is_on_step = true;
                        self.ground_info.step_height = step_height;
                        return Some(Vec3::new(
                            step_forward.x,
                            down_ray_origin.y - down_toi + collider_height * 0.5,
                            step_forward.z,
                        ));
                    }
                }
            }
        }

        self.ground_info.is_on_step = false;
        None
    }

    pub fn push_other(
        &self,
        other_velocity: &mut bevy_rapier3d::prelude::Velocity,
        other_mass: f32,
        contact_normal: Vec3,
    ) {
        let push_force = self.velocity.dot(-contact_normal).max(0.0) * self.config.push_power;
        let mass_ratio = self.config.mass / (self.config.mass + other_mass);
        let impulse = contact_normal * push_force * mass_ratio;
        other_velocity.linvel -= impulse;
    }

    pub fn teleport(&mut self, position: Vec3) {
        self.velocity = Vec3::ZERO;
        self.external_velocity = Vec3::ZERO;
        self.platform_velocity = Vec3::ZERO;
        self.last_ground_position = position;
        self.ground_info = GroundInfo::default();
    }
}

#[derive(Bundle)]
pub struct CharacterControllerBundle {
    pub controller: CharacterController,
    pub kinematic_controller: KinematicCharacterController,
    pub rigidbody: RigidBody,
    pub collider: Collider,
    pub collision_groups: CollisionGroups,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
}

impl CharacterControllerBundle {
    pub fn new(height: f32, radius: f32, position: Vec3) -> Self {
        let capsule_height = (height - 2.0 * radius).max(0.0);
        let controller = CharacterController::player();
        
        Self {
            controller,
            kinematic_controller: KinematicCharacterController {
                offset: CharacterLength::Absolute(0.01),
                slide: true,
                autostep: Some(CharacterAutostep {
                    max_height: CharacterLength::Absolute(0.35),
                    min_width: CharacterLength::Absolute(0.2),
                    include_dynamic_bodies: false,
                }),
                max_slope_climb_angle: 50.0_f32.to_radians(),
                min_slope_slide_angle: 30.0_f32.to_radians(),
                snap_to_ground: Some(CharacterLength::Absolute(0.3)),
                ..Default::default()
            },
            rigidbody: RigidBody::KinematicPositionBased,
            collider: Collider::capsule_y(capsule_height / 2.0, radius),
            collision_groups: CollisionFilter::player().to_collision_groups(),
            transform: Transform::from_translation(position),
            global_transform: GlobalTransform::default(),
        }
    }

    pub fn player(position: Vec3) -> Self {
        Self::new(1.8, 0.4, position)
    }

    pub fn npc(position: Vec3) -> Self {
        let mut bundle = Self::new(1.7, 0.35, position);
        bundle.controller = CharacterController::npc();
        bundle.collision_groups = CollisionFilter::npc().to_collision_groups();
        bundle
    }
}
