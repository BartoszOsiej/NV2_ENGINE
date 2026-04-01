use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum BlockType {
    Air = 0,
    Grass,
    Dirt,
    Stone,
    Sand,
    Gravel,
    Snow,
    Cobblestone,
    Bedrock,
    Water,
    OakLog,
    OakLeaves,
    CoalOre,
    IronOre,
    GoldOre,
    DiamondOre,
    EmeraldOre,
    RedstoneOre,
    Deepslate,
    DeepslateCoalOre,
    DeepslateDiamondOre,
    Tuff,
    Netherrack,
    Glowstone,
    Obsidian,
    StoneBricks,
    Andesite,
}

impl BlockType {
    pub const AIR_ID: u8 = 0;
    pub const GRASS_ID: u8 = 1;
    pub const DIRT_ID: u8 = 2;
    pub const STONE_ID: u8 = 3;
    pub const SAND_ID: u8 = 4;
    pub const GRAVEL_ID: u8 = 5;
    pub const SNOW_ID: u8 = 6;
    pub const COBBLESTONE_ID: u8 = 7;
    pub const BEDROCK_ID: u8 = 8;
    pub const WATER_ID: u8 = 9;
    pub const OAK_LOG_ID: u8 = 10;
    pub const OAK_LEAVES_ID: u8 = 11;
    pub const COAL_ORE_ID: u8 = 12;
    pub const IRON_ORE_ID: u8 = 13;
    pub const GOLD_ORE_ID: u8 = 14;
    pub const DIAMOND_ORE_ID: u8 = 15;
    pub const EMERALD_ORE_ID: u8 = 16;
    pub const REDSTONE_ORE_ID: u8 = 17;
    pub const DEEPSLATE_ID: u8 = 18;
    pub const DEEPSLATE_COAL_ORE_ID: u8 = 19;
    pub const DEEPSLATE_DIAMOND_ORE_ID: u8 = 20;
    pub const TUFF_ID: u8 = 21;
    pub const NETHERRACK_ID: u8 = 22;
    pub const GLOWSTONE_ID: u8 = 23;
    pub const OBSIDIAN_ID: u8 = 24;
    pub const STONE_BRICKS_ID: u8 = 25;
    pub const ANDESITE_ID: u8 = 26;

    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(BlockType::Air),
            1 => Some(BlockType::Grass),
            2 => Some(BlockType::Dirt),
            3 => Some(BlockType::Stone),
            4 => Some(BlockType::Sand),
            5 => Some(BlockType::Gravel),
            6 => Some(BlockType::Snow),
            7 => Some(BlockType::Cobblestone),
            8 => Some(BlockType::Bedrock),
            9 => Some(BlockType::Water),
            10 => Some(BlockType::OakLog),
            11 => Some(BlockType::OakLeaves),
            12 => Some(BlockType::CoalOre),
            13 => Some(BlockType::IronOre),
            14 => Some(BlockType::GoldOre),
            15 => Some(BlockType::DiamondOre),
            16 => Some(BlockType::EmeraldOre),
            17 => Some(BlockType::RedstoneOre),
            18 => Some(BlockType::Deepslate),
            19 => Some(BlockType::DeepslateCoalOre),
            20 => Some(BlockType::DeepslateDiamondOre),
            21 => Some(BlockType::Tuff),
            22 => Some(BlockType::Netherrack),
            23 => Some(BlockType::Glowstone),
            24 => Some(BlockType::Obsidian),
            25 => Some(BlockType::StoneBricks),
            26 => Some(BlockType::Andesite),
            _ => None,
        }
    }

    pub fn from_id_or_air(id: u8) -> Self {
        Self::from_id(id).unwrap_or(BlockType::Air)
    }

    pub fn from_id_or_default(id: u8, default: BlockType) -> BlockType {
        Self::from_id(id).unwrap_or(default)
    }

    pub fn is_opaque(self) -> bool {
        !matches!(self, BlockType::Air | BlockType::Water | BlockType::OakLeaves)
    }

    pub fn is_solid(self) -> bool {
        !matches!(self, BlockType::Air | BlockType::Water)
    }

    pub fn face_uvs(&self) -> [crate::renderer::texture_atlas::TileUV; 6] {
        use crate::renderer::texture_atlas::*;
        match self {
            BlockType::Air => [tile_stone(); 6], // dummy - should not render
            BlockType::Grass => [tile_grass_top(), tile_dirt(), tile_grass_side(), tile_grass_side(), tile_grass_side(), tile_grass_side()],
            BlockType::Dirt => [tile_dirt(); 6],
            BlockType::Stone => [tile_stone(); 6],
            BlockType::Sand => [tile_sand(); 6],
            BlockType::Gravel => [tile_gravel(); 6],
            BlockType::Snow => [tile_snow(); 6],
            BlockType::Cobblestone => [tile_cobblestone(); 6],
            BlockType::Bedrock => [tile_bedrock(); 6],
            BlockType::Water => [tile_water(); 6],
            BlockType::OakLog => [tile_oak_log_top(), tile_oak_log_top(), tile_oak_log_side(), tile_oak_log_side(), tile_oak_log_side(), tile_oak_log_side()],
            BlockType::OakLeaves => [tile_oak_leaves(); 6],
            BlockType::CoalOre => [tile_coal_ore(); 6],
            BlockType::IronOre => [tile_iron_ore(); 6],
            BlockType::GoldOre => [tile_gold_ore(); 6],
            BlockType::DiamondOre => [tile_diamond_ore(); 6],
            BlockType::EmeraldOre => [tile_emerald_ore(); 6],
            BlockType::RedstoneOre => [tile_redstone_ore(); 6],
            BlockType::Deepslate => [tile_deepslate(); 6],
            BlockType::DeepslateCoalOre => [tile_deepslate_coal_ore(); 6],
            BlockType::DeepslateDiamondOre => [tile_deepslate_diamond(); 6],
            BlockType::Tuff => [tile_tuff(); 6],
            BlockType::Netherrack => [tile_netherrack(); 6],
            BlockType::Glowstone => [tile_glowstone(); 6],
            BlockType::Obsidian => [tile_obsidian(); 6],
            BlockType::StoneBricks => [tile_stone_bricks(); 6],
            BlockType::Andesite => [tile_andesite(); 6],
        }
    }
}