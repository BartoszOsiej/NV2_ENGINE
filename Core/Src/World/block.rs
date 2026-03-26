use crate::renderer::texture_atlas::{
    TileUV,
    tile_grass_top, tile_grass_side, tile_dirt, tile_stone,
    tile_sand, tile_gravel, tile_snow_top, tile_snow_side,
    tile_water,
    tile_coal_ore, tile_gold_ore, tile_diamond_ore,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum BlockType {
    Air = 0,
    Grass,
    Dirt,
    Stone,
    Sand,
    Gravel,
    SnowGrass,
    Snow,
    Water,
    CoalOre,
    GoldOre,
    DiamondOre,
}

impl BlockType {
    pub fn is_opaque(self) -> bool {
        !matches!(self, BlockType::Air | BlockType::Water)
    }

    /// Returns a TileUV per face.
    /// Face order: Top, Bottom, Front, Back, Right, Left
    pub fn face_uvs(self) -> [TileUV; 6] {
        match self {
            BlockType::Grass => [
                tile_grass_top(),   // top
                tile_dirt(),        // bottom
                tile_grass_side(),  // front
                tile_grass_side(),  // back
                tile_grass_side(),  // right
                tile_grass_side(),  // left
            ],
            BlockType::Dirt     => [tile_dirt();        6],
            BlockType::Stone    => [tile_stone();       6],
            BlockType::Sand     => [tile_sand();        6],
            BlockType::Gravel   => [tile_gravel();      6],
            BlockType::Snow     => [tile_snow_top();    6],
            BlockType::SnowGrass => [
                tile_snow_top(),
                tile_dirt(),
                tile_snow_side(),
                tile_snow_side(),
                tile_snow_side(),
                tile_snow_side(),
            ],
            BlockType::CoalOre    => [tile_coal_ore();    6],
            BlockType::GoldOre    => [tile_gold_ore();    6],
            BlockType::DiamondOre => [tile_diamond_ore(); 6],
            // Water and Air should never be meshed but need a value
            BlockType::Water => [tile_water(); 6],
            BlockType::Air => [tile_dirt(); 6],
        }
    }
}