use std::{collections::HashMap, sync::Arc};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    AttributeModifierSlot,
    attributes::Attribute,
    data_component_impl::{AttributeModifiersImpl, Modifier, Operation},
    effect::StatusEffect,
    potion::Effect,
};
use pumpkin_inventory::entity_equipment::EntityEquipment;
use pumpkin_protocol::java::client::play::{
    CUpdateAttributes, CUpdateAttributesEntry, CUpdateAttributesModifier,
};
use pumpkin_world::item::ItemStack;
use tokio::sync::Mutex;

use crate::entity::Entity;

#[derive(Debug)]
pub enum AttributeError {
    AttributeNotFound,
    ModifierAlreadyExists(Attribute),
    ModifierDoesNotExist,
}

struct AttributeData {
    default: f64,
    current_base_value: AtomicCell<f64>,
    /// whether update packets should be sent
    tracked: bool,
}

struct ModifierStorage {
    equipment: Vec<(&'static str, Attribute, Operation, f64)>,
    effects: Vec<(&'static str, Attribute, Operation, f64)>,
    commands: Vec<(String, Attribute, Operation, f64)>,
}

impl ModifierStorage {
    fn iter(&self) -> impl Iterator<Item = (&str, Attribute, Operation, f64)> {
        self.equipment
            .iter()
            .chain(self.effects.iter())
            .copied()
            .chain(self.commands.iter().map(|(id, attr, op, value)| {
                let id: &str = id;
                (id, *attr, *op, *value)
            }))
    }
}

/// Entities have attributes such as attack damage, scale, armor, etc.
/// This struct keeps track of an entity's base values for each of the applicable attributes and calculates the total attribute value if modifiers (such as those of held items or status effects) are provided.
pub struct AttributeManager {
    attributes: HashMap<Attribute, AttributeData>,
    modifiers: Mutex<ModifierStorage>,
}

impl AttributeManager {
    #[must_use]
    pub fn builder() -> AttributeManagerBuilder {
        AttributeManagerBuilder::new()
    }

    /// Reads the base value of `attr` without equipment and effect modifiers.
    pub fn get_base(&self, attr: Attribute) -> Result<f64, AttributeError> {
        self.attributes
            .get(&attr)
            .map_or(Err(AttributeError::AttributeNotFound), |v| {
                Ok(v.current_base_value.load())
            })
    }

    /// todo: check if value is in allowed range for attribute
    pub async fn set_base(
        &self,
        entity: &Entity,
        attr: Attribute,
        value: f64,
    ) -> Result<(), AttributeError> {
        let Some(attr_value) = self.attributes.get(&attr) else {
            return Err(AttributeError::AttributeNotFound);
        };
        attr_value.current_base_value.store(value);

        if attr_value.tracked {
            self.broadcast_attribute_update(entity).await;
        }

        Ok(())
    }

    pub async fn reset_base(
        &self,
        entity: &Entity,
        attr: Attribute,
    ) -> Result<f64, AttributeError> {
        let Some(attr_value) = self.attributes.get(&attr) else {
            return Err(AttributeError::AttributeNotFound);
        };
        attr_value.current_base_value.store(attr_value.default);

        if attr_value.tracked {
            self.broadcast_attribute_update(entity).await;
        }

        Ok(attr_value.default)
    }

    /// Reads the base value of `attr` and applies equipment and effect modifiers before returning it.
    pub async fn get_modified(&self, attr: Attribute) -> Result<f64, AttributeError> {
        let Some(attr_data) = self.attributes.get(&attr) else {
            return Err(AttributeError::AttributeNotFound);
        };

        let base = attr_data.current_base_value.load();
        let mut modified = base;

        for (_mod_id, mod_attr, op, value) in self.modifiers.lock().await.iter() {
            if mod_attr != attr {
                continue;
            }

            match op {
                Operation::AddValue => modified += value,
                Operation::AddMultipliedBase => modified += value * base,
                Operation::AddMultipliedTotal => modified += value * modified,
            };
        }

        Ok(modified)
    }

    /// `main_hand` is only necessary when main hand is not included in `equipment`, i.e. for player entities. <- todo: fix this
    pub async fn notify_equipment_changed(
        &mut self,
        entity: &Entity,
        equipment: &Mutex<EntityEquipment>,
        main_hand: Option<Arc<Mutex<ItemStack>>>,
    ) {
        {
            let modifiers = &mut self.modifiers.lock().await.equipment;

            modifiers.clear();

            // Make sure lock is released immediatly after this block.
            // Using slot discriminant is not super pretty but it can be copied very cheaply.
            let mut equipment_data = {
                equipment
                    .lock()
                    .await
                    .equipment
                    .iter()
                    .map(|(slot, stack)| (slot.discriminant(), stack.clone()))
                    .collect::<Vec<_>>()
            };

            if let Some(main_hand) = main_hand {
                equipment_data.push((0, main_hand));
            }

            for (slot, stack_lock) in equipment_data {
                let stack = stack_lock.lock().await;

                let item_modifiers: &[Modifier] =
                    match stack.get_data_component::<AttributeModifiersImpl>() {
                        Some(modifiers) => &modifiers.attribute_modifiers,
                        None => continue,
                    };

                for modifier in item_modifiers {
                    // whether modifier is active when it's in its current slot
                    let is_applicable = match modifier.slot {
                        AttributeModifierSlot::Any => true,
                        AttributeModifierSlot::MainHand => slot == 0,
                        AttributeModifierSlot::OffHand => slot == 1,
                        AttributeModifierSlot::Hand => (0..=1).contains(&slot),
                        AttributeModifierSlot::Feet => slot == 2,
                        AttributeModifierSlot::Legs => slot == 3,
                        AttributeModifierSlot::Chest => slot == 4,
                        AttributeModifierSlot::Head => slot == 5,
                        AttributeModifierSlot::Armor => (2..=5).contains(&slot),
                        AttributeModifierSlot::Body => slot == 6,
                        AttributeModifierSlot::Saddle => slot == 7,
                    };
                    if !is_applicable {
                        continue;
                    }

                    modifiers.push((
                        modifier.id,
                        *modifier.r#type,
                        modifier.operation,
                        modifier.amount,
                    ));
                }
            }
        }

        self.broadcast_attribute_update(entity).await;
    }

    pub async fn notify_status_effects_changed(
        &self,
        entity: &Entity,
        effects: &Mutex<HashMap<&'static StatusEffect, Effect>>,
    ) {
        {
            let modifiers = &mut self.modifiers.lock().await.effects;

            modifiers.clear();

            for (status_effect, effect) in effects.lock().await.iter() {
                for modifier in status_effect.attribute_modifiers {
                    // effect modifiers scale linearly based on their amplifier
                    let amount = modifier.base_value * (effect.amplifier as f64 + 1.0);
                    modifiers.push((modifier.id, *modifier.attribute, modifier.operation, amount));
                }
            }
        }

        self.broadcast_attribute_update(entity).await;
    }

    pub async fn add_modifier(
        &self,
        entity: &Entity,
        attr: Attribute,
        id: String,
        value: f64,
        op: Operation,
    ) -> Result<(), AttributeError> {
        let Some(attr_data) = self.attributes.get(&attr) else {
            return Err(AttributeError::AttributeNotFound);
        };

        {
            let modifiers = &mut self.modifiers.lock().await;

            for (mod_id, _, _, _) in modifiers.iter() {
                if mod_id == id {
                    return Err(AttributeError::ModifierAlreadyExists(attr));
                }
            }

            modifiers.commands.push((id, attr, op, value))
        };

        if attr_data.tracked {
            self.broadcast_attribute_update(entity).await;
        }

        Ok(())
    }

    pub async fn remove_modifier(
        &self,
        entity: &Entity,
        attr: Attribute,
        id: &str,
    ) -> Result<(), AttributeError> {
        let Some(attr_data) = self.attributes.get(&attr) else {
            return Err(AttributeError::AttributeNotFound);
        };

        {
            let modifiers = &mut self.modifiers.lock().await;

            let len_before =
                modifiers.commands.len() + modifiers.effects.len() + modifiers.equipment.len();

            modifiers.commands.retain(|(mod_id, _, _, _)| mod_id != id);
            modifiers.effects.retain(|(mod_id, _, _, _)| *mod_id != id);
            modifiers
                .equipment
                .retain(|(mod_id, _, _, _)| *mod_id != id);

            let len_after =
                modifiers.commands.len() + modifiers.effects.len() + modifiers.equipment.len();

            if len_before == len_after {
                return Err(AttributeError::ModifierDoesNotExist);
            }
        };

        if attr_data.tracked {
            self.broadcast_attribute_update(entity).await;
        }

        Ok(())
    }

    pub async fn get_modifier_value(&self, id: &str) -> Result<f64, AttributeError> {
        for (mod_id, _attr, _op, value) in self.modifiers.lock().await.iter() {
            if id == mod_id {
                return Ok(value);
            }
        }

        Err(AttributeError::ModifierDoesNotExist)
    }

    async fn broadcast_attribute_update(&self, entity: &Entity) {
        let attributes = self.attributes.iter().filter(|(_attr, data)| data.tracked);

        let mut modifier_map = HashMap::new();
        for (attr, _) in attributes.clone() {
            modifier_map.insert(*attr, Vec::new());
        }
        {
            for (mod_id, mod_attr, op, value) in self.modifiers.lock().await.iter() {
                let Some(vec) = modifier_map.get_mut(&mod_attr) else {
                    continue;
                };

                vec.push(CUpdateAttributesModifier::new(mod_id.to_owned(), value, op));
            }
        }

        let packet = CUpdateAttributes::new(
            entity.entity_id,
            attributes
                .map(|(attr, data)| {
                    CUpdateAttributesEntry::new(
                        *attr,
                        data.current_base_value.load(),
                        modifier_map.remove(attr).unwrap_or_else(|| Vec::new()),
                    )
                })
                .collect(),
        );

        entity.world.broadcast_packet_all(&packet).await;
    }
}

pub struct AttributeManagerBuilder(Vec<(Attribute, f64, bool)>);

impl AttributeManagerBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub fn add(mut self, attr: Attribute, default_base_value: f64) -> Self {
        let tracked = true; // todo: load from assets
        self.0.push((attr, default_base_value, tracked));
        self
    }

    #[must_use]
    pub fn add_with_fallback_value(self, attr: Attribute) -> Self {
        self.add(attr, attr.get_fallback())
    }

    #[must_use]
    pub fn build(self) -> AttributeManager {
        let mut values = HashMap::with_capacity(self.0.len());

        for (attr, def_val, tracked) in self.0 {
            values.entry(attr).insert_entry(AttributeData {
                default: def_val,
                current_base_value: AtomicCell::new(def_val),
                tracked,
            });
        }

        AttributeManager {
            attributes: values,
            modifiers: Mutex::new(ModifierStorage {
                equipment: Vec::new(),
                effects: Vec::new(),
                commands: Vec::new(),
            }),
        }
    }
}

impl Default for AttributeManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
