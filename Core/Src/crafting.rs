use std::array;

use serde::{Deserialize, Serialize};

use crate::{inventory::ItemStack, world::BlockType};

pub type ItemType = BlockType;

const EARLY_GAME_LOGS: [BlockType; 6] = [
    BlockType::TreeTrunk,
    BlockType::NeedleWood,
    BlockType::WarmWood,
    BlockType::WetWood,
    BlockType::PaleWood,
    BlockType::DarkWood,
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CraftingGrid {
    pub width: u8,
    pub height: u8,
    pub slots: [Option<ItemStack>; 9],
}

impl CraftingGrid {
    pub fn new(width: u8, height: u8) -> Self {
        assert!((1..=3).contains(&width));
        assert!((1..=3).contains(&height));

        Self {
            width,
            height,
            slots: array::from_fn(|_| None),
        }
    }

    pub fn clear(&mut self) {
        for idx in 0..self.active_len() {
            self.slots[idx] = None;
        }
    }

    pub fn set_slot(&mut self, idx: usize, item: Option<ItemStack>) {
        assert!(idx < self.active_len());
        self.slots[idx] = item;
    }

    pub fn get_slot(&self, idx: usize) -> &Option<ItemStack> {
        assert!(idx < self.active_len());
        &self.slots[idx]
    }

    pub fn active_len(&self) -> usize {
        usize::from(self.width) * usize::from(self.height)
    }

    fn item_type_at(&self, x: usize, y: usize) -> Option<ItemType> {
        if x >= usize::from(self.width) || y >= usize::from(self.height) {
            return None;
        }

        self.slots[y * usize::from(self.width) + x]
            .as_ref()
            .and_then(|stack| stack.block_type)
    }

    fn consume_one_from_each_filled_slot(&mut self) {
        for idx in 0..self.active_len() {
            let Some(stack) = self.slots[idx].as_mut() else {
                continue;
            };

            if stack.count > 1 {
                stack.count -= 1;
            } else {
                self.slots[idx] = None;
            }
        }
    }

    fn collected_item_types(&self) -> Vec<ItemType> {
        let mut items = Vec::new();
        for idx in 0..self.active_len() {
            if let Some(item) = self.slots[idx].as_ref().and_then(|stack| stack.block_type) {
                items.push(item);
            }
        }
        items
    }
}

#[derive(Clone, Debug)]
pub struct ShapedRecipe {
    pub width: u8,
    pub height: u8,
    pub pattern: Vec<Option<ItemType>>,
    pub output: ItemStack,
}

#[derive(Clone, Debug)]
pub struct ShapelessRecipe {
    pub ingredients: Vec<ItemType>,
    pub output: ItemStack,
}

#[derive(Clone, Debug, Default)]
pub struct RecipeRegistry {
    pub shaped: Vec<ShapedRecipe>,
    pub shapeless: Vec<ShapelessRecipe>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MatchedRecipeKind {
    Shaped(usize),
    Shapeless(usize),
}

impl RecipeRegistry {
    pub fn new() -> Self {
        Self {
            shaped: Vec::new(),
            shapeless: Vec::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut recipes = Self::new();

        fn register_shaped(
            recipes: &mut RecipeRegistry,
            width: u8,
            height: u8,
            pattern: Vec<Option<BlockType>>,
            output: BlockType,
            count: u32,
        ) {
            recipes.register_shaped(ShapedRecipe {
                width,
                height,
                pattern,
                output: stack_of(output, count),
            });
        }

        for log in EARLY_GAME_LOGS {
            recipes.register_shapeless(ShapelessRecipe {
                ingredients: vec![log],
                output: stack_of(BlockType::Planks, 4),
            });
        }

        register_shaped(
            &mut recipes,
            1,
            2,
            vec![Some(BlockType::Planks), Some(BlockType::Planks)],
            BlockType::Stick,
            4,
        );

        for log in EARLY_GAME_LOGS {
            register_shaped(
                &mut recipes,
                3,
                3,
                vec![
                    Some(BlockType::Planks),
                    Some(BlockType::Planks),
                    Some(BlockType::Planks),
                    Some(BlockType::Planks),
                    Some(log),
                    Some(BlockType::Planks),
                    Some(BlockType::Planks),
                    Some(BlockType::Planks),
                    Some(BlockType::Planks),
                ],
                BlockType::NVCrafter,
                1,
            );
        }

        register_shaped(
            &mut recipes,
            3,
            3,
            vec![
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                None,
                Some(BlockType::Stick),
                None,
                None,
                Some(BlockType::Stick),
                None,
            ],
            BlockType::WoodenPickaxe,
            1,
        );

        register_shaped(
            &mut recipes,
            2,
            3,
            vec![
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Stick),
                None,
                Some(BlockType::Stick),
            ],
            BlockType::WoodenAxe,
            1,
        );

        register_shaped(
            &mut recipes,
            1,
            3,
            vec![
                Some(BlockType::Planks),
                Some(BlockType::Stick),
                Some(BlockType::Stick),
            ],
            BlockType::WoodenShovel,
            1,
        );

        register_shaped(
            &mut recipes,
            2,
            3,
            vec![
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                None,
                Some(BlockType::Stick),
                None,
                Some(BlockType::Stick),
            ],
            BlockType::WoodenHoe,
            1,
        );

        for log in EARLY_GAME_LOGS {
            register_shaped(
                &mut recipes,
                1,
                2,
                vec![Some(BlockType::Stick), Some(log)],
                BlockType::Torch,
                4,
            );
        }

        register_shaped(
            &mut recipes,
            3,
            3,
            vec![
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                None,
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
            ],
            BlockType::Chest,
            1,
        );

        register_shaped(
            &mut recipes,
            2,
            3,
            vec![
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
            ],
            BlockType::Door,
            3,
        );

        register_shaped(
            &mut recipes,
            3,
            2,
            vec![
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
            ],
            BlockType::Trapdoor,
            2,
        );

        register_shaped(
            &mut recipes,
            3,
            3,
            vec![
                Some(BlockType::Stick),
                Some(BlockType::Planks),
                Some(BlockType::Stick),
                Some(BlockType::Stick),
                Some(BlockType::Planks),
                Some(BlockType::Stick),
                Some(BlockType::Stick),
                Some(BlockType::Planks),
                Some(BlockType::Stick),
            ],
            BlockType::Ladder,
            3,
        );

        register_shaped(
            &mut recipes,
            3,
            2,
            vec![
                Some(BlockType::Stick),
                Some(BlockType::Planks),
                Some(BlockType::Stick),
                Some(BlockType::Stick),
                Some(BlockType::Planks),
                Some(BlockType::Stick),
            ],
            BlockType::Fence,
            3,
        );

        register_shaped(
            &mut recipes,
            3,
            2,
            vec![
                Some(BlockType::Planks),
                Some(BlockType::Stick),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Stick),
                Some(BlockType::Planks),
            ],
            BlockType::FenceGate,
            1,
        );

        for log in EARLY_GAME_LOGS {
            register_shaped(
                &mut recipes,
                3,
                3,
                vec![
                    Some(BlockType::Planks),
                    Some(log),
                    Some(BlockType::Planks),
                    Some(log),
                    Some(log),
                    Some(log),
                    Some(BlockType::Planks),
                    Some(log),
                    Some(BlockType::Planks),
                ],
                BlockType::WorkbenchUpgrade,
                1,
            );
        }

        recipes.register_shapeless(ShapelessRecipe {
            ingredients: vec![BlockType::Flint, BlockType::Stick],
            output: stack_of(BlockType::FlintPickaxe, 1),
        });

        register_shaped(
            &mut recipes,
            3,
            3,
            vec![
                Some(BlockType::Stone),
                Some(BlockType::Stone),
                Some(BlockType::Stone),
                None,
                Some(BlockType::Stick),
                None,
                None,
                Some(BlockType::Stick),
                None,
            ],
            BlockType::StonePickaxe,
            1,
        );

        register_shaped(
            &mut recipes,
            3,
            3,
            vec![
                Some(BlockType::IronIngot),
                Some(BlockType::IronIngot),
                Some(BlockType::IronIngot),
                None,
                Some(BlockType::Stick),
                None,
                None,
                Some(BlockType::Stick),
                None,
            ],
            BlockType::IronPickaxe,
            1,
        );

        recipes
    }

    pub fn register_shaped(&mut self, recipe: ShapedRecipe) {
        self.shaped.push(recipe);
    }

    pub fn register_shapeless(&mut self, recipe: ShapelessRecipe) {
        self.shapeless.push(recipe);
    }

    pub fn match_grid(&self, grid: &CraftingGrid) -> Option<ItemStack> {
        self.find_match(grid).map(|matched| self.output_for_match(matched))
    }

    fn find_match(&self, grid: &CraftingGrid) -> Option<MatchedRecipeKind> {
        for (index, recipe) in self.shaped.iter().enumerate() {
            if shaped_recipe_matches(grid, recipe) {
                return Some(MatchedRecipeKind::Shaped(index));
            }
        }

        for (index, recipe) in self.shapeless.iter().enumerate() {
            if shapeless_recipe_matches(grid, recipe) {
                return Some(MatchedRecipeKind::Shapeless(index));
            }
        }

        None
    }

    fn output_for_match(&self, matched: MatchedRecipeKind) -> ItemStack {
        match matched {
            MatchedRecipeKind::Shaped(index) => self.shaped[index].output.clone(),
            MatchedRecipeKind::Shapeless(index) => self.shapeless[index].output.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NVCrafterState {
    pub grid: CraftingGrid,
    pub output: Option<ItemStack>,
}

impl NVCrafterState {
    pub fn new() -> Self {
        Self {
            grid: CraftingGrid::new(3, 3),
            output: None,
        }
    }

    pub fn slot(&self, idx: usize) -> &Option<ItemStack> {
        self.grid.get_slot(idx)
    }

    pub fn set_slot(&mut self, idx: usize, item: Option<ItemStack>, recipes: &RecipeRegistry) {
        self.grid.set_slot(idx, item);
        self.update_output(recipes);
    }

    pub fn take_slot(&mut self, idx: usize, recipes: &RecipeRegistry) -> Option<ItemStack> {
        let stack = self.grid.get_slot(idx).clone();
        self.grid.set_slot(idx, None);
        self.update_output(recipes);
        stack
    }

    pub fn update_output(&mut self, recipes: &RecipeRegistry) {
        self.output = recipes.match_grid(&self.grid);
    }

    pub fn take_output(&mut self, recipes: &RecipeRegistry) -> Option<ItemStack> {
        let result = recipes.match_grid(&self.grid)?;
        self.grid.consume_one_from_each_filled_slot();
        self.update_output(recipes);
        Some(result)
    }
}

fn stack_of(block: BlockType, count: u32) -> ItemStack {
    let mut stack = ItemStack::from_inventory_item(block).expect("recipe output should be inventory item");
    stack.count = count.min(stack.max_stack);
    stack
}

fn shaped_recipe_matches(grid: &CraftingGrid, recipe: &ShapedRecipe) -> bool {
    if recipe.pattern.len() != usize::from(recipe.width) * usize::from(recipe.height) {
        return false;
    }
    if recipe.width > grid.width || recipe.height > grid.height {
        return false;
    }

    let max_offset_x = usize::from(grid.width - recipe.width);
    let max_offset_y = usize::from(grid.height - recipe.height);

    for offset_y in 0..=max_offset_y {
        for offset_x in 0..=max_offset_x {
            let mut matched = true;

            for y in 0..usize::from(grid.height) {
                for x in 0..usize::from(grid.width) {
                    let expected = if x >= offset_x
                        && x < offset_x + usize::from(recipe.width)
                        && y >= offset_y
                        && y < offset_y + usize::from(recipe.height)
                    {
                        let pattern_x = x - offset_x;
                        let pattern_y = y - offset_y;
                        recipe.pattern[pattern_y * usize::from(recipe.width) + pattern_x]
                    } else {
                        None
                    };

                    if grid.item_type_at(x, y) != expected {
                        matched = false;
                        break;
                    }
                }

                if !matched {
                    break;
                }
            }

            if matched {
                return true;
            }
        }
    }

    false
}

fn shapeless_recipe_matches(grid: &CraftingGrid, recipe: &ShapelessRecipe) -> bool {
    let mut remaining = recipe.ingredients.clone();
    let present = grid.collected_item_types();
    if present.len() != remaining.len() {
        return false;
    }

    for item in present {
        let Some(index) = remaining.iter().position(|candidate| *candidate == item) else {
            return false;
        };
        remaining.remove(index);
    }

    remaining.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_recipe_output(
        recipes: &RecipeRegistry,
        width: u8,
        height: u8,
        placements: &[(usize, BlockType)],
        expected_block: BlockType,
        expected_count: u32,
    ) {
        let mut grid = CraftingGrid::new(width, height);
        for &(index, block) in placements {
            grid.set_slot(index, Some(stack_of(block, 1)));
        }

        let output = recipes
            .match_grid(&grid)
            .expect("expected recipe to produce output");
        assert_eq!(output.block_type, Some(expected_block));
        assert_eq!(output.count, expected_count);
    }

    #[test]
    fn shaped_recipe_matches_when_aligned() {
        let mut registry = RecipeRegistry::new();
        registry.register_shaped(ShapedRecipe {
            width: 2,
            height: 2,
            pattern: vec![
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
                Some(BlockType::Planks),
            ],
            output: stack_of(BlockType::Chest, 1),
        });

        let mut grid = CraftingGrid::new(3, 3);
        for (idx, block) in [
            (4usize, BlockType::Planks),
            (5usize, BlockType::Planks),
            (7usize, BlockType::Planks),
            (8usize, BlockType::Planks),
        ] {
            grid.set_slot(idx, Some(stack_of(block, 1)));
        }

        let output = registry.match_grid(&grid).expect("expected aligned shaped recipe to match");
        assert_eq!(output.block_type, Some(BlockType::Chest));
    }

    #[test]
    fn shapeless_recipe_matches_multiset() {
        let mut registry = RecipeRegistry::new();
        registry.register_shapeless(ShapelessRecipe {
            ingredients: vec![BlockType::Flint, BlockType::Stick],
            output: stack_of(BlockType::FlintPickaxe, 1),
        });

        let mut grid = CraftingGrid::new(2, 2);
        grid.set_slot(0, Some(stack_of(BlockType::Stick, 1)));
        grid.set_slot(3, Some(stack_of(BlockType::Flint, 1)));

        let output = registry.match_grid(&grid).expect("expected shapeless recipe to match");
        assert_eq!(output.block_type, Some(BlockType::FlintPickaxe));
    }

    #[test]
    fn default_registry_crafts_requested_early_game_recipes() {
        let recipes = RecipeRegistry::with_defaults();

        assert_recipe_output(&recipes, 1, 1, &[(0, BlockType::TreeTrunk)], BlockType::Planks, 4);
        assert_recipe_output(
            &recipes,
            1,
            2,
            &[(0, BlockType::Planks), (1, BlockType::Planks)],
            BlockType::Stick,
            4,
        );
        assert_recipe_output(
            &recipes,
            3,
            3,
            &[
                (0, BlockType::Planks),
                (1, BlockType::Planks),
                (2, BlockType::Planks),
                (3, BlockType::Planks),
                (4, BlockType::TreeTrunk),
                (5, BlockType::Planks),
                (6, BlockType::Planks),
                (7, BlockType::Planks),
                (8, BlockType::Planks),
            ],
            BlockType::NVCrafter,
            1,
        );
        assert_recipe_output(
            &recipes,
            3,
            3,
            &[
                (0, BlockType::Planks),
                (1, BlockType::Planks),
                (2, BlockType::Planks),
                (4, BlockType::Stick),
                (7, BlockType::Stick),
            ],
            BlockType::WoodenPickaxe,
            1,
        );
        assert_recipe_output(
            &recipes,
            2,
            3,
            &[
                (0, BlockType::Planks),
                (1, BlockType::Planks),
                (2, BlockType::Planks),
                (3, BlockType::Stick),
                (5, BlockType::Stick),
            ],
            BlockType::WoodenAxe,
            1,
        );
        assert_recipe_output(
            &recipes,
            1,
            3,
            &[
                (0, BlockType::Planks),
                (1, BlockType::Stick),
                (2, BlockType::Stick),
            ],
            BlockType::WoodenShovel,
            1,
        );
        assert_recipe_output(
            &recipes,
            2,
            3,
            &[
                (0, BlockType::Planks),
                (1, BlockType::Planks),
                (3, BlockType::Stick),
                (5, BlockType::Stick),
            ],
            BlockType::WoodenHoe,
            1,
        );
        assert_recipe_output(
            &recipes,
            1,
            2,
            &[(0, BlockType::Stick), (1, BlockType::TreeTrunk)],
            BlockType::Torch,
            4,
        );
        assert_recipe_output(
            &recipes,
            3,
            3,
            &[
                (0, BlockType::Planks),
                (1, BlockType::Planks),
                (2, BlockType::Planks),
                (3, BlockType::Planks),
                (5, BlockType::Planks),
                (6, BlockType::Planks),
                (7, BlockType::Planks),
                (8, BlockType::Planks),
            ],
            BlockType::Chest,
            1,
        );
        assert_recipe_output(
            &recipes,
            2,
            3,
            &[
                (0, BlockType::Planks),
                (1, BlockType::Planks),
                (2, BlockType::Planks),
                (3, BlockType::Planks),
                (4, BlockType::Planks),
                (5, BlockType::Planks),
            ],
            BlockType::Door,
            3,
        );
        assert_recipe_output(
            &recipes,
            3,
            2,
            &[
                (0, BlockType::Planks),
                (1, BlockType::Planks),
                (2, BlockType::Planks),
                (3, BlockType::Planks),
                (4, BlockType::Planks),
                (5, BlockType::Planks),
            ],
            BlockType::Trapdoor,
            2,
        );
        assert_recipe_output(
            &recipes,
            3,
            3,
            &[
                (0, BlockType::Stick),
                (1, BlockType::Planks),
                (2, BlockType::Stick),
                (3, BlockType::Stick),
                (4, BlockType::Planks),
                (5, BlockType::Stick),
                (6, BlockType::Stick),
                (7, BlockType::Planks),
                (8, BlockType::Stick),
            ],
            BlockType::Ladder,
            3,
        );
        assert_recipe_output(
            &recipes,
            3,
            2,
            &[
                (0, BlockType::Stick),
                (1, BlockType::Planks),
                (2, BlockType::Stick),
                (3, BlockType::Stick),
                (4, BlockType::Planks),
                (5, BlockType::Stick),
            ],
            BlockType::Fence,
            3,
        );
        assert_recipe_output(
            &recipes,
            3,
            2,
            &[
                (0, BlockType::Planks),
                (1, BlockType::Stick),
                (2, BlockType::Planks),
                (3, BlockType::Planks),
                (4, BlockType::Stick),
                (5, BlockType::Planks),
            ],
            BlockType::FenceGate,
            1,
        );
        assert_recipe_output(
            &recipes,
            3,
            3,
            &[
                (0, BlockType::Planks),
                (1, BlockType::TreeTrunk),
                (2, BlockType::Planks),
                (3, BlockType::TreeTrunk),
                (4, BlockType::TreeTrunk),
                (5, BlockType::TreeTrunk),
                (6, BlockType::Planks),
                (7, BlockType::TreeTrunk),
                (8, BlockType::Planks),
            ],
            BlockType::WorkbenchUpgrade,
            1,
        );
    }

    #[test]
    fn log_based_recipes_accept_non_oak_logs() {
        let recipes = RecipeRegistry::with_defaults();

        assert_recipe_output(&recipes, 1, 1, &[(0, BlockType::DarkWood)], BlockType::Planks, 4);
        assert_recipe_output(
            &recipes,
            1,
            2,
            &[(0, BlockType::Stick), (1, BlockType::DarkWood)],
            BlockType::Torch,
            4,
        );
    }

    #[test]
    fn nvcrafter_3x3_recipe_produces_output() {
        let recipes = RecipeRegistry::with_defaults();
        let mut state = NVCrafterState::new();

        for idx in [0usize, 1, 2, 3, 5, 6, 7, 8] {
            state.grid.set_slot(idx, Some(stack_of(BlockType::Planks, 1)));
        }
        state.grid.set_slot(4, Some(stack_of(BlockType::TreeTrunk, 1)));

        state.update_output(&recipes);

        assert_eq!(state.output.as_ref().and_then(|stack| stack.block_type), Some(BlockType::NVCrafter));
    }

    #[test]
    fn nvcrafter_take_output_consumes_ingredients() {
        let recipes = RecipeRegistry::with_defaults();
        let mut state = NVCrafterState::new();

        for idx in [0usize, 1, 2, 3, 5, 6, 7, 8] {
            state.grid.set_slot(idx, Some(stack_of(BlockType::Planks, 1)));
        }
        state.grid.set_slot(4, Some(stack_of(BlockType::TreeTrunk, 1)));
        state.update_output(&recipes);

        let output = state.take_output(&recipes).expect("expected crafter output");
        assert_eq!(output.block_type, Some(BlockType::NVCrafter));
        assert!(state.output.is_none());
        for idx in 0..state.grid.active_len() {
            assert!(state.grid.get_slot(idx).is_none());
        }
    }
}