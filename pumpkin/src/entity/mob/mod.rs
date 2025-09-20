use super::{Entity, EntityBase, NBTStorage, ai::path::Navigator, living::LivingEntity};
use crate::entity::ai::control::look_control::LookControl;
use crate::entity::ai::goal::goal_selector::GoalSelector;
use crate::entity::attribute_manager::{AttributeManager, AttributeManagerBuilder};
use crate::server::Server;
use crate::world::World;
use async_trait::async_trait;
use crossbeam::atomic::AtomicCell;
use pumpkin_data::attributes::Attribute;
use pumpkin_data::damage::DamageType;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;
use tokio::sync::Mutex;

pub mod zombie;

pub struct MobEntity {
    pub living_entity: LivingEntity,
    pub goals_selector: GoalSelector, // Mutex isn't needed because all fields of GoalSelector are thread safe
    pub target_selector: GoalSelector,
    pub navigator: Mutex<Navigator>,
    pub target: Mutex<Option<Arc<dyn EntityBase>>>,
    pub look_control: Mutex<LookControl>,
    pub position_target: AtomicCell<BlockPos>,
    pub position_target_range: AtomicI32,
}

impl MobEntity {
    #[must_use]
    pub fn new(entity: Entity, attribute_manager: AttributeManager) -> Self {
        Self {
            living_entity: LivingEntity::new(entity, attribute_manager),
            goals_selector: GoalSelector::default(),
            target_selector: GoalSelector::default(),
            navigator: Mutex::new(Navigator::default()),
            target: Mutex::new(None),
            look_control: Mutex::new(LookControl::default()),
            position_target: AtomicCell::new(BlockPos::ZERO),
            position_target_range: AtomicI32::new(-1),
        }
    }

    #[must_use]
    pub fn mob_entity_attribute_builder() -> AttributeManagerBuilder {
        LivingEntity::living_entitiy_attribute_builder().add(Attribute::FOLLOW_RANGE, 16.0)
    }

    #[must_use]
    pub fn hostile_mob_entity_attribute_builder() -> AttributeManagerBuilder {
        Self::mob_entity_attribute_builder().add_with_fallback_value(Attribute::ATTACK_DAMAGE)
    }

    pub fn is_in_position_target_range(&self) -> bool {
        self.is_in_position_target_range_pos(self.living_entity.entity.block_pos.load())
    }

    pub fn is_in_position_target_range_pos(&self, block_pos: BlockPos) -> bool {
        let position_target_range = self.position_target_range.load(Relaxed);
        if position_target_range == -1 {
            true
        } else {
            self.position_target.load().squared_distance(block_pos)
                < position_target_range * position_target_range
        }
    }

    pub fn set_attacking(&self, _attacking: bool) {
        // TODO: set to data tracker
    }
}

// This trait contains all overridable functions
pub trait Mob: EntityBase + Send + Sync {
    fn get_random(&self) -> rand::rngs::ThreadRng {
        rand::rng()
    }

    fn get_max_look_yaw_change(&self) -> i32 {
        10
    }

    fn get_max_look_pitch_change(&self) -> i32 {
        40
    }

    fn get_max_head_rotation(&self) -> i32 {
        75
    }

    fn get_mob_entity(&self) -> &MobEntity;

    fn get_path_aware_entity(&self) -> Option<&dyn PathAwareEntity> {
        None
    }
}

#[async_trait]
impl<T> EntityBase for T
where
    T: Mob + Send + 'static,
{
    async fn tick(&self, caller: Arc<dyn EntityBase>, server: &Server) {
        let mob_entity = self.get_mob_entity();
        mob_entity.living_entity.tick(caller, server).await;

        let age = mob_entity.living_entity.entity.age.load(Relaxed);
        if (age + mob_entity.living_entity.entity.entity_id) % 2 != 0 && age > 1 {
            mob_entity.target_selector.tick_goals(self, false).await;
            mob_entity.goals_selector.tick_goals(self, false).await;
        } else {
            mob_entity.target_selector.tick(self).await;
            mob_entity.goals_selector.tick(self).await;
        }

        let mut navigator = mob_entity.navigator.lock().await;
        navigator.tick(&mob_entity.living_entity).await;
        drop(navigator);

        let look_control = mob_entity.look_control.lock().await;
        look_control.tick(self).await;
        drop(look_control);
    }

    async fn damage_with_context(
        &self,
        caller: Arc<dyn EntityBase>,
        amount: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.get_mob_entity()
            .living_entity
            .damage_with_context(caller, amount, damage_type, position, source, cause)
            .await
    }

    fn get_entity(&self) -> &Entity {
        &self.get_mob_entity().living_entity.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        Some(&self.get_mob_entity().living_entity)
    }

    fn as_nbt_storage(&self) -> &dyn NBTStorage {
        self
    }

    fn get_gravity(&self) -> f64 {
        self.get_mob_entity().living_entity.get_gravity()
    }
}

#[allow(dead_code)]
const DEFAULT_PATHFINDING_FAVOR: f32 = 0.0;
#[async_trait]
pub trait PathAwareEntity: Mob + Send + Sync {
    fn get_pathfinding_favor(&self, _block_pos: BlockPos, _world: Arc<World>) -> f32 {
        0.0
    }

    // TODO: missing SpawnReason attribute
    fn can_spawn(&self, world: Arc<World>) -> bool {
        self.get_pathfinding_favor(
            self.get_mob_entity().living_entity.entity.block_pos.load(),
            world,
        ) >= 0.0
    }

    async fn is_navigation(&self) -> bool {
        let navigator = self.get_mob_entity().navigator.lock().await;
        !navigator.is_idle()
    }

    // TODO: implement
    fn is_panicking(&self) -> bool {
        false
    }

    fn should_follow_leash(&self) -> bool {
        true
    }

    fn on_short_leash_tick(&self) {
        // TODO: implement
    }

    fn before_leash_tick(&self) {
        // TODO: implement
    }

    fn get_follow_leash_speed(&self) -> f32 {
        1.0
    }
}
