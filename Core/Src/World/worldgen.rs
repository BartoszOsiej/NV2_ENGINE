use super::block::BlockType;
use super::chunk::{CHUNK_D, CHUNK_H, CHUNK_W};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockWriteRule {
    GroundCover,
    TreeTrunk,
    TreeCanopy,
}

impl BlockWriteRule {
    pub fn allows(self, current: BlockType) -> bool {
        match self {
            BlockWriteRule::GroundCover => current.is_ground_cover_replaceable(),
            BlockWriteRule::TreeTrunk => current.is_tree_trunk_replaceable(),
            BlockWriteRule::TreeCanopy => current.is_tree_canopy_replaceable(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldBlockWrite {
    pub wx: i32,
    pub wy: i32,
    pub wz: i32,
    pub block: BlockType,
    pub rule: BlockWriteRule,
}

pub struct WorldGenWriter<'a> {
    origin_cx: i32,
    origin_cz: i32,
    origin_blocks: &'a mut Box<[[[BlockType; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
    writes: Vec<WorldBlockWrite>,
}

impl<'a> WorldGenWriter<'a> {
    pub fn new(
        origin_cx: i32,
        origin_cz: i32,
        origin_blocks: &'a mut Box<[[[BlockType; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
    ) -> Self {
        Self {
            origin_cx,
            origin_cz,
            origin_blocks,
            writes: Vec::new(),
        }
    }

    pub fn set_block(&mut self, wx: i32, wy: i32, wz: i32, block: BlockType, rule: BlockWriteRule) {
        if wy <= 0 || wy >= CHUNK_H as i32 {
            return;
        }

        let cx = wx.div_euclid(CHUNK_W as i32);
        let cz = wz.div_euclid(CHUNK_D as i32);

        if (cx, cz) == (self.origin_cx, self.origin_cz) {
            let lx = wx.rem_euclid(CHUNK_W as i32) as usize;
            let lz = wz.rem_euclid(CHUNK_D as i32) as usize;
            let current = self.origin_blocks[lx][wy as usize][lz];
            if rule.allows(current) {
                self.origin_blocks[lx][wy as usize][lz] = block;
            }
            return;
        }

        self.writes.push(WorldBlockWrite {
            wx,
            wy,
            wz,
            block,
            rule,
        });
    }

    pub fn finish(self) -> Vec<WorldBlockWrite> {
        self.writes
    }
}