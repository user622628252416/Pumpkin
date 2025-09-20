use crate::entity::item::ItemEntity;
use crate::world::World;
use crate::{server::Server, world::portal::PortalManager};
use async_trait::async_trait;
use bytes::BufMut;
use crossbeam::atomic::AtomicCell;
use living::LivingEntity;
use player::Player;
use pumpkin_data::BlockState;
use pumpkin_data::block_properties::{EnumVariants, Integer0To15};
use pumpkin_data::fluid::Fluid;
use pumpkin_data::{Block, BlockDirection};
use pumpkin_data::{
    block_properties::{Facing, HorizontalFacing},
    damage::DamageType,
    entity::{EntityPose, EntityType},
    sound::{Sound, SoundCategory},
};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_protocol::java::client::play::{CUpdateEntityPos, CUpdateEntityPosRot};
use pumpkin_protocol::{
    codec::var_int::VarInt,
    java::client::play::{
        CEntityPositionSync, CEntityVelocity, CHeadRot, CSetEntityMetadata, CSpawnEntity,
        CUpdateEntityRot, MetaDataType, Metadata,
    },
    ser::serializer::Serializer,
};
use pumpkin_registry::VanillaDimensionType;
use pumpkin_util::math::vector3::Axis;
use pumpkin_util::math::{
    boundingbox::{BoundingBox, EntityDimensions},
    get_section_cord,
    position::BlockPos,
    vector2::Vector2,
    vector3::Vector3,
    wrap_degrees,
};
use pumpkin_util::text::TextComponent;
use pumpkin_util::text::hover::HoverEvent;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{
        AtomicBool, AtomicI32, AtomicU32,
        Ordering::{self, Relaxed},
    },
};
use tokio::sync::Mutex;

pub mod ai;
pub mod attribute_manager;
pub mod decoration;
pub mod effect;
pub mod experience_orb;
pub mod falling;
pub mod hunger;
pub mod item;
pub mod living;
pub mod mob;
pub mod player;
pub mod projectile;
pub mod tnt;
pub mod r#type;

mod combat;
pub mod predicate;

#[async_trait]
pub trait EntityBase: Send + Sync + NBTStorage {
    /// Called every tick for this entity.
    ///
    /// The `caller` parameter is a reference to the entity that initiated the tick.
    /// This can be the same entity the method is being called on (`self`),
    /// but in some scenarios (e.g., interactions or events), it might be a different entity.
    ///
    /// The `server` parameter provides access to the game server instance.
    async fn tick(&self, caller: Arc<dyn EntityBase>, server: &Server) {
        if let Some(living) = self.get_living_entity() {
            living.tick(caller, server).await;
        } else {
            self.get_entity().tick(caller, server).await;
        }
    }

    async fn init_data_tracker(&self) {}

    async fn teleport(
        self: Arc<Self>,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        world: Arc<World>,
    ) {
        self.get_entity()
            .teleport(position, yaw, pitch, world)
            .await;
    }

    fn is_pushed_by_fluids(&self) -> bool {
        true
    }

    fn get_gravity(&self) -> f64 {
        0.0
    }

    /// Returns if damage was successful or not
    async fn damage(
        &self,
        caller: Arc<dyn EntityBase>,
        amount: f32,
        damage_type: DamageType,
    ) -> bool {
        self.damage_with_context(caller, amount, damage_type, None, None, None)
            .await
    }

    fn is_spectator(&self) -> bool {
        false
    }

    fn is_collidable(&self, _entity: Option<Box<dyn EntityBase>>) -> bool {
        false
    }

    fn can_hit(&self) -> bool {
        false
    }

    fn is_flutterer(&self) -> bool {
        false
    }

    async fn damage_with_context(
        &self,
        _caller: Arc<dyn EntityBase>,
        _amount: f32,
        _damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        _source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        // Just do nothing
        false
    }

    /// Called when a player collides with a entity
    async fn on_player_collision(&self, _player: &Arc<Player>) {}
    fn get_entity(&self) -> &Entity;
    fn get_living_entity(&self) -> Option<&LivingEntity>;

    fn get_item_entity(self: Arc<Self>) -> Option<Arc<ItemEntity>> {
        None
    }

    fn get_player(&self) -> Option<&Player> {
        None
    }

    /// Should return the name of the entity without click or hover events.
    fn get_name(&self) -> TextComponent {
        let entity = self.get_entity();
        entity
            .custom_name
            .clone()
            .unwrap_or(TextComponent::translate(
                format!("entity.minecraft.{}", entity.entity_type.resource_name),
                [],
            ))
    }
    async fn get_display_name(&self) -> TextComponent {
        // TODO: team color
        let entity = self.get_entity();
        let mut name = entity
            .custom_name
            .clone()
            .unwrap_or(TextComponent::translate(
                format!("entity.minecraft.{}", entity.entity_type.resource_name),
                [],
            ));
        let name_clone = name.clone();
        name = name.hover_event(HoverEvent::show_entity(
            entity.entity_uuid.to_string(),
            entity.entity_type.resource_name.into(),
            Some(name_clone),
        ));
        name = name.insertion(entity.entity_uuid.to_string());
        name
    }

    /// Kills the Entity.
    async fn kill(&self, caller: Arc<dyn EntityBase>) {
        if let Some(living) = self.get_living_entity() {
            living
                .damage(caller, f32::MAX, DamageType::GENERIC_KILL)
                .await;
        } else {
            // TODO this should be removed once all entities are implemented
            self.get_entity().remove().await;
        }
    }

    /// Returns itself as the nbt storage for saving and loading data.
    fn as_nbt_storage(&self) -> &dyn NBTStorage;
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum RemovalReason {
    Killed,
    Discarded,
    UnloadedToChunk,
    UnloadedWithPlayer,
    ChangedDimension,
}

impl RemovalReason {
    #[must_use]
    pub fn should_destroy(&self) -> bool {
        match self {
            Self::Killed | Self::Discarded => true,
            Self::UnloadedToChunk | Self::UnloadedWithPlayer | Self::ChangedDimension => false,
        }
    }

    #[must_use]
    pub fn should_save(&self) -> bool {
        match self {
            Self::Killed | Self::Discarded | Self::UnloadedWithPlayer | Self::ChangedDimension => {
                false
            }
            Self::UnloadedToChunk => true,
        }
    }
}

static CURRENT_ID: AtomicI32 = AtomicI32::new(0);

/// Represents a non-living Entity (e.g. Item, Egg, Snowball...)
pub struct Entity {
    /// A unique identifier for the entity
    pub entity_id: i32,
    /// A persistent, unique identifier for the entity
    pub entity_uuid: uuid::Uuid,
    /// The type of entity (e.g., player, zombie, item)
    pub entity_type: &'static EntityType,
    /// The world in which the entity exists.
    pub world: Arc<World>,
    /// The entity's current position in the world
    pub pos: AtomicCell<Vector3<f64>>,
    /// The last known position of the entity.
    pub last_pos: AtomicCell<Vector3<f64>>,
    /// The entity's position rounded to the nearest block coordinates
    pub block_pos: AtomicCell<BlockPos>,
    /// The block supporting the entity
    pub supporting_block_pos: AtomicCell<Option<BlockPos>>,
    /// The chunk coordinates of the entity's current position
    pub chunk_pos: AtomicCell<Vector2<i32>>,
    /// Indicates whether the entity is sneaking
    pub sneaking: AtomicBool,
    /// Indicates whether the entity is sprinting
    pub sprinting: AtomicBool,
    /// Indicates whether the entity is flying due to a fall
    pub fall_flying: AtomicBool,
    /// The entity's current velocity vector, aka knockback
    pub velocity: AtomicCell<Vector3<f64>>,
    /// Tracks a horizontal collision
    pub horizontal_collision: AtomicBool,
    /// Indicates whether the entity is on the ground (may not always be accurate).
    pub on_ground: AtomicBool,
    /// Indicates whether the entity is touching water
    pub touching_water: AtomicBool,
    /// Indicates the fluid height
    pub water_height: AtomicCell<f64>,
    /// Indicates whether the entity is touching lava
    pub touching_lava: AtomicBool,
    /// Indicates the fluid height
    pub lava_height: AtomicCell<f64>,
    /// The entity's yaw rotation (horizontal rotation) ← →
    pub yaw: AtomicCell<f32>,
    /// The entity's head yaw rotation (horizontal rotation of the head)
    pub head_yaw: AtomicCell<f32>,
    /// The entity's body yaw rotation (horizontal rotation of the body)
    pub body_yaw: AtomicCell<f32>,
    /// The entity's pitch rotation (vertical rotation) ↑ ↓
    pub pitch: AtomicCell<f32>,
    /// The height of the entity's eyes from the ground.
    pub standing_eye_height: f32,
    /// The entity's current pose (e.g., standing, sitting, swimming).
    pub pose: AtomicCell<EntityPose>,
    /// The bounding box of an entity (hitbox)
    pub bounding_box: AtomicCell<BoundingBox>,
    ///The size (width and height) of the bounding box
    pub bounding_box_size: AtomicCell<EntityDimensions>,
    /// Whether this entity is invulnerable to all damage
    pub invulnerable: AtomicBool,
    /// List of damage types this entity is immune to
    pub damage_immunities: Vec<DamageType>,
    pub fire_ticks: AtomicI32,
    pub has_visual_fire: AtomicBool,
    pub removal_reason: AtomicCell<Option<RemovalReason>>,
    // The passengers that entity has
    pub passengers: Mutex<Vec<Arc<dyn EntityBase>>>,
    /// The vehicle that entity is in
    pub vehicle: Mutex<Option<Arc<dyn EntityBase>>>,
    pub age: AtomicI32,

    pub first_loaded_chunk_position: AtomicCell<Option<Vector3<i32>>>,

    pub portal_cooldown: AtomicU32,

    pub portal_manager: Mutex<Option<Mutex<PortalManager>>>,
    /// Custom name for the entity
    pub custom_name: Option<TextComponent>,
    /// Indicates whether the entity's custom name is visible
    pub custom_name_visible: bool,
    /// The data send in the Entity Spawn packet
    pub data: AtomicI32,
    /// If true, the entity cannot collide with anything (e.g. spectator)
    pub no_clip: AtomicBool,
    /// Multiplies movement for one tick before being reset
    pub movement_multiplier: AtomicCell<Vector3<f64>>,
    /// Determines whether the entity's velocity needs to be sent
    pub velocity_dirty: AtomicBool,
    /// Set when an Entity is to be removed but could still be referenced
    pub removed: AtomicBool,
}

impl Entity {
    pub fn new(
        entity_uuid: uuid::Uuid,
        world: Arc<World>,
        position: Vector3<f64>,
        entity_type: &'static EntityType,
        invulnerable: bool,
    ) -> Self {
        let floor_x = position.x.floor() as i32;
        let floor_y = position.y.floor() as i32;
        let floor_z = position.z.floor() as i32;

        let bounding_box_size = EntityDimensions {
            width: entity_type.dimension[0],
            height: entity_type.dimension[1],
        };

        Self {
            entity_id: CURRENT_ID.fetch_add(1, Relaxed),
            entity_uuid,
            entity_type,
            on_ground: AtomicBool::new(false),
            touching_water: AtomicBool::new(false),
            water_height: AtomicCell::new(0.0),
            touching_lava: AtomicBool::new(false),
            lava_height: AtomicCell::new(0.0),
            horizontal_collision: AtomicBool::new(false),
            pos: AtomicCell::new(position),
            last_pos: AtomicCell::new(position),
            block_pos: AtomicCell::new(BlockPos(Vector3::new(floor_x, floor_y, floor_z))),
            supporting_block_pos: AtomicCell::new(None),
            chunk_pos: AtomicCell::new(Vector2::new(
                get_section_cord(floor_x),
                get_section_cord(floor_z),
            )),
            sneaking: AtomicBool::new(false),
            world,
            sprinting: AtomicBool::new(false),
            fall_flying: AtomicBool::new(false),
            yaw: AtomicCell::new(0.0),
            head_yaw: AtomicCell::new(0.0),
            body_yaw: AtomicCell::new(0.0),
            pitch: AtomicCell::new(0.0),
            velocity: AtomicCell::new(Vector3::new(0.0, 0.0, 0.0)),
            standing_eye_height: entity_type.eye_height,
            pose: AtomicCell::new(EntityPose::Standing),
            first_loaded_chunk_position: AtomicCell::new(None),
            bounding_box: AtomicCell::new(BoundingBox::new_from_pos(
                position.x,
                position.y,
                position.z,
                &bounding_box_size,
            )),
            bounding_box_size: AtomicCell::new(bounding_box_size),
            invulnerable: AtomicBool::new(invulnerable),
            damage_immunities: Vec::new(),
            data: AtomicI32::new(0),
            fire_ticks: AtomicI32::new(-1),
            has_visual_fire: AtomicBool::new(false),
            removal_reason: AtomicCell::new(None),
            passengers: Mutex::new(Vec::new()),
            vehicle: Mutex::new(None),
            age: AtomicI32::new(0),
            portal_cooldown: AtomicU32::new(0),
            portal_manager: Mutex::new(None),
            custom_name: None,
            custom_name_visible: false,
            no_clip: AtomicBool::new(false),
            movement_multiplier: AtomicCell::new(Vector3::default()),
            velocity_dirty: AtomicBool::new(true),
            removed: AtomicBool::new(false),
        }
    }

    pub async fn set_velocity(&self, velocity: Vector3<f64>) {
        self.velocity.store(velocity);
        self.send_velocity().await;
    }

    /// Sets a custom name for the entity, typically used with nametags
    pub async fn set_custom_name(&self, name: TextComponent) {
        self.send_meta_data(&[Metadata::new(
            2,
            MetaDataType::OptionalTextComponent,
            Some(name),
        )])
        .await;
    }

    pub async fn send_velocity(&self) {
        let velocity = self.velocity.load();
        self.world
            .broadcast_packet_all(&CEntityVelocity::new(self.entity_id.into(), velocity))
            .await;
    }

    /// Updates the entity's position, block position, and chunk position.
    ///
    /// This function calculates the new position, block position, and chunk position based on the provided coordinates. If any of these values change, the corresponding fields are updated.
    pub fn set_pos(&self, new_position: Vector3<f64>) {
        let pos = self.pos.load();
        if pos != new_position {
            self.pos.store(new_position);
            self.bounding_box.store(BoundingBox::new_from_pos(
                new_position.x,
                new_position.y,
                new_position.z,
                &self.bounding_box_size.load(),
            ));

            let floor_x = new_position.x.floor() as i32;
            let floor_y = new_position.y.floor() as i32;
            let floor_z = new_position.z.floor() as i32;

            let block_pos = self.block_pos.load();
            let block_pos_vec = block_pos.0;
            if floor_x != block_pos_vec.x
                || floor_y != block_pos_vec.y
                || floor_z != block_pos_vec.z
            {
                let new_block_pos = Vector3::new(floor_x, floor_y, floor_z);
                self.block_pos.store(BlockPos(new_block_pos));

                let chunk_pos = self.chunk_pos.load();
                if get_section_cord(floor_x) != chunk_pos.x
                    || get_section_cord(floor_z) != chunk_pos.y
                {
                    self.chunk_pos.store(Vector2::new(
                        get_section_cord(new_block_pos.x),
                        get_section_cord(new_block_pos.z),
                    ));
                }
            }
        }
    }

    /// Returns entity rotation as vector
    pub fn rotation(&self) -> Vector3<f32> {
        // Convert degrees to radians if necessary
        let yaw_rad = self.yaw.load().to_radians();
        let pitch_rad = self.pitch.load().to_radians();

        Vector3::new(
            yaw_rad.cos() * pitch_rad.cos(),
            pitch_rad.sin(),
            yaw_rad.sin() * pitch_rad.cos(),
        )
        .normalize()
    }

    /// Changes this entity's pitch and yaw to look at target
    pub async fn look_at(&self, target: Vector3<f64>) {
        let position = self.pos.load();
        let delta = target.sub(&position);
        let root = delta.x.hypot(delta.z);
        let pitch = wrap_degrees(-delta.y.atan2(root) as f32 * 180.0 / std::f32::consts::PI);
        let yaw =
            wrap_degrees((delta.z.atan2(delta.x) as f32 * 180.0 / std::f32::consts::PI) - 90.0);
        self.pitch.store(pitch);
        self.yaw.store(yaw);

        self.send_rotation().await;
    }

    pub async fn send_rotation(&self) {
        let yaw = self.yaw.load();
        let pitch = self.pitch.load();

        // Broadcast the update packet.

        // TODO: Do caching to only send the packet when needed.

        let yaw = (yaw * 256.0 / 360.0).rem_euclid(256.0);

        let yaw = (yaw * 256.0 / 360.0).rem_euclid(256.0) as u8;

        let pitch = (pitch * 256.0 / 360.0).rem_euclid(256.0);

        self.world
            .broadcast_packet_all(&CUpdateEntityRot::new(
                self.entity_id.into(),
                yaw,
                pitch as u8,
                self.on_ground.load(Relaxed),
            ))
            .await;

        self.send_head_rot(yaw).await;
    }

    pub async fn send_head_rot(&self, head_yaw: u8) {
        self.world
            .broadcast_packet_all(&CHeadRot::new(self.entity_id.into(), head_yaw))
            .await;
    }

    fn default_portal_cooldown(&self) -> u32 {
        if self.entity_type == &EntityType::PLAYER {
            10
        } else {
            300
        }
    }

    #[allow(clippy::float_cmp)]
    async fn adjust_movement_for_collisions(&self, movement: Vector3<f64>) -> Vector3<f64> {
        self.on_ground.store(false, Ordering::SeqCst);

        self.supporting_block_pos.store(None);

        self.horizontal_collision.store(false, Ordering::SeqCst);

        if movement.length_squared() == 0.0 {
            return movement;
        }

        let bounding_box = self.bounding_box.load();

        let (collisions, block_positions) = self
            .world
            .get_block_collisions(bounding_box.stretch(movement))
            .await;

        if collisions.is_empty() {
            return movement;
        }

        let mut adjusted_movement = movement;

        // Y-Axis adjustment

        if movement.get_axis(Axis::Y) != 0.0 {
            let mut max_time = 1.0;

            let mut positions = block_positions.into_iter();

            let (mut collisions_len, mut position) = positions.next().unwrap();

            let mut supporting_block_pos = None;

            for (i, inert_box) in collisions.iter().enumerate() {
                if i == collisions_len {
                    (collisions_len, position) = positions.next().unwrap();
                }

                if let Some(collision_time) = bounding_box.calculate_collision_time(
                    inert_box,
                    adjusted_movement,
                    Axis::Y,
                    max_time,
                ) {
                    max_time = collision_time;

                    supporting_block_pos = Some(position);
                }
            }

            if max_time != 1.0 {
                let changed_component = adjusted_movement.get_axis(Axis::Y) * max_time;

                adjusted_movement.set_axis(Axis::Y, changed_component);
            }

            self.on_ground
                .store(supporting_block_pos.is_some(), Ordering::SeqCst);

            self.supporting_block_pos.store(supporting_block_pos);
        }

        let mut horizontal_collision = false;

        for axis in Axis::horizontal() {
            if movement.get_axis(axis) == 0.0 {
                continue;
            }

            let mut max_time = 1.0;

            for inert_box in &collisions {
                if let Some(collision_time) = bounding_box.calculate_collision_time(
                    inert_box,
                    adjusted_movement,
                    axis,
                    max_time,
                ) {
                    max_time = collision_time;
                }
            }

            if max_time != 1.0 {
                let changed_component = adjusted_movement.get_axis(axis) * max_time;

                adjusted_movement.set_axis(axis, changed_component);

                horizontal_collision = true;
            }
        }

        self.horizontal_collision
            .store(horizontal_collision, Ordering::SeqCst);

        adjusted_movement
    }

    /// Applies knockback to the entity, following vanilla Minecraft's mechanics.
    /// `LivingEntity.takeKnockback()`
    /// This function calculates the entity's new velocity based on the specified knockback strength and direction.
    pub fn apply_knockback(&self, strength: f64, mut x: f64, mut z: f64) {
        // TODO: strength *= 1 - Entity attribute knockback resistance

        if strength <= 0.0 {
            return;
        }

        self.velocity_dirty.store(true, Ordering::SeqCst);

        // This has some vanilla magic

        while x.mul_add(x, z * z) < 1.0E-5 {
            x = (rand::random::<f64>() - rand::random::<f64>()) * 0.01;

            z = (rand::random::<f64>() - rand::random::<f64>()) * 0.01;
        }

        let var8 = Vector3::new(x, 0.0, z).normalize() * strength;

        let velocity = self.velocity.load();

        self.velocity.store(Vector3::new(
            velocity.x / 2.0 - var8.x,
            if self.on_ground.load(Relaxed) {
                (velocity.y / 2.0 + strength).min(0.4)
            } else {
                velocity.y
            },
            velocity.z / 2.0 - var8.z,
        ));
    }

    // Part of LivingEntity.tickMovement() in yarn

    pub fn check_zero_velo(&self) {
        let mut motion = self.velocity.load();

        if self.entity_type == &EntityType::PLAYER {
            if motion.horizontal_length_squared() < 9.0E-6 {
                motion.x = 0.0;

                motion.z = 0.0;
            }
        } else {
            if motion.x.abs() < 0.003 {
                motion.x = 0.0;
            }

            if motion.z.abs() < 0.003 {
                motion.z = 0.0;
            }
        }

        if motion.y.abs() < 0.003 {
            motion.y = 0.0;
        }

        self.velocity.store(motion);
    }

    #[allow(dead_code)]
    fn tick_block_underneath(_caller: &Arc<dyn EntityBase>) {
        // let world = self.world.read().await;

        // let (pos, block, state) = self.get_block_with_y_offset(0.2).await;

        // world
        //     .block_registry
        //     .on_stepped_on(&world, caller.as_ref(), pos, block, state)
        //     .await;

        // TODO: Add this to on_stepped_on

        /*


        if self.on_ground.load(Ordering::SeqCst) {


            let (_pos, block, state) = self.get_block_with_y_offset(0.2).await;


            if let Some(live) = living {


                if block == Block::CAMPFIRE


                    || block == Block::SOUL_CAMPFIRE


                        && CampfireLikeProperties::from_state_id(state.id, &block).r#signal_fire


                {


                    let _ = live.damage(1.0, DamageType::CAMPFIRE).await;


                }





                if block == Block::MAGMA_BLOCK {


                    let _ = live.damage(1.0, DamageType::HOT_FLOOR).await;


                }


            }


        }


        */
    }

    // Returns whether the entity's eye level is in a wall

    async fn tick_block_collisions(&self, caller: &Arc<dyn EntityBase>, server: &Server) -> bool {
        let bounding_box = self.bounding_box.load();

        let mut suffocating = false;

        let aabb = bounding_box.expand(-0.001, -0.001, -0.001);

        let min = aabb.min_block_pos();

        let max = aabb.max_block_pos();

        let mut eye_level_box = aabb;

        let eye_height = f64::from(self.standing_eye_height);

        eye_level_box.min.y += eye_height;

        eye_level_box.max.y = eye_level_box.min.y;

        for x in min.0.x..=max.0.x {
            for y in min.0.y..=max.0.y {
                for z in min.0.z..=max.0.z {
                    let pos = BlockPos::new(x, y, z);

                    let (block, state) = self.world.get_block_and_state(&pos).await;

                    let collided = World::check_outline(
                        &bounding_box,
                        pos,
                        state,
                        !suffocating && state.is_solid(),
                        |collision_shape: &BoundingBox| {
                            suffocating = collision_shape.intersects(&eye_level_box);
                        },
                    );

                    if collided {
                        self.world
                            .block_registry
                            .on_entity_collision(
                                block,
                                &self.world,
                                caller.as_ref(),
                                &pos,
                                state,
                                server,
                            )
                            .await;
                    }
                }
            }
        }

        suffocating
    }

    pub async fn send_pos_rot(&self) {
        let old = self.update_last_pos();

        let new = self.pos.load();

        let converted = Vector3::new(
            new.x.mul_add(4096.0, -(old.x * 4096.0)) as i16,
            new.y.mul_add(4096.0, -(old.y * 4096.0)) as i16,
            new.z.mul_add(4096.0, -(old.z * 4096.0)) as i16,
        );

        let yaw = self.yaw.load();

        let pitch = self.pitch.load();

        // Broadcast the update packet.

        // TODO: Do caching to only send the packet when needed.

        let yaw = (yaw * 256.0 / 360.0).rem_euclid(256.0) as u8;

        let pitch = (pitch * 256.0 / 360.0).rem_euclid(256.0);

        self.world
            .broadcast_packet_all(&CUpdateEntityPosRot::new(
                self.entity_id.into(),
                Vector3::new(converted.x, converted.y, converted.z),
                yaw,
                pitch as u8,
                self.on_ground.load(Relaxed),
            ))
            .await;
        self.send_head_rot(yaw).await;
    }

    pub fn update_last_pos(&self) -> Vector3<f64> {
        let pos = self.pos.load();
        let old = self.last_pos.load();

        self.last_pos.store(pos);
        old
    }

    pub async fn send_pos(&self) {
        let old = self.update_last_pos();
        let new = self.pos.load();

        let converted = Vector3::new(
            new.x.mul_add(4096.0, -(old.x * 4096.0)) as i16,
            new.y.mul_add(4096.0, -(old.y * 4096.0)) as i16,
            new.z.mul_add(4096.0, -(old.z * 4096.0)) as i16,
        );

        self.world
            .broadcast_packet_all(&CUpdateEntityPos::new(
                self.entity_id.into(),
                Vector3::new(converted.x, converted.y, converted.z),
                self.on_ground.load(Relaxed),
            ))
            .await;
    }

    // updateWaterState() in yarn

    async fn update_fluid_state(&self, caller: &Arc<dyn EntityBase>) {
        let is_pushed = caller.is_pushed_by_fluids();

        let mut fluids = BTreeMap::new();

        let water_push = Vector3::default();

        let water_n = 0;

        let lava_push = Vector3::default();

        let lava_n = 0;

        let mut fluid_push = [water_push, lava_push];

        let mut fluid_n = [water_n, lava_n];

        let mut in_fluid = [false, false];

        // The maximum fluid height found

        let mut fluid_height: [f64; 2] = [0.0, 0.0];

        let bounding_box = self.bounding_box.load().expand(-0.001, -0.001, -0.001);

        let min = bounding_box.min_block_pos();

        let max = bounding_box.max_block_pos();

        for x in min.0.x..=max.0.x {
            for y in min.0.y..=max.0.y {
                for z in min.0.z..=max.0.z {
                    let pos = BlockPos::new(x, y, z);

                    let (fluid, state) = self.world.get_fluid_and_fluid_state(&pos).await;

                    if fluid.id != Fluid::EMPTY.id {
                        let marginal_height =
                            f64::from(state.height) + f64::from(y) - bounding_box.min.y;

                        if marginal_height >= 0.0 {
                            let i = usize::from(
                                fluid.id == Fluid::FLOWING_LAVA.id || fluid.id == Fluid::LAVA.id,
                            );

                            fluid_height[i] = fluid_height[i].max(marginal_height);

                            in_fluid[i] = true;

                            if !is_pushed {
                                fluids.insert(fluid.id, fluid);

                                continue;
                            }

                            let mut fluid_velo =
                                self.world.get_fluid_velocity(pos, &fluid, &state).await;

                            if fluid_height[i] < 0.4 {
                                fluid_velo = fluid_velo * fluid_height[i];
                            }

                            fluid_push[i] += fluid_velo;

                            fluid_n[i] += 1;

                            fluids.insert(fluid.id, fluid);
                        }
                    }
                }
            }
        }

        // BTreeMap auto-sorts water before lava as in vanilla

        for (_, fluid) in fluids {
            self.world
                .block_registry
                .on_entity_collision_fluid(&fluid, caller.as_ref())
                .await;
        }

        let lava_speed = if self.world.dimension_type == VanillaDimensionType::TheNether {
            0.007
        } else {
            0.002_333_333
        };

        self.push_by_fluid(0.014, fluid_push[0], fluid_n[0]);

        self.push_by_fluid(lava_speed, fluid_push[1], fluid_n[1]);

        let water_height = fluid_height[0];

        let in_water = in_fluid[0];

        if in_water {
            if let Some(living) = caller.get_living_entity() {
                living.fall_distance.store(0.0);
            }

            if !self.touching_water.load(Ordering::SeqCst) {

                // TODO: Spawn splash particles
            }
        }

        self.water_height.store(water_height);

        self.touching_water.store(in_water, Ordering::SeqCst);

        let lava_height = fluid_height[1];

        let in_lava = in_fluid[1];

        if in_lava && let Some(living) = caller.get_living_entity() {
            let halved_fall = living.fall_distance.load() / 2.0;

            if halved_fall != 0.0 {
                living.fall_distance.store(halved_fall);
            }
        }

        self.lava_height.store(lava_height);

        self.touching_lava.store(in_lava, Ordering::SeqCst);
    }

    fn push_by_fluid(&self, speed: f64, mut push: Vector3<f64>, n: usize) {
        if push.length_squared() != 0.0 {
            if n > 0 {
                push = push * (1.0 / (n as f64));
            }

            if self.entity_type != &EntityType::PLAYER {
                push = push.normalize();
            }

            push = push * speed;

            let velo = self.velocity.load();

            if velo.x.abs() < 0.003 && velo.z.abs() < 0.003 && velo.length_squared() < 0.000_020_25
            {
                push = push.normalize() * 0.0045;
            }

            self.velocity.store(velo + push);
        }
    }

    async fn get_pos_with_y_offset(
        &self,
        offset: f64,
    ) -> (
        BlockPos,
        Option<&'static Block>,
        Option<&'static BlockState>,
    ) {
        if let Some(mut supporting_block) = self.supporting_block_pos.load() {
            if offset > 1.0e-5 {
                let (block, state) = self.world.get_block_and_state(&supporting_block).await;

                // if let Some(props) = block.properties(state.id) {
                //     let name = props.;

                //     if offset <= 0.5
                //         && (name == "OakFenceLikeProperties"
                //             || name == "ResinBrickWallLikeProperties"
                //             || name == "OakFenceGateLikeProperties"
                //                 && OakFenceGateLikeProperties::from_state_id(state.id, &block)
                //                     .r#open)
                //     {
                //         return (supporting_block, Some(block), Some(state));
                //     }
                // }

                supporting_block.0.y = (self.pos.load().y - offset).floor() as i32;

                return (supporting_block, Some(block), Some(state));
            }

            return (supporting_block, None, None);
        }

        let mut block_pos = self.block_pos.load();

        block_pos.0.y = (self.pos.load().y - offset).floor() as i32;

        (block_pos, None, None)
    }

    async fn get_block_with_y_offset(
        &self,
        offset: f64,
    ) -> (BlockPos, &'static Block, &'static BlockState) {
        let (pos, block, state) = self.get_pos_with_y_offset(offset).await;

        if let (Some(b), Some(s)) = (block, state) {
            (pos, b, s)
        } else {
            let (b, s) = self.world.get_block_and_state(&pos).await;

            (pos, b, s)
        }
    }

    // Entity.updateVelocity in yarn

    fn update_velocity_from_input(&self, movement_input: Vector3<f64>, speed: f64) {
        let final_input = self.movement_input_to_velocity(movement_input, speed);

        self.velocity.store(self.velocity.load() + final_input);
    }

    // Entity.movementInputToVelocity in yarn

    #[allow(dead_code)]
    fn movement_input_to_velocity(&self, movement_input: Vector3<f64>, speed: f64) -> Vector3<f64> {
        let yaw = f64::from(self.yaw.load()).to_radians();

        let dist = movement_input.length_squared();

        if dist < 1.0e-7 {
            return Vector3::default();
        }

        let input = if dist > 1.0 {
            movement_input.normalize()
        } else {
            movement_input * speed
        };

        let sin = yaw.sin();

        let cos = yaw.cos();

        Vector3::new(
            input.x * cos - input.z * sin,
            input.y,
            input.z * cos + input.x * sin,
        )
    }

    #[allow(clippy::float_cmp)]
    async fn get_velocity_multiplier(&self) -> f32 {
        let block = self.world.get_block(&self.block_pos.load()).await;

        let multiplier = block.velocity_multiplier;

        if multiplier != 1.0 || block == &Block::WATER || block == &Block::BUBBLE_COLUMN {
            multiplier
        } else {
            let (_pos, block, _state) = self.get_block_with_y_offset(0.500_001).await;

            block.velocity_multiplier
        }
    }

    #[allow(clippy::float_cmp)]
    async fn get_jump_velocity_multiplier(&self) -> f32 {
        let f = self
            .world
            .get_block(&self.block_pos.load())
            .await
            .jump_velocity_multiplier;

        let g = self
            .get_block_with_y_offset(0.500_001)
            .await
            .1
            .jump_velocity_multiplier;

        if f == 1f32 { g } else { f }
    }

    pub fn move_pos(&self, delta: Vector3<f64>) {
        self.set_pos(self.pos.load() + delta);
    }

    // Move by a delta, adjust for collisions, and send

    // Does not send movement. That must be done separately
    async fn move_entity(&self, caller: Arc<dyn EntityBase>, mut motion: Vector3<f64>) {
        if caller.get_player().is_some() {
            return;
        }

        if self.no_clip.load(Ordering::Relaxed) {
            self.move_pos(motion);

            return;
        }

        let movement_multiplier = self.movement_multiplier.swap(Vector3::default());

        if movement_multiplier.length_squared() > 1.0e-7 {
            motion = motion.multiply(
                movement_multiplier.x,
                movement_multiplier.y,
                movement_multiplier.z,
            );

            self.velocity.store(Vector3::default());
        }

        let final_move = self.adjust_movement_for_collisions(motion).await;

        self.move_pos(final_move);

        let velocity_multiplier = f64::from(self.get_velocity_multiplier().await);

        self.velocity.store(final_move * velocity_multiplier);

        if let Some(living) = caller.get_living_entity() {
            living
                .update_fall_distance(
                    caller.clone(),
                    final_move.y,
                    self.on_ground.load(Ordering::SeqCst),
                    false,
                )
                .await;
        }
    }

    pub async fn push_out_of_blocks(&self, center_pos: Vector3<f64>) {
        let block_pos = BlockPos::floored_v(center_pos);

        let delta = center_pos.sub(&block_pos.0.to_f64());

        let mut min_dist = f64::MAX;

        let mut direction = BlockDirection::Up;

        for dir in BlockDirection::all() {
            if dir == BlockDirection::Down {
                continue;
            }

            let offset = dir.to_offset();

            if self
                .world
                .get_block_state(&block_pos.offset(offset))
                .await
                .is_full_cube()
            {
                continue;
            }

            let component = delta.get_axis(dir.to_axis().into());

            let dist = if dir.positive() {
                1.0 - component
            } else {
                component
            };

            if dist < min_dist {
                min_dist = dist;

                direction = dir;
            }
        }

        let amplitude = rand::random::<f64>() * 0.2 + 0.1;

        let axis = direction.to_axis().into();

        let sign = if direction.positive() { 1.0 } else { -1.0 };

        let mut velo = self.velocity.load();

        velo = velo * 0.75;

        velo.set_axis(axis, sign * amplitude);

        self.velocity.store(velo);
    }

    async fn tick_portal(&self, caller: &Arc<dyn EntityBase>) {
        if self.portal_cooldown.load(Ordering::Relaxed) > 0 {
            self.portal_cooldown.fetch_sub(1, Ordering::Relaxed);
        }
        let mut manager_guard = self.portal_manager.lock().await;
        // I know this is ugly, but a quick fix because i can't modify the thing while using it
        let mut should_remove = false;
        if let Some(pmanager_mutex) = manager_guard.as_ref() {
            let mut portal_manager = pmanager_mutex.lock().await;
            if portal_manager.tick() {
                // reset cooldown
                self.portal_cooldown
                    .store(self.default_portal_cooldown(), Ordering::Relaxed);
                let pos = self.pos.load();
                // TODO: this is bad
                let scale_factor_new = if portal_manager.portal_world.dimension_type
                    == VanillaDimensionType::TheNether
                {
                    8.0
                } else {
                    1.0
                };
                // TODO: this is bad
                let scale_factor_current =
                    if self.world.dimension_type == VanillaDimensionType::TheNether {
                        8.0
                    } else {
                        1.0
                    };
                let scale_factor = scale_factor_current / scale_factor_new;
                // TODO
                let pos = BlockPos::floored(pos.x * scale_factor, pos.y, pos.z * scale_factor);
                caller
                    .clone()
                    .teleport(
                        pos.0.to_f64(),
                        None,
                        None,
                        portal_manager.portal_world.clone(),
                    )
                    .await;
                drop(portal_manager);
            } else if portal_manager.ticks_in_portal == 0 {
                should_remove = true;
            }
        }
        if should_remove {
            *manager_guard = None;
        }
    }

    pub async fn try_use_portal(&self, portal_delay: u32, portal_world: Arc<World>, pos: BlockPos) {
        if self.portal_cooldown.load(Ordering::Relaxed) > 0 {
            self.portal_cooldown
                .store(self.default_portal_cooldown(), Ordering::Relaxed);
            return;
        }
        let mut manager = self.portal_manager.lock().await;
        if manager.is_none() {
            *manager = Some(Mutex::new(PortalManager::new(
                portal_delay,
                portal_world,
                pos,
            )));
        } else if let Some(manager) = manager.as_ref() {
            let mut manager = manager.lock().await;
            manager.pos = pos;
            manager.in_portal = true;
        }
    }

    /// Extinguishes this entity.
    pub fn extinguish(&self) {
        self.fire_ticks.store(0, Ordering::Relaxed);
    }

    pub fn set_on_fire_for(&self, seconds: f32) {
        self.set_on_fire_for_ticks((seconds * 20.0).floor() as u32);
    }

    pub fn set_on_fire_for_ticks(&self, ticks: u32) {
        if self.fire_ticks.load(Ordering::Relaxed) < ticks as i32 {
            self.fire_ticks.store(ticks as i32, Ordering::Relaxed);
        }
        // TODO: defrost
    }

    /// Sets the `Entity` yaw & pitch rotation
    pub fn set_rotation(&self, yaw: f32, pitch: f32) {
        // TODO
        self.yaw.store(yaw);
        self.set_pitch(pitch);
    }

    pub fn set_pitch(&self, pitch: f32) {
        self.pitch.store(pitch.clamp(-90.0, 90.0) % 360.0);
    }

    /// Removes the `Entity` from their current `World`
    pub async fn remove(&self) {
        self.world.remove_entity(self).await;
    }

    pub fn create_spawn_packet(&self) -> CSpawnEntity {
        let entity_loc = self.pos.load();
        let entity_vel = self.velocity.load();
        CSpawnEntity::new(
            VarInt(self.entity_id),
            self.entity_uuid,
            VarInt(i32::from(self.entity_type.id)),
            entity_loc,
            self.pitch.load(),
            self.yaw.load(),
            self.head_yaw.load(), // todo: head_yaw and yaw are swapped, find out why
            self.data.load(Relaxed).into(),
            entity_vel,
        )
    }
    pub fn width(&self) -> f32 {
        self.bounding_box_size.load().width
    }

    pub fn height(&self) -> f32 {
        self.bounding_box_size.load().height
    }

    /// Applies knockback to the entity, following vanilla Minecraft's mechanics.
    ///
    /// This function calculates the entity's new velocity based on the specified knockback strength and direction.
    pub fn knockback(&self, strength: f64, x: f64, z: f64) {
        // This has some vanilla magic
        let mut x = x;
        let mut z = z;
        while x.mul_add(x, z * z) < 1.0E-5 {
            x = (rand::random::<f64>() - rand::random::<f64>()) * 0.01;
            z = (rand::random::<f64>() - rand::random::<f64>()) * 0.01;
        }

        let var8 = Vector3::new(x, 0.0, z).normalize() * strength;
        let velocity = self.velocity.load();
        self.velocity.store(Vector3::new(
            velocity.x / 2.0 - var8.x,
            if self.on_ground.load(Relaxed) {
                (velocity.y / 2.0 + strength).min(0.4)
            } else {
                velocity.y
            },
            velocity.z / 2.0 - var8.z,
        ));
    }

    pub async fn set_sneaking(&self, sneaking: bool) {
        //assert!(self.sneaking.load(Relaxed) != sneaking);
        self.sneaking.store(sneaking, Relaxed);
        self.set_flag(Flag::Sneaking, sneaking).await;
        if sneaking {
            self.set_pose(EntityPose::Crouching).await;
        } else {
            self.set_pose(EntityPose::Standing).await;
        }
    }

    pub async fn set_on_fire(&self, on_fire: bool) {
        if self.has_visual_fire.load(Ordering::Relaxed) != on_fire {
            self.has_visual_fire.store(on_fire, Ordering::Relaxed);
            self.set_flag(Flag::OnFire, on_fire).await;
        }
    }

    pub fn get_horizontal_facing(&self) -> HorizontalFacing {
        let adjusted_yaw = self.yaw.load().rem_euclid(360.0); // Normalize yaw to [0, 360)

        match adjusted_yaw {
            0.0..=45.0 | 315.0..=360.0 => HorizontalFacing::South,
            45.0..=135.0 => HorizontalFacing::West,
            135.0..=225.0 => HorizontalFacing::North,
            225.0..=315.0 => HorizontalFacing::East,
            _ => HorizontalFacing::South, // Default case, should not occur
        }
    }

    pub fn get_rotation_16(&self) -> Integer0To15 {
        let adjusted_yaw = self.yaw.load().rem_euclid(360.0);

        let index = (adjusted_yaw / 22.5).round() as u16 % 16;

        Integer0To15::from_index(index)
    }

    pub fn get_flipped_rotation_16(&self) -> Integer0To15 {
        match self.get_rotation_16() {
            Integer0To15::L0 => Integer0To15::L8,
            Integer0To15::L1 => Integer0To15::L9,
            Integer0To15::L2 => Integer0To15::L10,
            Integer0To15::L3 => Integer0To15::L11,
            Integer0To15::L4 => Integer0To15::L12,
            Integer0To15::L5 => Integer0To15::L13,
            Integer0To15::L6 => Integer0To15::L14,
            Integer0To15::L7 => Integer0To15::L15,
            Integer0To15::L8 => Integer0To15::L0,
            Integer0To15::L9 => Integer0To15::L1,
            Integer0To15::L10 => Integer0To15::L2,
            Integer0To15::L11 => Integer0To15::L3,
            Integer0To15::L12 => Integer0To15::L4,
            Integer0To15::L13 => Integer0To15::L5,
            Integer0To15::L14 => Integer0To15::L6,
            Integer0To15::L15 => Integer0To15::L7,
        }
    }

    pub fn get_facing(&self) -> Facing {
        let pitch = self.pitch.load().to_radians();
        let yaw = -self.yaw.load().to_radians();

        let (sin_p, cos_p) = pitch.sin_cos();
        let (sin_y, cos_y) = yaw.sin_cos();

        let x = sin_y * cos_p;
        let y = -sin_p;
        let z = cos_y * cos_p;

        let ax = x.abs();
        let ay = y.abs();
        let az = z.abs();

        if ax > ay && ax > az {
            if x > 0.0 { Facing::East } else { Facing::West }
        } else if ay > ax && ay > az {
            if y > 0.0 { Facing::Up } else { Facing::Down }
        } else if z > 0.0 {
            Facing::South
        } else {
            Facing::North
        }
    }

    pub fn get_entity_facing_order(&self) -> [Facing; 6] {
        let pitch = self.pitch.load().to_radians();
        let yaw = -self.yaw.load().to_radians();

        let sin_p = pitch.sin();
        let cos_p = pitch.cos();
        let sin_y = yaw.sin();
        let cos_y = yaw.cos();

        let east_west = if sin_y > 0.0 {
            Facing::East
        } else {
            Facing::West
        };
        let up_down = if sin_p < 0.0 {
            Facing::Up
        } else {
            Facing::Down
        };
        let south_north = if cos_y > 0.0 {
            Facing::South
        } else {
            Facing::North
        };

        let x_axis = sin_y.abs();
        let y_axis = sin_p.abs();
        let z_axis = cos_y.abs();
        let x_weight = x_axis * cos_p;
        let z_weight = z_axis * cos_p;

        let (first, second, third) = if x_axis > z_axis {
            if y_axis > x_weight {
                (up_down, east_west, south_north)
            } else if z_weight > y_axis {
                (east_west, south_north, up_down)
            } else {
                (east_west, up_down, south_north)
            }
        } else if y_axis > z_weight {
            (up_down, south_north, east_west)
        } else if x_weight > y_axis {
            (south_north, east_west, up_down)
        } else {
            (south_north, up_down, east_west)
        };

        [
            first,
            second,
            third,
            third.opposite(),
            second.opposite(),
            first.opposite(),
        ]
    }

    pub async fn set_sprinting(&self, sprinting: bool) {
        //assert!(self.sprinting.load(Relaxed) != sprinting);
        self.sprinting.store(sprinting, Relaxed);
        self.set_flag(Flag::Sprinting, sprinting).await;
    }

    pub fn check_fall_flying(&self) -> bool {
        !self.on_ground.load(Relaxed)
    }

    pub async fn set_fall_flying(&self, fall_flying: bool) {
        assert!(self.fall_flying.load(Relaxed) != fall_flying);
        self.fall_flying.store(fall_flying, Relaxed);
        self.set_flag(Flag::FallFlying, fall_flying).await;
    }

    async fn set_flag(&self, flag: Flag, value: bool) {
        let index = flag as u8;
        let mut b = 0i8;
        if value {
            b |= 1 << index;
        } else {
            b &= !(1 << index);
        }
        self.send_meta_data(&[Metadata::new(0, MetaDataType::Byte, b)])
            .await;
    }

    /// Plays sound at this entity's position with the entity's sound category
    pub async fn play_sound(&self, sound: Sound) {
        self.world
            .play_sound(sound, SoundCategory::Neutral, &self.pos.load())
            .await;
    }

    pub async fn send_meta_data<T: Serialize>(&self, meta: &[Metadata<T>]) {
        let mut buf = Vec::new();
        for meta in meta {
            let mut serializer_buf = Vec::new();
            let mut serializer = Serializer::new(&mut serializer_buf);
            meta.serialize(&mut serializer).unwrap();
            buf.extend(serializer_buf);
        }
        buf.put_u8(255);
        self.world
            .broadcast_packet_all(&CSetEntityMetadata::new(self.entity_id.into(), buf.into()))
            .await;
    }

    pub async fn set_pose(&self, pose: EntityPose) {
        self.pose.store(pose);
        let pose = pose as i32;
        self.send_meta_data(&[Metadata::new(6, MetaDataType::EntityPose, VarInt(pose))])
            .await;
    }

    pub fn is_invulnerable_to(&self, damage_type: &DamageType) -> bool {
        *damage_type != DamageType::GENERIC_KILL
            && (self.invulnerable.load(Relaxed) || self.damage_immunities.contains(damage_type))
    }

    pub async fn check_block_collision(entity: &dyn EntityBase, server: &Server) {
        let aabb = entity.get_entity().bounding_box.load();
        let blockpos = BlockPos::new(
            (aabb.min.x + 0.001).floor() as i32,
            (aabb.min.y + 0.001).floor() as i32,
            (aabb.min.z + 0.001).floor() as i32,
        );
        let blockpos1 = BlockPos::new(
            (aabb.max.x - 0.001).floor() as i32,
            (aabb.max.y - 0.001).floor() as i32,
            (aabb.max.z - 0.001).floor() as i32,
        );
        let world = &entity.get_entity().world;

        for x in blockpos.0.x..=blockpos1.0.x {
            for y in blockpos.0.y..=blockpos1.0.y {
                for z in blockpos.0.z..=blockpos1.0.z {
                    let pos = BlockPos::new(x, y, z);
                    let (block, state) = world.get_block_and_state(&pos).await;
                    let block_outlines = state.get_block_outline_shapes();

                    if let Some(outlines) = block_outlines {
                        if outlines.is_empty() {
                            world
                                .block_registry
                                .on_entity_collision(block, world, entity, &pos, state, server)
                                .await;
                            let fluid = world.get_fluid(&pos).await;
                            world
                                .block_registry
                                .on_entity_collision_fluid(fluid, entity)
                                .await;
                            continue;
                        }
                        for outline in outlines {
                            let outline_aabb = outline.at_pos(pos);
                            if outline_aabb.intersects(&aabb) {
                                world
                                    .block_registry
                                    .on_entity_collision(block, world, entity, &pos, state, server)
                                    .await;
                                let fluid = world.get_fluid(&pos).await;
                                world
                                    .block_registry
                                    .on_entity_collision_fluid(fluid, entity)
                                    .await;
                                break;
                            }
                        }
                    } else {
                        world
                            .block_registry
                            .on_entity_collision(block, world, entity, &pos, state, server)
                            .await;
                        let fluid = world.get_fluid(&pos).await;
                        world
                            .block_registry
                            .on_entity_collision_fluid(fluid, entity)
                            .await;
                    }
                }
            }
        }
    }

    async fn teleport(
        &self,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        _world: Arc<World>,
    ) {
        // TODO: handle world change
        self.world
            .broadcast_packet_all(&CEntityPositionSync::new(
                self.entity_id.into(),
                position,
                Vector3::new(0.0, 0.0, 0.0),
                yaw.unwrap_or(0.0),
                pitch.unwrap_or(0.0),
                self.on_ground.load(Ordering::SeqCst),
            ))
            .await;
    }

    pub fn get_eye_y(&self) -> f64 {
        self.pos.load().y + f64::from(self.standing_eye_height)
    }

    pub fn is_removed(&self) -> bool {
        self.removal_reason.load().is_some()
    }

    pub fn is_alive(&self) -> bool {
        !self.is_removed()
    }

    pub async fn has_passengers(&self) -> bool {
        !self.passengers.lock().await.is_empty()
    }

    pub async fn has_vehicle(&self) -> bool {
        let vehicle = self.vehicle.lock().await;
        vehicle.is_some()
    }

    pub async fn check_out_of_world(&self, dyn_self: Arc<dyn EntityBase>) {
        if self.pos.load().y < f64::from(self.world.generation_settings().shape.min_y) - 64.0 {
            // Tick out of world damage
            dyn_self
                .damage(dyn_self.clone(), 4.0, DamageType::OUT_OF_WORLD)
                .await;
        }
    }

    #[allow(clippy::unused_async)]
    pub async fn reset_state(&self) {
        self.pose.store(EntityPose::Standing);
        self.fall_flying.store(false, Relaxed);
    }
}

#[async_trait]
impl NBTStorage for Entity {
    async fn write_nbt(&self, nbt: &mut NbtCompound) {
        let position = self.pos.load();
        nbt.put_string(
            "id",
            format!("minecraft:{}", self.entity_type.resource_name),
        );
        let uuid = self.entity_uuid.as_u128();
        nbt.put(
            "UUID",
            NbtTag::IntArray(vec![
                (uuid >> 96) as i32,
                ((uuid >> 64) & 0xFFFF_FFFF) as i32,
                ((uuid >> 32) & 0xFFFF_FFFF) as i32,
                (uuid & 0xFFFF_FFFF) as i32,
            ]),
        );
        nbt.put(
            "Pos",
            NbtTag::List(vec![
                position.x.into(),
                position.y.into(),
                position.z.into(),
            ]),
        );
        let velocity = self.velocity.load();
        nbt.put(
            "Motion",
            NbtTag::List(vec![
                velocity.x.into(),
                velocity.y.into(),
                velocity.z.into(),
            ]),
        );
        nbt.put(
            "Rotation",
            NbtTag::List(vec![self.yaw.load().into(), self.pitch.load().into()]),
        );
        nbt.put_short("Fire", self.fire_ticks.load(Relaxed) as i16);
        nbt.put_bool("OnGround", self.on_ground.load(Relaxed));
        nbt.put_bool("Invulnerable", self.invulnerable.load(Relaxed));
        nbt.put_int("PortalCooldown", self.portal_cooldown.load(Relaxed) as i32);
        if self.has_visual_fire.load(Relaxed) {
            nbt.put_bool("HasVisualFire", true);
        }

        // todo more...
    }

    async fn read_nbt_non_mut(&self, nbt: &NbtCompound) {
        let position = nbt.get_list("Pos").unwrap();
        let x = position[0].extract_double().unwrap_or(0.0);
        let y = position[1].extract_double().unwrap_or(0.0);
        let z = position[2].extract_double().unwrap_or(0.0);
        let pos = Vector3::new(x, y, z);
        self.set_pos(pos);
        self.first_loaded_chunk_position.store(Some(pos.to_i32()));
        let velocity = nbt.get_list("Motion").unwrap();
        let x = velocity[0].extract_double().unwrap_or(0.0);
        let y = velocity[1].extract_double().unwrap_or(0.0);
        let z = velocity[2].extract_double().unwrap_or(0.0);
        self.velocity.store(Vector3::new(x, y, z));
        let rotation = nbt.get_list("Rotation").unwrap();
        let yaw = rotation[0].extract_float().unwrap_or(0.0);
        let pitch = rotation[1].extract_float().unwrap_or(0.0);
        self.set_rotation(yaw, pitch);
        self.head_yaw.store(yaw);
        self.fire_ticks
            .store(i32::from(nbt.get_short("Fire").unwrap_or(0)), Relaxed);
        self.on_ground
            .store(nbt.get_bool("OnGround").unwrap_or(false), Relaxed);
        self.invulnerable
            .store(nbt.get_bool("Invulnerable").unwrap_or(false), Relaxed);
        self.portal_cooldown
            .store(nbt.get_int("PortalCooldown").unwrap_or(0) as u32, Relaxed);
        self.has_visual_fire
            .store(nbt.get_bool("HasVisualFire").unwrap_or(false), Relaxed);
        // todo more...
    }
}

#[async_trait]
impl EntityBase for Entity {
    async fn tick(&self, caller: Arc<dyn EntityBase>, _server: &Server) {
        self.tick_portal(&caller).await;
        self.update_fluid_state(&caller).await;
        self.check_out_of_world(caller.clone()).await;
        let fire_ticks = self.fire_ticks.load(Ordering::Relaxed);
        if fire_ticks > 0 {
            if self.entity_type.fire_immune {
                self.fire_ticks.store(fire_ticks - 4, Ordering::Relaxed);
                if self.fire_ticks.load(Ordering::Relaxed) < 0 {
                    self.extinguish();
                }
            } else {
                if fire_ticks % 20 == 0 {
                    caller
                        .damage(caller.clone(), 1.0, DamageType::ON_FIRE)
                        .await;
                }

                self.fire_ticks.store(fire_ticks - 1, Ordering::Relaxed);
            }
        }
        self.set_on_fire(self.fire_ticks.load(Ordering::Relaxed) > 0)
            .await;
        // TODO: Tick
    }

    async fn teleport(
        self: Arc<Self>,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        world: Arc<World>,
    ) {
        // TODO: handle world change
        self.teleport(position, yaw, pitch, world).await;
    }

    fn get_entity(&self) -> &Entity {
        self
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn as_nbt_storage(&self) -> &dyn NBTStorage {
        self
    }
}

#[async_trait]
pub trait NBTStorage: Send + Sync {
    async fn write_nbt(&self, _nbt: &mut NbtCompound) {}

    async fn read_nbt(&mut self, nbt: &mut NbtCompound) {
        self.read_nbt_non_mut(nbt).await;
    }

    async fn read_nbt_non_mut(&self, _nbt: &NbtCompound) {}
}

#[async_trait]
pub trait NBTStorageInit: Send + Sync + Sized {
    /// Creates an instance of the type from NBT data. If the NBT data is invalid or cannot be parsed, it returns `None`.
    async fn create_from_nbt(_nbt: &mut NbtCompound) -> Option<Self> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Represents various entity flags that are sent in entity metadata.
///
/// These flags are used by the client to modify the rendering of entities based on their current state.
///
/// **Purpose:**
///
/// This enum provides a more type-safe and readable way to represent entity flags compared to using raw integer values.
pub enum Flag {
    /// Indicates if the entity is on fire.
    OnFire = 0,
    /// Indicates if the entity is sneaking.
    Sneaking = 1,
    /// Indicates if the entity is sprinting.
    Sprinting = 3,
    /// Indicates if the entity is swimming.
    Swimming = 4,
    /// Indicates if the entity is invisible.
    Invisible = 5,
    /// Indicates if the entity is glowing.
    Glowing = 6,
    /// Indicates if the entity is flying due to a fall.
    FallFlying = 7,
}
