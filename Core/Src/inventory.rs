use serde::{Deserialize, Serialize};

use crate::{
    crafting::{CraftingGrid, RecipeRegistry},
    world::{block::ToolTier, BlockType},
};

pub const INVENTORY_SLOT_COUNT: usize = 36;
pub const HOTBAR_SLOT_COUNT: usize = 9;
pub const HOTBAR_START: usize = INVENTORY_SLOT_COUNT - HOTBAR_SLOT_COUNT;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: String,
    pub count: u32,
    pub max_stack: u32,
    pub block_type: Option<BlockType>,
    pub durability: Option<u32>,
    pub max_durability: Option<u32>,
}

impl ItemStack {
    pub fn from_block(block: BlockType) -> Option<Self> {
        block
            .is_placeable_item()
            .then(|| Self::from_inventory_item(block))
            .flatten()
    }

    pub fn from_inventory_item(block: BlockType) -> Option<Self> {
        block.is_inventory_item().then(|| {
            let mut stack = Self {
                id: block.name().to_string(),
                count: 1,
                max_stack: block.inventory_max_stack(),
                block_type: Some(block),
                durability: None,
                max_durability: None,
            };
            stack.normalize_in_place();
            stack
        })
    }

    fn normalize_in_place(&mut self) {
        if let Some(block) = self.block_type {
            self.id = block.name().to_string();
            self.max_stack = block.inventory_max_stack().max(1);
            if let Some(tool) = block.tool_stats() {
                self.count = 1;
                self.max_stack = 1;
                self.max_durability = Some(tool.max_durability);
                self.durability = Some(
                    self.durability
                        .unwrap_or(tool.max_durability)
                        .min(tool.max_durability)
                        .max(1),
                );
            } else {
                self.durability = None;
                self.max_durability = None;
            }
        } else {
            self.max_stack = self.max_stack.max(1);
            if self.max_stack > 1 {
                self.durability = None;
                self.max_durability = None;
            }
        }

        if self.count > self.max_stack {
            self.count = self.max_stack;
        }
    }

    fn normalized(mut self) -> Self {
        self.normalize_in_place();
        self
    }

    pub fn can_stack_with(&self, other: &Self) -> bool {
        if self.durability.is_some() || other.durability.is_some() {
            return false;
        }

        match (self.block_type, other.block_type) {
            (Some(left), Some(right)) => left == right,
            (None, None) => self.id == other.id,
            _ => false,
        }
    }

    pub fn tool_power(&self) -> u8 {
        self.block_type
            .and_then(|block| block.tool_stats().map(|stats| stats.tier.power()))
            .unwrap_or(ToolTier::Hand.power())
    }
}

#[derive(Clone, Debug)]
pub struct Inventory {
    slots: Vec<Option<ItemStack>>,
    active_hotbar_slot: usize,
    pub crafting_grid: CraftingGrid,
    pub crafting_output: Option<ItemStack>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            slots: vec![None; INVENTORY_SLOT_COUNT],
            active_hotbar_slot: 0,
            crafting_grid: CraftingGrid::new(2, 2),
            crafting_output: None,
        }
    }

    pub fn slots(&self) -> &[Option<ItemStack>] {
        &self.slots
    }

    pub fn slot(&self, index: usize) -> Option<&ItemStack> {
        self.slots.get(index).and_then(|slot| slot.as_ref())
    }

    pub fn set_slot(&mut self, index: usize, stack: Option<ItemStack>) {
        if let Some(slot) = self.slots.get_mut(index) {
            *slot = stack.map(ItemStack::normalized);
        }
    }

    pub fn take_slot(&mut self, index: usize) -> Option<ItemStack> {
        self.slots.get_mut(index).and_then(Option::take)
    }

    pub fn active_hotbar_slot(&self) -> usize {
        self.active_hotbar_slot
    }

    pub fn active_slot_index(&self) -> usize {
        HOTBAR_START + self.active_hotbar_slot
    }

    pub fn active_stack(&self) -> Option<&ItemStack> {
        self.slot(self.active_slot_index())
    }

    pub fn active_stack_mut(&mut self) -> Option<&mut ItemStack> {
        let active_index = self.active_slot_index();
        self.slots.get_mut(active_index).and_then(|slot| slot.as_mut())
    }

    pub fn active_tool_power(&self) -> u8 {
        self.active_stack()
            .map(ItemStack::tool_power)
            .unwrap_or(ToolTier::Hand.power())
    }

    pub fn crafting_slot(&self, index: usize) -> &Option<ItemStack> {
        self.crafting_grid.get_slot(index)
    }

    pub fn set_crafting_slot(
        &mut self,
        index: usize,
        stack: Option<ItemStack>,
        recipes: &RecipeRegistry,
    ) {
        self.crafting_grid.set_slot(index, stack.map(ItemStack::normalized));
        self.update_crafting_output(recipes);
    }

    pub fn take_crafting_slot(
        &mut self,
        index: usize,
        recipes: &RecipeRegistry,
    ) -> Option<ItemStack> {
        let stack = self.crafting_grid.get_slot(index).clone();
        self.crafting_grid.set_slot(index, None);
        self.update_crafting_output(recipes);
        stack
    }

    pub fn can_accept_item(&self, stack: &ItemStack) -> bool {
        let mut preview = self.clone();
        preview.add_item(stack.clone()).is_none()
    }

    pub fn update_crafting_output(&mut self, recipes: &RecipeRegistry) {
        self.crafting_output = recipes.match_grid(&self.crafting_grid);
    }

    pub fn take_crafting_output(&mut self, recipes: &RecipeRegistry) -> Option<ItemStack> {
        let result = recipes.match_grid(&self.crafting_grid)?;

        for idx in 0..self.crafting_grid.active_len() {
            let Some(stack) = self.crafting_grid.slots[idx].as_mut() else {
                continue;
            };

            if stack.count > 1 {
                stack.count -= 1;
            } else {
                self.crafting_grid.slots[idx] = None;
            }
        }

        self.update_crafting_output(recipes);
        Some(result)
    }

    pub fn set_active_hotbar_slot(&mut self, slot: usize) {
        self.active_hotbar_slot = slot.min(HOTBAR_SLOT_COUNT - 1);
    }

    pub fn scroll_hotbar(&mut self, delta: i32) {
        if delta == 0 {
            return;
        }

        self.active_hotbar_slot =
            (self.active_hotbar_slot as i32 - delta).rem_euclid(HOTBAR_SLOT_COUNT as i32) as usize;
    }

    pub fn add_block(&mut self, block: BlockType) -> bool {
        ItemStack::from_block(block)
            .map(|stack| self.add_item(stack).is_none())
            .unwrap_or(false)
    }

    pub fn add_item(&mut self, mut stack: ItemStack) -> Option<ItemStack> {
        stack.normalize_in_place();
        if stack.count == 0 {
            return None;
        }

        for slot in &mut self.slots {
            let Some(existing) = slot.as_mut() else { continue; };
            existing.normalize_in_place();
            if !existing.can_stack_with(&stack) || existing.count >= existing.max_stack {
                continue;
            }

            let transfer = existing.max_stack.saturating_sub(existing.count).min(stack.count);
            if transfer == 0 {
                continue;
            }

            existing.count += transfer;
            stack.count -= transfer;
            if stack.count == 0 {
                return None;
            }
        }

        for slot in &mut self.slots {
            if slot.is_some() {
                continue;
            }

            let transfer = stack.max_stack.min(stack.count);
            let mut placed = stack.clone();
            placed.count = transfer;
            *slot = Some(placed);
            stack.count -= transfer;
            if stack.count == 0 {
                return None;
            }
        }

        Some(stack)
    }

    pub fn consume_active_item(&mut self, amount: u32) -> bool {
        let active_index = self.active_slot_index();
        let Some(slot) = self.slots.get_mut(active_index) else {
            return false;
        };
        let Some(stack) = slot.as_mut() else {
            return false;
        };
        if stack.count < amount {
            return false;
        }

        stack.count -= amount;
        if stack.count == 0 {
            *slot = None;
        }
        true
    }

    pub fn damage_active_tool(&mut self, amount: u32) -> bool {
        if amount == 0 {
            return false;
        }

        let active_index = self.active_slot_index();
        let Some(slot) = self.slots.get_mut(active_index) else {
            return false;
        };
        let Some(stack) = slot.as_mut() else {
            return false;
        };
        let Some(durability) = stack.durability else {
            return false;
        };

        let remaining = durability.saturating_sub(amount);
        if remaining == 0 {
            *slot = None;
        } else {
            stack.durability = Some(remaining);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::crafting::RecipeRegistry;

    #[test]
    fn inventory_stacks_identical_items_before_using_empty_slots() {
        let mut inventory = Inventory::new();
        inventory.set_slot(
            0,
            Some(ItemStack {
                id: "stone".to_string(),
                count: 63,
                max_stack: 64,
                block_type: Some(BlockType::Stone),
                durability: None,
                max_durability: None,
            }),
        );

        let leftover = inventory.add_item(ItemStack {
            id: "stone".to_string(),
            count: 2,
            max_stack: 64,
            block_type: Some(BlockType::Stone),
            durability: None,
            max_durability: None,
        });

        assert!(leftover.is_none());
        assert_eq!(inventory.slot(0).unwrap().count, 64);
        assert_eq!(inventory.slot(1).unwrap().count, 1);
    }

    #[test]
    fn inventory_merges_non_placeable_items_into_existing_slot() {
        let mut inventory = Inventory::new();

        for _ in 0..20 {
            let leftover = inventory.add_item(ItemStack {
                id: "temporary_stick_id".to_string(),
                count: 1,
                max_stack: 1,
                block_type: Some(BlockType::Stick),
                durability: None,
                max_durability: None,
            });

            assert!(leftover.is_none());
        }

        assert_eq!(inventory.slot(0).unwrap().count, 20);
        assert_eq!(inventory.slot(0).unwrap().id, BlockType::Stick.name());
        assert_eq!(inventory.slot(0).unwrap().max_stack, 64);
        assert!(inventory.slot(1).is_none());
    }

    #[test]
    fn inventory_fills_compatible_stacks_before_using_empty_slots() {
        let mut inventory = Inventory::new();
        inventory.set_slot(
            0,
            Some(ItemStack {
                id: "tree_trunk".to_string(),
                count: 63,
                max_stack: 64,
                block_type: Some(BlockType::TreeTrunk),
                durability: None,
                max_durability: None,
            }),
        );
        inventory.set_slot(
            HOTBAR_START,
            Some(ItemStack {
                id: "tree_trunk".to_string(),
                count: 62,
                max_stack: 64,
                block_type: Some(BlockType::TreeTrunk),
                durability: None,
                max_durability: None,
            }),
        );

        let leftover = inventory.add_item(ItemStack {
            id: "wrong_tree_trunk_id".to_string(),
            count: 3,
            max_stack: 16,
            block_type: Some(BlockType::TreeTrunk),
            durability: None,
            max_durability: None,
        });

        assert!(leftover.is_none());
        assert_eq!(inventory.slot(0).unwrap().count, 64);
        assert_eq!(inventory.slot(HOTBAR_START).unwrap().count, 64);
        assert!(inventory.slot(1).is_none());
    }

    #[test]
    fn saplings_stack_using_same_inventory_logic() {
        let mut inventory = Inventory::new();
        inventory.set_slot(
            HOTBAR_START,
            Some(ItemStack {
                id: "sapling".to_string(),
                count: 63,
                max_stack: 64,
                block_type: Some(BlockType::Sapling),
                durability: None,
                max_durability: None,
            }),
        );

        let leftover = inventory.add_item(ItemStack {
            id: "placeholder".to_string(),
            count: 2,
            max_stack: 1,
            block_type: Some(BlockType::Sapling),
            durability: None,
            max_durability: None,
        });

        assert!(leftover.is_none());
        assert_eq!(inventory.slot(HOTBAR_START).unwrap().count, 64);
        assert_eq!(inventory.slot(0).unwrap().count, 1);
    }

    #[test]
    fn hotbar_selection_wraps_with_scroll() {
        let mut inventory = Inventory::new();
        inventory.set_active_hotbar_slot(0);
        inventory.scroll_hotbar(1);
        assert_eq!(inventory.active_hotbar_slot(), HOTBAR_SLOT_COUNT - 1);

        inventory.scroll_hotbar(-1);
        assert_eq!(inventory.active_hotbar_slot(), 0);
    }

    #[test]
    fn tool_items_normalize_to_single_stack_with_durability() {
        let tool = ItemStack::from_inventory_item(BlockType::StonePickaxe).expect("tool should be inventory item");

        assert_eq!(tool.count, 1);
        assert_eq!(tool.max_stack, 1);
        assert_eq!(tool.durability, Some(48));
        assert_eq!(tool.max_durability, Some(48));
    }

    #[test]
    fn tool_items_never_stack_into_existing_tool_slots() {
        let mut inventory = Inventory::new();
        inventory.set_slot(0, ItemStack::from_inventory_item(BlockType::StonePickaxe));

        let leftover = inventory.add_item(
            ItemStack::from_inventory_item(BlockType::StonePickaxe)
                .expect("tool should be inventory item"),
        );

        assert!(leftover.is_none());
        assert_eq!(inventory.slot(0).unwrap().count, 1);
        assert_eq!(inventory.slot(1).unwrap().count, 1);
    }

    #[test]
    fn damaging_active_tool_removes_broken_tool() {
        let mut inventory = Inventory::new();
        inventory.set_slot(
            HOTBAR_START,
            Some(ItemStack {
                id: BlockType::FlintPickaxe.name().to_string(),
                count: 1,
                max_stack: 1,
                block_type: Some(BlockType::FlintPickaxe),
                durability: Some(1),
                max_durability: Some(24),
            }),
        );

        assert!(inventory.damage_active_tool(1));
        assert!(inventory.slot(HOTBAR_START).is_none());
    }

    #[test]
    fn empty_active_slot_defaults_to_hand_power() {
        let inventory = Inventory::new();

        assert_eq!(inventory.active_tool_power(), ToolTier::Hand.power());
    }

    #[test]
    fn tool_items_report_expected_tool_power() {
        let flint_pickaxe = ItemStack::from_inventory_item(BlockType::FlintPickaxe)
            .expect("tool should be inventory item");
        let stone_pickaxe = ItemStack::from_inventory_item(BlockType::StonePickaxe)
            .expect("tool should be inventory item");
        let iron_pickaxe = ItemStack::from_inventory_item(BlockType::IronPickaxe)
            .expect("tool should be inventory item");
        let diamond_pickaxe = ItemStack::from_inventory_item(BlockType::DiamondPickaxe)
            .expect("tool should be inventory item");
        let netherite_pickaxe = ItemStack::from_inventory_item(BlockType::NetheritePickaxe)
            .expect("tool should be inventory item");
        let dirt = ItemStack::from_inventory_item(BlockType::Dirt)
            .expect("dirt should be inventory item");

        assert_eq!(ToolTier::Hand.power(), 1);
        assert_eq!(flint_pickaxe.tool_power(), ToolTier::Flint.power());
        assert_eq!(stone_pickaxe.tool_power(), ToolTier::Stone.power());
        assert_eq!(iron_pickaxe.tool_power(), ToolTier::Iron.power());
        assert_eq!(diamond_pickaxe.tool_power(), ToolTier::Diamond.power());
        assert_eq!(netherite_pickaxe.tool_power(), ToolTier::Netherite.power());
        assert_eq!(dirt.tool_power(), ToolTier::Hand.power());
    }

    #[test]
    fn inventory_crafting_2x2_log_to_planks() {
        let recipes = RecipeRegistry::with_defaults();
        let mut inventory = Inventory::new();

        inventory.set_crafting_slot(
            0,
            Some(ItemStack::from_inventory_item(BlockType::TreeTrunk).expect("log should exist")),
            &recipes,
        );

        assert_eq!(
            inventory.crafting_output.as_ref().and_then(|stack| stack.block_type),
            Some(BlockType::Planks)
        );
        assert_eq!(inventory.crafting_output.as_ref().map(|stack| stack.count), Some(4));
    }

    #[test]
    fn inventory_crafting_2x2_planks_to_sticks() {
        let recipes = RecipeRegistry::with_defaults();
        let mut inventory = Inventory::new();

        inventory.set_crafting_slot(
            0,
            Some(ItemStack::from_inventory_item(BlockType::Planks).expect("planks should exist")),
            &recipes,
        );
        inventory.set_crafting_slot(
            2,
            Some(ItemStack::from_inventory_item(BlockType::Planks).expect("planks should exist")),
            &recipes,
        );

        assert_eq!(
            inventory.crafting_output.as_ref().and_then(|stack| stack.block_type),
            Some(BlockType::Stick)
        );
        assert_eq!(inventory.crafting_output.as_ref().map(|stack| stack.count), Some(4));
    }

    #[test]
    fn inventory_crafting_2x2_no_match_yields_none() {
        let recipes = RecipeRegistry::with_defaults();
        let mut inventory = Inventory::new();

        inventory.set_crafting_slot(
            0,
            Some(ItemStack::from_inventory_item(BlockType::Stone).expect("stone should exist")),
            &recipes,
        );
        inventory.set_crafting_slot(
            1,
            Some(ItemStack::from_inventory_item(BlockType::Mud).expect("mud should exist")),
            &recipes,
        );

        assert!(inventory.crafting_output.is_none());
    }
}