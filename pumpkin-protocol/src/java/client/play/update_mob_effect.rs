use pumpkin_data::{packet::clientbound::PLAY_UPDATE_MOB_EFFECT, potion::Effect};
use pumpkin_macros::packet;
use serde::Serialize;

use crate::codec::var_int::VarInt;

#[derive(Serialize)]
#[packet(PLAY_UPDATE_MOB_EFFECT)]
pub struct CUpdateMobEffect {
    pub entity_id: VarInt,
    pub effect_id: VarInt,
    pub amplifier: VarInt,
    pub duration: VarInt,
    pub flags: i8,
}

impl CUpdateMobEffect {
    pub fn new(entity_id: i32, effect: &Effect) -> Self {
        let mut flags: i8 = 0;

        if effect.ambient {
            flags |= 1;
        }
        if effect.show_particles {
            flags |= 2;
        }
        if effect.show_icon {
            flags |= 4;
        }
        if effect.blend {
            flags |= 8;
        }

        Self {
            entity_id: entity_id.into(),
            effect_id: (effect.effect_type.id as i32).into(),
            amplifier: (effect.amplifier as i32).into(),
            duration: effect.duration.into(),
            flags,
        }
    }
}
