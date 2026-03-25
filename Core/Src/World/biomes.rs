/// Every block type in the game.
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
}

impl BlockType {
    pub fn is_opaque(self) -> bool {
        !matches!(self, BlockType::Air | BlockType::Water)
    }

    /// Returns (atlas_col, atlas_row) for each face.
    /// Face order: Top, Bottom, Front, Back, Left, Right
    pub fn face_uvs(self) -> [(u32, u32); 6] {
        match self {
            BlockType::Grass => [
                (0, 0), // Top  → grass top
                (1, 0), // Bot  → dirt
                (2, 0), // Front→ grass side
                (2, 0),
                (2, 0),
                (2, 0),
            ],
            BlockType::Dirt => [(1, 0); 6],
            BlockType::Stone => [(3, 0); 6],
            BlockType::Sand => [(0, 1); 6],
            BlockType::Gravel => [(1, 1); 6],
            BlockType::SnowGrass => [
                (0, 2), // top  → snow
                (1, 0), // bot  → dirt
                (3, 1), // side → snowy grass side
                (3, 1),
                (3, 1),
                (3, 1),
            ],
            BlockType::Snow => [(0, 2); 6],
            BlockType::Water => [(2, 1); 6],
            BlockType::Air => [(0, 0); 6],
        }
    }
}