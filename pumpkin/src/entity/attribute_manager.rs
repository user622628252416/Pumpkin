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
use pumpkin_world::item::ItemStack;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct AttributeNoteFoundError;

struct AttributeValueTuple {
    default: f64,
    current_base_value: AtomicCell<f64>,
}

/// Entities have attributes such as attack damage, scale, armor, etc.
/// This struct keeps track of an entity's base values for each of the applicable attributes and calculates the total attribute value if modifiers (such as those of held items or status effects) are provided.
pub struct AttributeManager {
    values: HashMap<Attribute, AttributeValueTuple>,
}

impl AttributeManager {
    #[must_use]
    pub fn builder() -> AttributeManagerBuilder {
        AttributeManagerBuilder::new()
    }

    /// Reads the base value of `attr` without equipment and effect modifiers.
    pub fn get_base(&self, attr: Attribute) -> Result<f64, AttributeNoteFoundError> {
        self.values
            .get(&attr)
            .map_or(Err(AttributeNoteFoundError), |v| {
                Ok(v.current_base_value.load())
            })
    }

    pub fn set_base(&self, attr: Attribute, value: f64) -> Result<(), AttributeNoteFoundError> {
        let Some(attr_value) = self.values.get(&attr) else {
            return Err(AttributeNoteFoundError);
        };
        attr_value.current_base_value.store(value);
        Ok(())
    }

    pub fn reset_base(&self, attr: Attribute) -> Result<f64, AttributeNoteFoundError> {
        let Some(attr_value) = self.values.get(&attr) else {
            return Err(AttributeNoteFoundError);
        };
        attr_value.current_base_value.store(attr_value.default);
        Ok(attr_value.default)
    }

    /// Reads the base value of `attr` and applies equipment and effect modifiers before returning it.
    ///
    /// `main_hand` is only necessary when main hand is not included in `equipment`, i.e. for player entities.
    pub async fn get_modified(
        &self,
        attr: Attribute,
        equipment: &Mutex<EntityEquipment>,
        main_hand: Option<Arc<Mutex<ItemStack>>>,
        effects: &Mutex<HashMap<&'static StatusEffect, Effect>>,
    ) -> Result<f64, AttributeNoteFoundError> {
        let Some(attr_value) = self.values.get(&attr) else {
            return Err(AttributeNoteFoundError);
        };

        let base = attr_value.current_base_value.load();
        let mut modified = base;

        // item modifiers
        {
            // cloned so lock can be released
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
                    // modifier is for different attribute
                    if *modifier.r#type != attr {
                        continue;
                    }

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

                    // apply modifier
                    match modifier.operation {
                        Operation::AddValue => modified += modifier.amount,
                        Operation::AddMultipliedBase => modified += modifier.amount * base,
                        Operation::AddMultipliedTotal => modified += modifier.amount * modified,
                    };
                }
            }
        }

        // status effect modifiers
        for (status_effect, effect) in effects.lock().await.iter() {
            for modifier in status_effect.attribute_modifiers {
                // modifier is for different attribute
                if *modifier.attribute != attr {
                    continue;
                }

                // apply modifier
                let amount = modifier.base_value * (effect.amplifier as f64 + 1.0);
                match modifier.operation {
                    Operation::AddValue => modified += amount,
                    Operation::AddMultipliedBase => modified += amount * base,
                    Operation::AddMultipliedTotal => modified += amount * modified,
                };
            }
        }

        Ok(modified)
    }
}

pub struct AttributeManagerBuilder(Vec<(Attribute, f64)>);

impl AttributeManagerBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub fn add(mut self, attr: Attribute, default_base_value: f64) -> Self {
        self.0.push((attr, default_base_value));
        self
    }

    #[must_use]
    pub fn add_with_fallback_value(mut self, attr: Attribute) -> Self {
        self.0.push((attr, attr.get_fallback()));
        self
    }

    #[must_use]
    pub fn build(self) -> AttributeManager {
        let mut values = HashMap::with_capacity(self.0.len());

        for (attr, def_val) in self.0 {
            values.entry(attr).insert_entry(AttributeValueTuple {
                default: def_val,
                current_base_value: AtomicCell::new(def_val),
            });
        }

        AttributeManager { values }
    }
}

impl Default for AttributeManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
