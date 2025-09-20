use pumpkin_data::{
    attributes::Attribute, data_component_impl::Operation,
    packet::clientbound::PLAY_UPDATE_ATTRIBUTES,
};
use pumpkin_macros::packet;
use serde::Serialize;

use crate::codec::var_int::VarInt;

#[derive(Serialize)]
#[packet(PLAY_UPDATE_ATTRIBUTES)]
pub struct CUpdateAttributes {
    pub entity_id: VarInt,
    pub entries: Vec<CUpdateAttributesEntry>,
}

#[derive(Serialize)]
pub struct CUpdateAttributesEntry {
    pub attribute: VarInt,
    pub base_value: f64,
    pub modifiers: Vec<CUpdateAttributesModifier>,
}

#[derive(Serialize)]
pub struct CUpdateAttributesModifier {
    pub id: String,
    pub value: f64,
    pub operation: VarInt,
}

impl CUpdateAttributes {
    pub fn new(entity_id: i32, entries: Vec<CUpdateAttributesEntry>) -> Self {
        Self {
            entity_id: entity_id.into(),
            entries,
        }
    }
}

impl CUpdateAttributesEntry {
    pub fn new(
        attribute: Attribute,
        base_value: f64,
        modifiers: Vec<CUpdateAttributesModifier>,
    ) -> Self {
        Self {
            attribute: attribute.get_raw_id().into(),
            base_value,
            modifiers,
        }
    }
}

impl CUpdateAttributesModifier {
    pub fn new(id: String, value: f64, operation: Operation) -> Self {
        Self {
            id,
            value,
            operation: match operation {
                Operation::AddValue => 0,
                Operation::AddMultipliedBase => 1,
                Operation::AddMultipliedTotal => 2,
            }
            .into(),
        }
    }
}
