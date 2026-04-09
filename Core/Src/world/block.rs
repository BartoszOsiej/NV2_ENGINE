use serde::{Deserialize, Serialize};

pub const BLOCK_REGISTRY: &[(u8, &str, &str)] = &[
    (0,  "air",               ""),
    (1,  "grass",             "grass_block_top"),
    (2,  "dirt",              "dirt"),
    (3,  "stone",             "stone"),
    (4,  "sand",              "sand"),
    (5,  "gravel",            "gravel"),
    (6,  "snow",              "snow"),
    (7,  "cobblestone",       "cobblestone"),
    (8,  "bedrock",           "bedrock"),
    (9,  "water",             "water_still"),
    (10, "tree_trunk",        "oak_log"),
    (11, "tree_leaves",       "oak_leaves"),
    (12, "coal_ore",          "coal_ore"),
    (13, "iron_ore",          "iron_ore"),
    (14, "gold_ore",          "gold_ore"),
    (15, "diamond_ore",       "diamond_ore"),
    (16, "emerald_ore",       "emerald_ore"),
    (17, "redstone_ore",      "redstone_ore"),
    (18, "slate_rock",        "deepslate"),
    (19, "slate_coal_ore",    "deepslate_coal_ore"),
    (20, "slate_diamond_ore", "deepslate_diamond_ore"),
    (21, "tuff",              "tuff"),
    (22, "ember_rock",        "netherrack"),
    (23, "glow_rock",         "glowstone"),
    (24, "obsidian",          "obsidian"),
    (25, "stone_bricks",      "stone_bricks"),
    (26, "andesite",          "andesite"),
    (27, "bush",              "bush"),
    (28, "tall_grass",        "short_grass"),
    (29, "flower",            "dandelion"),
    (30, "dead_bush",         "dead_bush"),
    (31, "cactus",            "cactus_side"),
    (32, "clay",              "clay"),
    (33, "moss_mat",          "moss_block"),
    (34, "mud",               "mud"),
    (35, "packed_mud",        "packed_mud"),
    (36, "rooted_soil",       "rooted_dirt"),
    (37, "coarse_soil",       "coarse_dirt"),
    (38, "forest_floor",      "podzol_top"),
    (39, "bloom_floor",       "mycelium_top"),
    (40, "root_lattice",      "muddy_mangrove_roots_top"),
    (41, "needle_wood",       "spruce_log"),
    (42, "warm_wood",         "acacia_log"),
    (43, "wet_wood",          "mangrove_log"),
    (44, "pale_wood",         "birch_log"),
    (45, "needle_canopy",     "spruce_leaves"),
    (46, "warm_canopy",       "acacia_leaves"),
    (47, "wet_canopy",        "mangrove_leaves"),
    (48, "pale_canopy",       "birch_leaves"),
    (49, "bloom_canopy",      "azalea_leaves"),
    (50, "dark_wood",         "dark_oak_log"),
    (51, "dark_canopy",       "dark_oak_leaves"),
    (52, "sapling",           "oak_sapling"),
    (53, "stick",             "oak_log"),
    (54, "flint",             "gravel"),
    (55, "flint_pickaxe",     "stone"),
    (56, "stone_pickaxe",     "cobblestone"),
    (57, "iron_pickaxe",      "iron_ore"),
    (58, "diamond_pickaxe",   "diamond_ore"),
    (59, "netherite_pickaxe", "obsidian"),
    (60, "planks",            "oak_planks"),
    (61, "iron_ingot",        "iron_ore"),
    (62, "chest",             "oak_planks"),
    (63, "nvcrafter",         "stone_bricks"),
    (64, "wooden_pickaxe",    "oak_planks"),
    (65, "wooden_axe",        "oak_planks"),
    (66, "wooden_shovel",     "oak_planks"),
    (67, "wooden_hoe",        "oak_planks"),
    (68, "torch",             "glowstone"),
    (69, "door",              "oak_planks"),
    (70, "trapdoor",          "oak_planks"),
    (71, "ladder",            "oak_planks"),
    (72, "fence",             "oak_planks"),
    (73, "fence_gate",        "oak_planks"),
    (74, "workbench_upgrade", "stone_bricks"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum BlockType {
    Air = 0,
    Grass = 1,
    Dirt = 2,
    Stone = 3,
    Sand = 4,
    Gravel = 5,
    Snow = 6,
    Cobblestone = 7,
    Bedrock = 8,
    Water = 9,
    TreeTrunk = 10,
    TreeLeaves = 11,
    CoalOre = 12,
    IronOre = 13,
    GoldOre = 14,
    DiamondOre = 15,
    EmeraldOre = 16,
    RedstoneOre = 17,
    SlateRock = 18,
    SlateCoalOre = 19,
    SlateDiamondOre = 20,
    Tuff = 21,
    EmberRock = 22,
    GlowRock = 23,
    Obsidian = 24,
    StoneBricks = 25,
    Andesite = 26,
    Bush = 27,
    TallGrass = 28,
    Flower = 29,
    DeadBush = 30,
    Cactus = 31,
    Clay = 32,
    MossMat = 33,
    Mud = 34,
    PackedMud = 35,
    RootedSoil = 36,
    CoarseSoil = 37,
    ForestFloor = 38,
    BloomFloor = 39,
    RootLattice = 40,
    NeedleWood = 41,
    WarmWood = 42,
    WetWood = 43,
    PaleWood = 44,
    NeedleCanopy = 45,
    WarmCanopy = 46,
    WetCanopy = 47,
    PaleCanopy = 48,
    BloomCanopy = 49,
    DarkWood = 50,
    DarkCanopy = 51,
    Sapling = 52,
    Stick = 53,
    Flint = 54,
    FlintPickaxe = 55,
    StonePickaxe = 56,
    IronPickaxe = 57,
    DiamondPickaxe = 58,
    NetheritePickaxe = 59,
    Planks = 60,
    IronIngot = 61,
    Chest = 62,
    NVCrafter = 63,
    WoodenPickaxe = 64,
    WoodenAxe = 65,
    WoodenShovel = 66,
    WoodenHoe = 67,
    Torch = 68,
    Door = 69,
    Trapdoor = 70,
    Ladder = 71,
    Fence = 72,
    FenceGate = 73,
    WorkbenchUpgrade = 74,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum ToolTier {
    Hand = 1,
    Flint = 2,
    Stone = 3,
    Iron = 4,
    Diamond = 5,
    Netherite = 6,
}

impl ToolTier {
    pub fn power(self) -> u8 {
        match self {
            ToolTier::Hand => 1,
            ToolTier::Flint => 2,
            ToolTier::Stone => 3,
            ToolTier::Iron => 5,
            ToolTier::Diamond => 7,
            ToolTier::Netherite => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolStats {
    pub tier: ToolTier,
    pub speed_multiplier: f32,
    pub max_durability: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MovementMediumKind {
    Foliage,
}

#[derive(Clone, Copy, Debug)]
pub struct MovementMedium {
    pub kind: MovementMediumKind,
    pub movement_speed_multiplier: f32,
    pub sprint_speed_multiplier: f32,
    pub fall_speed_multiplier: f32,
    pub sound_dampening: f32,
}

impl MovementMedium {
    pub const FOLIAGE: Self = Self {
        kind: MovementMediumKind::Foliage,
        movement_speed_multiplier: 0.55,
        sprint_speed_multiplier: 0.65,
        fall_speed_multiplier: 0.35,
        sound_dampening: 0.6,
    };
}

impl BlockType {
    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn is_foliage(self) -> bool {
        matches!(
            self,
            BlockType::TreeLeaves
                | BlockType::NeedleCanopy
                | BlockType::WarmCanopy
                | BlockType::WetCanopy
                | BlockType::PaleCanopy
                | BlockType::BloomCanopy
                | BlockType::DarkCanopy
        ) || {
            let name = self.name();
            name.contains("canopy") || name.ends_with("leaves")
        }
    }

    pub fn is_foliage_medium(self) -> bool {
        matches!(
            self,
            BlockType::TreeLeaves
                | BlockType::NeedleCanopy
                | BlockType::WarmCanopy
                | BlockType::WetCanopy
                | BlockType::PaleCanopy
                | BlockType::BloomCanopy
                | BlockType::DarkCanopy
        )
    }

    pub fn hide_on_low_end(self) -> bool {
        self.is_foliage()
            || matches!(
                self,
                BlockType::Bush
                    | BlockType::TallGrass
                    | BlockType::Flower
                    | BlockType::DeadBush
                    | BlockType::RootLattice
                    | BlockType::Sapling
            )
    }

    pub fn movement_medium(self) -> Option<MovementMedium> {
        self.is_foliage_medium().then_some(MovementMedium::FOLIAGE)
    }

    pub fn name(self) -> &'static str {
        BLOCK_REGISTRY
            .get(self as usize)
            .map(|entry| entry.1)
            .unwrap_or("unknown")
    }

    /// Human-readable display name shown in the inventory tooltip.
    pub fn display_name(self) -> &'static str {
        match self {
            BlockType::Air              => "Air",
            BlockType::Grass            => "Grass Block",
            BlockType::Dirt             => "Dirt",
            BlockType::Stone            => "Stone",
            BlockType::Sand             => "Sand",
            BlockType::Gravel           => "Gravel",
            BlockType::Snow             => "Snow",
            BlockType::Cobblestone      => "Cobblestone",
            BlockType::Bedrock          => "Bedrock",
            BlockType::Water            => "Water",
            BlockType::TreeTrunk        => "Oak Log",
            BlockType::TreeLeaves       => "Oak Leaves",
            BlockType::CoalOre          => "Coal Ore",
            BlockType::IronOre          => "Iron Ore",
            BlockType::GoldOre          => "Gold Ore",
            BlockType::DiamondOre       => "Diamond Ore",
            BlockType::EmeraldOre       => "Emerald Ore",
            BlockType::RedstoneOre      => "Redstone Ore",
            BlockType::SlateRock        => "Deepslate",
            BlockType::SlateCoalOre     => "Deepslate Coal Ore",
            BlockType::SlateDiamondOre  => "Deepslate Diamond Ore",
            BlockType::Tuff             => "Tuff",
            BlockType::EmberRock        => "Netherrack",
            BlockType::GlowRock         => "Glowstone",
            BlockType::Obsidian         => "Obsidian",
            BlockType::StoneBricks      => "Stone Bricks",
            BlockType::Andesite         => "Andesite",
            BlockType::Bush             => "Bush",
            BlockType::TallGrass        => "Tall Grass",
            BlockType::Flower           => "Flower",
            BlockType::DeadBush         => "Dead Bush",
            BlockType::Cactus           => "Cactus",
            BlockType::Clay             => "Clay",
            BlockType::MossMat          => "Moss Block",
            BlockType::Mud              => "Mud",
            BlockType::PackedMud        => "Packed Mud",
            BlockType::RootedSoil       => "Rooted Dirt",
            BlockType::CoarseSoil       => "Coarse Dirt",
            BlockType::ForestFloor      => "Podzol",
            BlockType::BloomFloor       => "Mycelium",
            BlockType::RootLattice      => "Mangrove Roots",
            BlockType::NeedleWood       => "Spruce Log",
            BlockType::WarmWood         => "Acacia Log",
            BlockType::WetWood          => "Mangrove Log",
            BlockType::PaleWood         => "Birch Log",
            BlockType::NeedleCanopy     => "Spruce Leaves",
            BlockType::WarmCanopy       => "Acacia Leaves",
            BlockType::WetCanopy        => "Mangrove Leaves",
            BlockType::PaleCanopy       => "Birch Leaves",
            BlockType::BloomCanopy      => "Azalea Leaves",
            BlockType::DarkWood         => "Dark Oak Log",
            BlockType::DarkCanopy       => "Dark Oak Leaves",
            BlockType::Sapling          => "Sapling",
            BlockType::Stick            => "Stick",
            BlockType::Flint            => "Flint",
            BlockType::FlintPickaxe     => "Flint Pickaxe",
            BlockType::StonePickaxe     => "Stone Pickaxe",
            BlockType::IronPickaxe      => "Iron Pickaxe",
            BlockType::DiamondPickaxe   => "Diamond Pickaxe",
            BlockType::NetheritePickaxe => "Netherite Pickaxe",
            BlockType::Planks           => "Oak Planks",
            BlockType::IronIngot        => "Iron Ingot",
            BlockType::Chest            => "Chest",
            BlockType::NVCrafter        => "Crafting Table",
            BlockType::WoodenPickaxe    => "Wooden Pickaxe",
            BlockType::WoodenAxe        => "Wooden Axe",
            BlockType::WoodenShovel     => "Wooden Shovel",
            BlockType::WoodenHoe        => "Wooden Hoe",
            BlockType::Torch            => "Torch",
            BlockType::Door             => "Oak Door",
            BlockType::Trapdoor         => "Oak Trapdoor",
            BlockType::Ladder           => "Ladder",
            BlockType::Fence            => "Oak Fence",
            BlockType::FenceGate        => "Oak Fence Gate",
            BlockType::WorkbenchUpgrade => "Workbench Upgrade",
        }
    }

    pub fn texture_name(self) -> &'static str {
        BLOCK_REGISTRY
            .get(self as usize)
            .map(|entry| entry.2)
            .unwrap_or("")
    }

    pub fn from_name(name: &str) -> Option<Self> {
        BLOCK_REGISTRY
            .iter()
            .find(|(_, block_name, _)| *block_name == name)
            .and_then(|(id, _, _)| Self::from_id(*id))
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
            10 => Some(BlockType::TreeTrunk),
            11 => Some(BlockType::TreeLeaves),
            12 => Some(BlockType::CoalOre),
            13 => Some(BlockType::IronOre),
            14 => Some(BlockType::GoldOre),
            15 => Some(BlockType::DiamondOre),
            16 => Some(BlockType::EmeraldOre),
            17 => Some(BlockType::RedstoneOre),
            18 => Some(BlockType::SlateRock),
            19 => Some(BlockType::SlateCoalOre),
            20 => Some(BlockType::SlateDiamondOre),
            21 => Some(BlockType::Tuff),
            22 => Some(BlockType::EmberRock),
            23 => Some(BlockType::GlowRock),
            24 => Some(BlockType::Obsidian),
            25 => Some(BlockType::StoneBricks),
            26 => Some(BlockType::Andesite),
            27 => Some(BlockType::Bush),
            28 => Some(BlockType::TallGrass),
            29 => Some(BlockType::Flower),
            30 => Some(BlockType::DeadBush),
            31 => Some(BlockType::Cactus),
            32 => Some(BlockType::Clay),
            33 => Some(BlockType::MossMat),
            34 => Some(BlockType::Mud),
            35 => Some(BlockType::PackedMud),
            36 => Some(BlockType::RootedSoil),
            37 => Some(BlockType::CoarseSoil),
            38 => Some(BlockType::ForestFloor),
            39 => Some(BlockType::BloomFloor),
            40 => Some(BlockType::RootLattice),
            41 => Some(BlockType::NeedleWood),
            42 => Some(BlockType::WarmWood),
            43 => Some(BlockType::WetWood),
            44 => Some(BlockType::PaleWood),
            45 => Some(BlockType::NeedleCanopy),
            46 => Some(BlockType::WarmCanopy),
            47 => Some(BlockType::WetCanopy),
            48 => Some(BlockType::PaleCanopy),
            49 => Some(BlockType::BloomCanopy),
            50 => Some(BlockType::DarkWood),
            51 => Some(BlockType::DarkCanopy),
            52 => Some(BlockType::Sapling),
            53 => Some(BlockType::Stick),
            54 => Some(BlockType::Flint),
            55 => Some(BlockType::FlintPickaxe),
            56 => Some(BlockType::StonePickaxe),
            57 => Some(BlockType::IronPickaxe),
            58 => Some(BlockType::DiamondPickaxe),
            59 => Some(BlockType::NetheritePickaxe),
            60 => Some(BlockType::Planks),
            61 => Some(BlockType::IronIngot),
            62 => Some(BlockType::Chest),
            63 => Some(BlockType::NVCrafter),
            64 => Some(BlockType::WoodenPickaxe),
            65 => Some(BlockType::WoodenAxe),
            66 => Some(BlockType::WoodenShovel),
            67 => Some(BlockType::WoodenHoe),
            68 => Some(BlockType::Torch),
            69 => Some(BlockType::Door),
            70 => Some(BlockType::Trapdoor),
            71 => Some(BlockType::Ladder),
            72 => Some(BlockType::Fence),
            73 => Some(BlockType::FenceGate),
            74 => Some(BlockType::WorkbenchUpgrade),
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
        if self.is_foliage() {
            return false;
        }

        !matches!(
            self,
            BlockType::Air
                | BlockType::Water
                | BlockType::Sapling
                | BlockType::Stick
                | BlockType::Flint
                | BlockType::IronIngot
                | BlockType::FlintPickaxe
                | BlockType::StonePickaxe
                | BlockType::IronPickaxe
                | BlockType::DiamondPickaxe
                | BlockType::NetheritePickaxe
                | BlockType::WoodenPickaxe
                | BlockType::WoodenAxe
                | BlockType::WoodenShovel
                | BlockType::WoodenHoe
                | BlockType::Torch
                | BlockType::WorkbenchUpgrade
                | BlockType::Bush
                | BlockType::TallGrass
                | BlockType::Flower
                | BlockType::DeadBush
        )
    }

    pub fn is_solid(self) -> bool {
        if self.is_foliage() {
            return false;
        }

        !matches!(
            self,
            BlockType::Air
                | BlockType::Water
                | BlockType::Sapling
                | BlockType::Stick
                | BlockType::Flint
                | BlockType::IronIngot
                | BlockType::FlintPickaxe
                | BlockType::StonePickaxe
                | BlockType::IronPickaxe
                | BlockType::DiamondPickaxe
                | BlockType::NetheritePickaxe
                | BlockType::WoodenPickaxe
                | BlockType::WoodenAxe
                | BlockType::WoodenShovel
                | BlockType::WoodenHoe
                | BlockType::Torch
                | BlockType::WorkbenchUpgrade
                | BlockType::Bush
                | BlockType::TallGrass
                | BlockType::Flower
                | BlockType::DeadBush
        )
    }

    pub fn is_cube_meshed(self) -> bool {
        self.is_opaque() || self.is_foliage()
    }

    pub fn hardness(self) -> u32 {
        match self {
            BlockType::Air | BlockType::Water => 0,
            BlockType::Bedrock => 18,
            BlockType::Snow
            | BlockType::Bush
            | BlockType::TallGrass
            | BlockType::Flower
            | BlockType::DeadBush
            | BlockType::Sapling
            | BlockType::Stick => 1,
            BlockType::TreeLeaves
            | BlockType::NeedleCanopy
            | BlockType::WarmCanopy
            | BlockType::WetCanopy
            | BlockType::PaleCanopy
            | BlockType::BloomCanopy
            | BlockType::DarkCanopy => 1,
            BlockType::Grass
            | BlockType::Dirt
            | BlockType::Sand
            | BlockType::Gravel
            | BlockType::Clay
            | BlockType::MossMat
            | BlockType::Mud
            | BlockType::PackedMud
            | BlockType::RootedSoil
            | BlockType::CoarseSoil
            | BlockType::ForestFloor
            | BlockType::BloomFloor
            | BlockType::RootLattice
            | BlockType::Cactus
            | BlockType::Planks
            | BlockType::Door
            | BlockType::Trapdoor
            | BlockType::Ladder
            | BlockType::Fence
            | BlockType::FenceGate
            | BlockType::Chest
            | BlockType::Torch
            | BlockType::WorkbenchUpgrade
            | BlockType::Flint => 1,
            BlockType::TreeTrunk
            | BlockType::NeedleWood
            | BlockType::WarmWood
            | BlockType::WetWood
            | BlockType::PaleWood
            | BlockType::DarkWood => 5,
            BlockType::NVCrafter => 3,
            BlockType::IronIngot => 1,
            BlockType::WoodenPickaxe
            | BlockType::WoodenAxe
            | BlockType::WoodenShovel
            | BlockType::WoodenHoe => 1,
            BlockType::Stone
            | BlockType::Cobblestone
            | BlockType::StoneBricks
            | BlockType::Andesite
            | BlockType::SlateRock
            | BlockType::CoalOre
            | BlockType::SlateCoalOre
            | BlockType::Tuff
            | BlockType::EmberRock
            | BlockType::GlowRock => 2,
            BlockType::IronOre | BlockType::GoldOre => 3,
            BlockType::DiamondOre
            | BlockType::EmeraldOre
            | BlockType::RedstoneOre
            | BlockType::SlateDiamondOre => 5,
            BlockType::Obsidian => 7,
            BlockType::FlintPickaxe
            | BlockType::StonePickaxe
            | BlockType::IronPickaxe
            | BlockType::DiamondPickaxe
            | BlockType::NetheritePickaxe => 0,
        }
    }

    pub fn required_tool_tier(self) -> Option<ToolTier> {
        match self {
            BlockType::Stone
            | BlockType::Cobblestone
            | BlockType::StoneBricks
            | BlockType::Andesite
            | BlockType::SlateRock
            | BlockType::CoalOre
            | BlockType::SlateCoalOre
            | BlockType::Tuff => Some(ToolTier::Flint),
            BlockType::IronOre | BlockType::GoldOre => Some(ToolTier::Stone),
            BlockType::DiamondOre
            | BlockType::EmeraldOre
            | BlockType::RedstoneOre
            | BlockType::SlateDiamondOre => Some(ToolTier::Iron),
            BlockType::Obsidian => Some(ToolTier::Diamond),
            _ => None,
        }
    }

    pub fn tool_stats(self) -> Option<ToolStats> {
        match self {
            BlockType::FlintPickaxe => Some(ToolStats {
                tier: ToolTier::Flint,
                speed_multiplier: 1.4,
                max_durability: 24,
            }),
            BlockType::WoodenPickaxe => Some(ToolStats {
                tier: ToolTier::Hand,
                speed_multiplier: 1.1,
                max_durability: 24,
            }),
            BlockType::WoodenAxe => Some(ToolStats {
                tier: ToolTier::Hand,
                speed_multiplier: 1.05,
                max_durability: 24,
            }),
            BlockType::WoodenShovel => Some(ToolStats {
                tier: ToolTier::Hand,
                speed_multiplier: 1.0,
                max_durability: 24,
            }),
            BlockType::WoodenHoe => Some(ToolStats {
                tier: ToolTier::Hand,
                speed_multiplier: 1.0,
                max_durability: 24,
            }),
            BlockType::StonePickaxe => Some(ToolStats {
                tier: ToolTier::Stone,
                speed_multiplier: 1.75,
                max_durability: 48,
            }),
            BlockType::IronPickaxe => Some(ToolStats {
                tier: ToolTier::Iron,
                speed_multiplier: 2.1,
                max_durability: 96,
            }),
            BlockType::DiamondPickaxe => Some(ToolStats {
                tier: ToolTier::Diamond,
                speed_multiplier: 2.55,
                max_durability: 256,
            }),
            BlockType::NetheritePickaxe => Some(ToolStats {
                tier: ToolTier::Netherite,
                speed_multiplier: 2.9,
                max_durability: 512,
            }),
            _ => None,
        }
    }

    pub fn break_time_seconds(self) -> Option<f32> {
        match self {
            BlockType::Air
            | BlockType::Water
            | BlockType::Bedrock
            | BlockType::Sapling
            | BlockType::Stick
            | BlockType::Flint
            | BlockType::IronIngot
            | BlockType::FlintPickaxe
            | BlockType::WoodenPickaxe
            | BlockType::WoodenAxe
            | BlockType::WoodenShovel
            | BlockType::WoodenHoe
            | BlockType::Torch
            | BlockType::StonePickaxe
            | BlockType::IronPickaxe
            | BlockType::DiamondPickaxe
            | BlockType::NetheritePickaxe
            | BlockType::WorkbenchUpgrade => None,
            BlockType::Planks
            | BlockType::Door
            | BlockType::Trapdoor
            | BlockType::Ladder
            | BlockType::Fence
            | BlockType::FenceGate
            | BlockType::Chest => Some(self.hardness() as f32 * 0.10),
            BlockType::NVCrafter => Some(self.hardness() as f32 * 0.12),
            BlockType::TreeLeaves
            | BlockType::NeedleCanopy
            | BlockType::WarmCanopy
            | BlockType::WetCanopy
            | BlockType::PaleCanopy
            | BlockType::BloomCanopy
            | BlockType::DarkCanopy
            | BlockType::Bush
            | BlockType::TallGrass
            | BlockType::Flower
            | BlockType::DeadBush => Some(0.05),
            BlockType::TreeTrunk
            | BlockType::NeedleWood
            | BlockType::WarmWood
            | BlockType::WetWood
            | BlockType::PaleWood
            | BlockType::DarkWood
            | BlockType::Cactus => Some(self.hardness() as f32 * 0.20),
            BlockType::Sand | BlockType::Gravel | BlockType::Snow => Some(self.hardness() as f32 * 0.08),
            BlockType::Grass
            | BlockType::Dirt
            | BlockType::Clay
            | BlockType::MossMat
            | BlockType::Mud
            | BlockType::PackedMud
            | BlockType::RootedSoil
            | BlockType::CoarseSoil
            | BlockType::ForestFloor
            | BlockType::BloomFloor
            | BlockType::RootLattice => Some(self.hardness() as f32 * 0.10),
            _ => {
                let tier_weight = self.required_tool_tier().map_or(0.0, |required| {
                    (required.power().saturating_sub(ToolTier::Hand.power())) as f32 * 0.22
                });
                Some(self.hardness() as f32 * 0.12 + tier_weight)
            }
        }
    }

    pub fn inventory_max_stack(self) -> u32 {
        match self {
            BlockType::FlintPickaxe
            | BlockType::WoodenPickaxe
            | BlockType::WoodenAxe
            | BlockType::WoodenShovel
            | BlockType::WoodenHoe
            | BlockType::StonePickaxe
            | BlockType::IronPickaxe
            | BlockType::DiamondPickaxe
            | BlockType::NetheritePickaxe => 1,
            _ => 64,
        }
    }

    pub fn is_inventory_item(self) -> bool {
        !matches!(self, BlockType::Air | BlockType::Water | BlockType::Bedrock)
    }

    pub fn is_placeable_item(self) -> bool {
        !matches!(
            self,
            BlockType::Air
                | BlockType::Water
                | BlockType::Bedrock
                | BlockType::Sapling
                | BlockType::Stick
                | BlockType::Flint
                | BlockType::IronIngot
                | BlockType::FlintPickaxe
                | BlockType::WoodenPickaxe
                | BlockType::WoodenAxe
                | BlockType::WoodenShovel
                | BlockType::WoodenHoe
                | BlockType::Torch
                | BlockType::StonePickaxe
                | BlockType::IronPickaxe
                | BlockType::DiamondPickaxe
                | BlockType::NetheritePickaxe
                | BlockType::WorkbenchUpgrade
        )
    }

    pub fn has_gui(self) -> bool {
        matches!(self, BlockType::NVCrafter)
    }

    pub fn has_model(self) -> bool {
        let texture_name = self.texture_name();
        !texture_name.is_empty() && crate::assets::BlockModelLoader::get_model(texture_name).is_some()
    }

    pub fn has_registered_texture(self) -> bool {
        let texture_name = self.texture_name();
        !texture_name.is_empty() && crate::renderer::texture_atlas::tile_by_texture_name(texture_name, false).is_some()
    }

    pub fn is_ground_cover_replaceable(self) -> bool {
        matches!(
            self,
            BlockType::Air
                | BlockType::Bush
                | BlockType::TallGrass
                | BlockType::Flower
                | BlockType::DeadBush
        )
    }

    pub fn is_tree_trunk_replaceable(self) -> bool {
        matches!(
            self,
            BlockType::Air
                | BlockType::Bush
                | BlockType::TallGrass
                | BlockType::Flower
                | BlockType::DeadBush
        ) || self.is_foliage()
    }

    pub fn is_tree_canopy_replaceable(self) -> bool {
        matches!(
            self,
            BlockType::Air
                | BlockType::Bush
                | BlockType::TallGrass
                | BlockType::Flower
                | BlockType::DeadBush
        ) || self.is_foliage()
    }

    pub fn face_uvs(&self) -> [crate::renderer::texture_atlas::TileUV; 6] {
        use crate::renderer::texture_atlas::*;

        match self {
            BlockType::Air => [tile_stone(); 6],
            BlockType::Grass => [
                tile_grass_top(),
                tile_dirt(),
                tile_grass_side(),
                tile_grass_side(),
                tile_grass_side(),
                tile_grass_side(),
            ],
            BlockType::Dirt => [tile_dirt(); 6],
            BlockType::Stone => [tile_stone(); 6],
            BlockType::Sand => [tile_sand(); 6],
            BlockType::Gravel => [tile_gravel(); 6],
            BlockType::Snow => [tile_snow(); 6],
            BlockType::Cobblestone => [tile_cobblestone(); 6],
            BlockType::Bedrock => [tile_bedrock(); 6],
            BlockType::Water => [tile_water(); 6],
            BlockType::TreeTrunk => [
                tile_tree_trunk_top(),
                tile_tree_trunk_top(),
                tile_tree_trunk_side(),
                tile_tree_trunk_side(),
                tile_tree_trunk_side(),
                tile_tree_trunk_side(),
            ],
            BlockType::TreeLeaves => [tile_tree_leaves(); 6],
            BlockType::CoalOre => [tile_coal_ore(); 6],
            BlockType::IronOre => [tile_iron_ore(); 6],
            BlockType::GoldOre => [tile_gold_ore(); 6],
            BlockType::DiamondOre => [tile_diamond_ore(); 6],
            BlockType::EmeraldOre => [tile_emerald_ore(); 6],
            BlockType::RedstoneOre => [tile_redstone_ore(); 6],
            BlockType::SlateRock => [tile_slate_rock(); 6],
            BlockType::SlateCoalOre => [tile_slate_coal_ore(); 6],
            BlockType::SlateDiamondOre => [tile_slate_diamond_ore(); 6],
            BlockType::Tuff => [tile_tuff(); 6],
            BlockType::EmberRock => [tile_nether_rock(); 6],
            BlockType::GlowRock => [tile_glow_rock(); 6],
            BlockType::Obsidian => [tile_obsidian(); 6],
            BlockType::StoneBricks => [tile_stone_bricks(); 6],
            BlockType::Andesite => [tile_andesite(); 6],
            BlockType::Bush => [tile_bush(); 6],
            BlockType::TallGrass => [tile_tall_grass(); 6],
            BlockType::Flower => [tile_flower(); 6],
            BlockType::DeadBush => [tile_dead_bush(); 6],
            BlockType::Cactus => [
                tile_cactus_top(),
                tile_cactus_top(),
                tile_cactus_side(),
                tile_cactus_side(),
                tile_cactus_side(),
                tile_cactus_side(),
            ],
            BlockType::Clay => [tile_clay(); 6],
            BlockType::MossMat => [tile_moss_block(); 6],
            BlockType::Mud => [tile_mud(); 6],
            BlockType::PackedMud => [tile_packed_mud(); 6],
            BlockType::RootedSoil => [tile_rooted_dirt(); 6],
            BlockType::CoarseSoil => [tile_coarse_dirt(); 6],
            BlockType::ForestFloor => [
                tile_podzol(),
                tile_dirt(),
                tile_dirt(),
                tile_dirt(),
                tile_dirt(),
                tile_dirt(),
            ],
            BlockType::BloomFloor => [
                tile_mycelium(),
                tile_dirt(),
                tile_dirt(),
                tile_dirt(),
                tile_dirt(),
                tile_dirt(),
            ],
            BlockType::RootLattice => [
                tile_muddy_mangrove_roots(),
                tile_rooted_dirt(),
                tile_rooted_dirt(),
                tile_rooted_dirt(),
                tile_rooted_dirt(),
                tile_rooted_dirt(),
            ],
            BlockType::NeedleWood => [
                tile_tree_trunk_top(),
                tile_tree_trunk_top(),
                tile_spruce_log_side(),
                tile_spruce_log_side(),
                tile_spruce_log_side(),
                tile_spruce_log_side(),
            ],
            BlockType::WarmWood => [
                tile_tree_trunk_top(),
                tile_tree_trunk_top(),
                tile_acacia_log_side(),
                tile_acacia_log_side(),
                tile_acacia_log_side(),
                tile_acacia_log_side(),
            ],
            BlockType::WetWood => [
                tile_tree_trunk_top(),
                tile_tree_trunk_top(),
                tile_mangrove_log_side(),
                tile_mangrove_log_side(),
                tile_mangrove_log_side(),
                tile_mangrove_log_side(),
            ],
            BlockType::PaleWood => [
                tile_tree_trunk_top(),
                tile_tree_trunk_top(),
                tile_birch_log_side(),
                tile_birch_log_side(),
                tile_birch_log_side(),
                tile_birch_log_side(),
            ],
            BlockType::NeedleCanopy => [tile_spruce_leaves(); 6],
            BlockType::WarmCanopy => [tile_acacia_leaves(); 6],
            BlockType::WetCanopy => [tile_mangrove_leaves(); 6],
            BlockType::PaleCanopy => [tile_birch_leaves(); 6],
            BlockType::BloomCanopy => [tile_azalea_leaves(); 6],
            BlockType::DarkWood => [
                tile_tree_trunk_top(),
                tile_tree_trunk_top(),
                tile_dark_oak_log_side(),
                tile_dark_oak_log_side(),
                tile_dark_oak_log_side(),
                tile_dark_oak_log_side(),
            ],
            BlockType::DarkCanopy => [tile_dark_oak_leaves(); 6],
            BlockType::Sapling => [tile_bush(); 6],
            BlockType::Stick => [tile_tree_trunk_side(); 6],
            BlockType::Flint => [tile_gravel(); 6],
            BlockType::FlintPickaxe => [tile_stone(); 6],
            BlockType::StonePickaxe => [tile_cobblestone(); 6],
            BlockType::IronPickaxe => [tile_iron_ore(); 6],
            BlockType::DiamondPickaxe => [tile_diamond_ore(); 6],
            BlockType::NetheritePickaxe => [tile_obsidian(); 6],
            BlockType::Planks => [tile_oak_planks(); 6],
            BlockType::IronIngot => [tile_iron_ore(); 6],
            BlockType::Chest => [tile_oak_planks(); 6],
            BlockType::NVCrafter => [tile_stone_bricks(); 6],
            BlockType::WoodenPickaxe => [tile_oak_planks(); 6],
            BlockType::WoodenAxe => [tile_tree_trunk_side(); 6],
            BlockType::WoodenShovel => [tile_oak_planks(); 6],
            BlockType::WoodenHoe => [tile_oak_planks(); 6],
            BlockType::Torch => [tile_glow_rock(); 6],
            BlockType::Door => [tile_oak_planks(); 6],
            BlockType::Trapdoor => [tile_oak_planks(); 6],
            BlockType::Ladder => [tile_tree_trunk_side(); 6],
            BlockType::Fence => [tile_oak_planks(); 6],
            BlockType::FenceGate => [tile_oak_planks(); 6],
            BlockType::WorkbenchUpgrade => [tile_stone_bricks(); 6],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canopy_blocks_expose_foliage_medium() {
        for block in [
            BlockType::TreeLeaves,
            BlockType::NeedleCanopy,
            BlockType::WetCanopy,
            BlockType::PaleCanopy,
            BlockType::DarkCanopy,
        ] {
            assert!(block.is_foliage_medium());
            assert!(!block.is_solid());

            let medium = block.movement_medium().expect("expected foliage movement medium");
            assert_eq!(medium.kind, MovementMediumKind::Foliage);
            assert_eq!(medium.movement_speed_multiplier, 0.55);
            assert_eq!(medium.sprint_speed_multiplier, 0.65);
            assert_eq!(medium.fall_speed_multiplier, 0.35);
            assert_eq!(medium.sound_dampening, 0.6);
        }
    }

    #[test]
    fn non_foliage_blocks_do_not_expose_medium() {
        assert!(BlockType::Stone.movement_medium().is_none());
        assert!(BlockType::Water.movement_medium().is_none());
        assert!(!BlockType::Stone.is_foliage_medium());
    }

    #[test]
    fn mineral_blocks_report_expected_tool_tiers() {
        assert_eq!(BlockType::Stone.required_tool_tier(), Some(ToolTier::Flint));
        assert_eq!(BlockType::IronOre.required_tool_tier(), Some(ToolTier::Stone));
        assert_eq!(BlockType::DiamondOre.required_tool_tier(), Some(ToolTier::Iron));
        assert_eq!(BlockType::Obsidian.required_tool_tier(), Some(ToolTier::Diamond));
        assert_eq!(BlockType::Gravel.required_tool_tier(), None);
    }

    #[test]
    fn tool_tier_powers_match_hardcore_progression() {
        assert_eq!(ToolTier::Hand.power(), 1);
        assert_eq!(ToolTier::Flint.power(), 2);
        assert_eq!(ToolTier::Stone.power(), 3);
        assert_eq!(ToolTier::Iron.power(), 5);
        assert_eq!(ToolTier::Diamond.power(), 7);
        assert_eq!(ToolTier::Netherite.power(), 8);
    }

    #[test]
    fn log_blocks_require_iron_tier_hardness_and_leaves_stay_soft() {
        for block in [
            BlockType::TreeTrunk,
            BlockType::NeedleWood,
            BlockType::WarmWood,
            BlockType::WetWood,
            BlockType::PaleWood,
            BlockType::DarkWood,
        ] {
            assert_eq!(block.hardness(), 5, "{} should require iron-tier power", block.name());
        }

        for block in [
            BlockType::TreeLeaves,
            BlockType::NeedleCanopy,
            BlockType::WarmCanopy,
            BlockType::WetCanopy,
            BlockType::PaleCanopy,
            BlockType::BloomCanopy,
            BlockType::DarkCanopy,
        ] {
            assert!(block.hardness() <= 1, "{} should remain soft", block.name());
        }
    }

    #[test]
    fn registered_world_blocks_do_not_accidentally_default_to_zero_hardness() {
        for (id, name, _) in BLOCK_REGISTRY {
            let block = BlockType::from_id(*id).expect("registry id should resolve");
            let zero_hardness_is_intended = matches!(
                block,
                BlockType::Air
                    | BlockType::Water
                    | BlockType::Flint
                    | BlockType::FlintPickaxe
                    | BlockType::StonePickaxe
                    | BlockType::IronPickaxe
                    | BlockType::DiamondPickaxe
                    | BlockType::NetheritePickaxe
            );

            if !zero_hardness_is_intended {
                assert!(block.hardness() > 0, "{} unexpectedly has zero hardness", name);
            }
        }

        assert_eq!(BlockType::Stone.hardness(), 2);
        assert_eq!(BlockType::TreeTrunk.hardness(), 5);
        assert_eq!(BlockType::IronOre.hardness(), 3);
        assert_eq!(BlockType::DiamondOre.hardness(), 5);
        assert_eq!(BlockType::Obsidian.hardness(), 7);
    }

    #[test]
    fn tool_items_are_non_placeable_single_stack_items() {
        for block in [
            BlockType::Flint,
            BlockType::FlintPickaxe,
            BlockType::WoodenPickaxe,
            BlockType::WoodenAxe,
            BlockType::WoodenShovel,
            BlockType::WoodenHoe,
            BlockType::StonePickaxe,
            BlockType::IronPickaxe,
            BlockType::DiamondPickaxe,
            BlockType::NetheritePickaxe,
        ] {
            assert!(block.is_inventory_item());
            assert!(!block.is_placeable_item());
            assert!(!block.is_solid());
        }

        for tool in [
            BlockType::FlintPickaxe,
            BlockType::WoodenPickaxe,
            BlockType::WoodenAxe,
            BlockType::WoodenShovel,
            BlockType::WoodenHoe,
            BlockType::StonePickaxe,
            BlockType::IronPickaxe,
            BlockType::DiamondPickaxe,
            BlockType::NetheritePickaxe,
        ] {
            assert_eq!(tool.inventory_max_stack(), 1);
            assert!(tool.tool_stats().is_some());
        }
    }
}
