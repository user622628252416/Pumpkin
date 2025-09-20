use pumpkin_data::packet::clientbound::PLAY_UPDATE_ATTRIBUTES;
use pumpkin_macros::packet;
use serde::Serialize;

#[derive(Serialize)]
#[packet(PLAY_UPDATE_ATTRIBUTES)]
pub struct CUpdateAttributes {}

impl CUpdateAttributes {
    pub fn new() -> Self {
        Self {}
    }
}
