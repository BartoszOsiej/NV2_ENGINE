#[allow(dead_code)]
pub struct BlockPalette {
    pub blocks: Vec<BlockType>,
}

#[allow(dead_code)]
pub struct BlockType {
    pub name: String,
    pub texture_id: u32,
    pub is_solid: bool,
}

impl BlockPalette {
    pub fn new_default() -> Self {
        Self {
            blocks: vec![
                BlockType { name: "Air".into(), texture_id: 0, is_solid: false },
                BlockType { name: "Grass".into(), texture_id: 1, is_solid: true },
                BlockType { name: "Dirt".into(), texture_id: 2, is_solid: true },
                BlockType { name: "Stone".into(), texture_id: 3, is_solid: true },
                BlockType { name: "Ignis_Ore".into(), texture_id: 4, is_solid: true },
            ],
        }
    }
}