use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

pub static BLOCKS: LazyLock<TopLevel> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../assets/blocks.json"))
        .expect("Could not parse blocks.json registry.")
});

static BLOCKS_BY_ID: LazyLock<Vec<Block>> = LazyLock::new(|| {
    let mut vec = BLOCKS.blocks.clone();
    vec.sort_by_key(|b| b.id);
    vec
});

static BLOCK_ID_BY_REGISTRY_ID: LazyLock<HashMap<String, u16>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for block in &BLOCKS.blocks {
        map.insert(block.name.clone(), block.id);
    }
    map
});

static BLOCK_ID_BY_STATE_ID: LazyLock<Vec<u16>> = LazyLock::new(|| {
    let mut states = Vec::new();
    for block in &BLOCKS.blocks {
        for state in &block.states {
            states.push((block.id, state.id))
        }
    }

    states.sort_by_key(|(_, s_id)| *s_id);

    states.iter().map(|(b_id, _)| *b_id).collect()
});

static STATE_INDEX_BY_STATE_ID: LazyLock<Vec<usize>> = LazyLock::new(|| {
    let mut states = Vec::new();
    for block in &BLOCKS.blocks {
        for (i, state) in block.states.iter().enumerate() {
            states.push((i, state.id))
        }
    }

    states.sort_by_key(|(_, s_id)| *s_id);

    states.iter().map(|(i, _)| *i).collect()
});

static BLOCK_ID_BY_ITEM_ID: LazyLock<HashMap<u16, u16>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for block in &BLOCKS.blocks {
        map.insert(block.item_id, block.id);
    }
    map
});

pub fn get_block(registry_id: &str) -> Option<&Block> {
    let id = *BLOCK_ID_BY_REGISTRY_ID.get(registry_id)?;
    BLOCKS_BY_ID.get(id as usize)
}

pub fn get_block_by_id<'a>(id: u16) -> Option<&'a Block> {
    BLOCKS_BY_ID.get(id as usize)
}

pub fn get_state_by_state_id<'a>(id: u16) -> Option<&'a State> {
    get_block_and_state_by_state_id(id).map(|(_, state)| state)
}

pub fn get_block_by_state_id<'a>(id: u16) -> Option<&'a Block> {
    let block_id = *BLOCK_ID_BY_STATE_ID.get(id as usize)?;
    BLOCKS_BY_ID.get(block_id as usize)
}

pub fn get_block_and_state_by_state_id<'a>(id: u16) -> Option<(&'a Block, &'a State)> {
    let block = get_block_by_state_id(id)?;
    let state_index = *STATE_INDEX_BY_STATE_ID.get(id as usize)?;
    let state = block.states.get(state_index)?;
    Some((block, state))
}

pub fn get_block_by_item<'a>(item_id: u16) -> Option<&'a Block> {
    let block_id = *BLOCK_ID_BY_ITEM_ID.get(&item_id)?;
    BLOCKS_BY_ID.get(block_id as usize)
}
#[expect(dead_code)]
#[derive(Deserialize, Clone, Debug)]
pub struct TopLevel {
    pub blocks: Vec<Block>,
    shapes: Vec<Shape>,
    block_entity_types: Vec<BlockEntityKind>,
}
#[derive(Deserialize, Clone, Debug)]
pub struct Block {
    pub id: u16,
    pub item_id: u16,
    pub hardness: f32,
    pub wall_variant_id: Option<u16>,
    pub translation_key: String,
    pub name: String,
    pub properties: Vec<Property>,
    pub default_state_id: u16,
    pub states: Vec<State>,
}
#[expect(dead_code)]
#[derive(Deserialize, Clone, Debug)]
struct BlockEntityKind {
    id: u32,
    ident: String,
    name: String,
}
#[expect(dead_code)]
#[derive(Deserialize, Clone, Debug)]
pub struct Property {
    name: String,
    values: Vec<String>,
}
#[derive(Deserialize, Clone, Debug)]
pub struct State {
    pub id: u16,
    pub air: bool,
    pub luminance: u8,
    pub burnable: bool,
    pub opacity: Option<u32>,
    pub replaceable: bool,
    pub collision_shapes: Vec<u16>,
    pub block_entity_type: Option<u32>,
}
#[expect(dead_code)]
#[derive(Deserialize, Clone, Debug)]
struct Shape {
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
}
