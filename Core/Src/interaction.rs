use std::{array, collections::{HashSet, VecDeque}};

use cgmath::Vector3;
use winit::event::MouseButton;

use crate::{
    crafting::RecipeRegistry,
    input::InputState,
    inventory::{Inventory, ItemStack, HOTBAR_SLOT_COUNT, HOTBAR_START, INVENTORY_SLOT_COUNT},
    renderer::camera::Camera,
    world::{BlockType, RaycastHit, World},
};

const SLOT_SIZE: f32 = 48.0;
const SLOT_GAP: f32 = 8.0;
const PANEL_PADDING: f32 = 18.0;
const PANEL_GAP: f32 = 20.0;
const HOTBAR_BOTTOM_MARGIN: f32 = 22.0;
const TITLE_HEIGHT: f32 = 30.0;
const PLAYER_CRAFTING_SLOT_COUNT: usize = 4;
const NVCRAFTER_SLOT_COUNT: usize = 9;
const TREE_TRUNK_SEARCH_LIMIT: usize = 512;
const TREE_LEAF_SEARCH_LIMIT: usize = 512;
const SAPLING_DROP_ODDS: u64 = 100;
const STICK_DROP_ODDS: u64 = 10;
const FLINT_DROP_ODDS: u64 = 10;
const TRUNK_NEIGHBOR_OFFSETS: [(i32, i32, i32); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];
const LEAF_NEIGHBOR_OFFSETS: [(i32, i32, i32); 5] = [
    (0, 1, 0),
    (1, 0, 0),
    (-1, 0, 0),
    (0, 0, 1),
    (0, 0, -1),
];

#[derive(Clone, Copy, Debug)]
pub struct SlotRect {
    pub x: f32,
    pub y: f32,
    pub size: f32,
}

impl SlotRect {
    pub fn contains(self, cursor_x: f32, cursor_y: f32) -> bool {
        cursor_x >= self.x
            && cursor_x <= self.x + self.size
            && cursor_y >= self.y
            && cursor_y <= self.y + self.size
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PanelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiType {
    Inventory,
    NVCrafter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenGui {
    Inventory,
    NVCrafter { position: Vector3<i32> },
}

impl OpenGui {
    fn gui_type(self) -> GuiType {
        match self {
            OpenGui::Inventory => GuiType::Inventory,
            OpenGui::NVCrafter { .. } => GuiType::NVCrafter,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiSlotId {
    Inventory(usize),
    PlayerCrafting(usize),
    PlayerCraftingOutput,
    NVCrafter(usize),
    NVCrafterOutput,
}

impl UiSlotId {
    pub fn is_output(self) -> bool {
        matches!(self, UiSlotId::PlayerCraftingOutput | UiSlotId::NVCrafterOutput)
    }
}

#[derive(Clone, Debug)]
pub struct InventoryLayout {
    pub hotbar_panel: PanelRect,
    pub main_panel: Option<PanelRect>,
    pub hotbar_slots: [SlotRect; HOTBAR_SLOT_COUNT],
    pub player_slot_rects: [SlotRect; INVENTORY_SLOT_COUNT],
    pub player_crafting_slots: [Option<SlotRect>; PLAYER_CRAFTING_SLOT_COUNT],
    pub player_crafting_output: Option<SlotRect>,
    pub nvcrafter_slots: [Option<SlotRect>; NVCRAFTER_SLOT_COUNT],
    pub nvcrafter_output: Option<SlotRect>,
    pub title_position: Option<(f32, f32)>,
}

impl InventoryLayout {
    pub fn slot_at(&self, cursor_x: f32, cursor_y: f32, gui_type: Option<GuiType>) -> Option<UiSlotId> {
        match gui_type {
            None => self.hotbar_slots.iter().enumerate().find_map(|(offset, rect)| {
                rect.contains(cursor_x, cursor_y)
                    .then_some(UiSlotId::Inventory(HOTBAR_START + offset))
            }),
            Some(GuiType::Inventory) => {
                if let Some(rect) = self.player_crafting_output {
                    if rect.contains(cursor_x, cursor_y) {
                        return Some(UiSlotId::PlayerCraftingOutput);
                    }
                }

                for (index, rect) in self.player_crafting_slots.iter().enumerate() {
                    if rect.is_some_and(|slot| slot.contains(cursor_x, cursor_y)) {
                        return Some(UiSlotId::PlayerCrafting(index));
                    }
                }

                self.player_slot_rects.iter().enumerate().find_map(|(index, rect)| {
                    rect.contains(cursor_x, cursor_y).then_some(UiSlotId::Inventory(index))
                })
            }
            Some(GuiType::NVCrafter) => {
                if let Some(rect) = self.nvcrafter_output {
                    if rect.contains(cursor_x, cursor_y) {
                        return Some(UiSlotId::NVCrafterOutput);
                    }
                }

                for (index, rect) in self.nvcrafter_slots.iter().enumerate() {
                    if rect.is_some_and(|slot| slot.contains(cursor_x, cursor_y)) {
                        return Some(UiSlotId::NVCrafter(index));
                    }
                }

                self.player_slot_rects.iter().enumerate().find_map(|(index, rect)| {
                    rect.contains(cursor_x, cursor_y).then_some(UiSlotId::Inventory(index))
                })
            }
        }
    }

    pub fn visible_slots(&self, gui_type: Option<GuiType>) -> Vec<(UiSlotId, SlotRect)> {
        let mut slots = Vec::new();

        match gui_type {
            None => {
                for (offset, rect) in self.hotbar_slots.iter().enumerate() {
                    slots.push((UiSlotId::Inventory(HOTBAR_START + offset), *rect));
                }
            }
            Some(GuiType::Inventory) => {
                for (index, rect) in self.player_slot_rects.iter().enumerate() {
                    slots.push((UiSlotId::Inventory(index), *rect));
                }
                for (index, rect) in self.player_crafting_slots.iter().enumerate() {
                    if let Some(rect) = rect {
                        slots.push((UiSlotId::PlayerCrafting(index), *rect));
                    }
                }
                if let Some(rect) = self.player_crafting_output {
                    slots.push((UiSlotId::PlayerCraftingOutput, rect));
                }
            }
            Some(GuiType::NVCrafter) => {
                for (index, rect) in self.player_slot_rects.iter().enumerate() {
                    slots.push((UiSlotId::Inventory(index), *rect));
                }
                for (index, rect) in self.nvcrafter_slots.iter().enumerate() {
                    if let Some(rect) = rect {
                        slots.push((UiSlotId::NVCrafter(index), *rect));
                    }
                }
                if let Some(rect) = self.nvcrafter_output {
                    slots.push((UiSlotId::NVCrafterOutput, rect));
                }
            }
        }

        slots
    }
}

pub fn build_inventory_layout(screen_size: (u32, u32), gui_type: Option<GuiType>) -> InventoryLayout {
    let screen_w = screen_size.0 as f32;
    let screen_h = screen_size.1 as f32;
    let grid_width = SLOT_SIZE * HOTBAR_SLOT_COUNT as f32 + SLOT_GAP * (HOTBAR_SLOT_COUNT as f32 - 1.0);
    let hotbar_panel_width = grid_width + PANEL_PADDING * 2.0;
    let hotbar_panel_height = SLOT_SIZE + PANEL_PADDING * 2.0;
    let hotbar_x = (screen_w - hotbar_panel_width) * 0.5;
    let hotbar_y = screen_h - HOTBAR_BOTTOM_MARGIN - hotbar_panel_height;

    let hotbar_slots = array::from_fn(|slot| SlotRect {
        x: hotbar_x + PANEL_PADDING + slot as f32 * (SLOT_SIZE + SLOT_GAP),
        y: hotbar_y + PANEL_PADDING,
        size: SLOT_SIZE,
    });

    let mut player_slot_rects = [SlotRect { x: 0.0, y: 0.0, size: SLOT_SIZE }; INVENTORY_SLOT_COUNT];
    for hotbar_slot in 0..HOTBAR_SLOT_COUNT {
        player_slot_rects[HOTBAR_START + hotbar_slot] = hotbar_slots[hotbar_slot];
    }
    let mut player_crafting_slots = [None; PLAYER_CRAFTING_SLOT_COUNT];
    let mut nvcrafter_slots = [None; NVCRAFTER_SLOT_COUNT];

    if gui_type.is_none() {
        return InventoryLayout {
            hotbar_panel: PanelRect {
                x: hotbar_x,
                y: hotbar_y,
                width: hotbar_panel_width,
                height: hotbar_panel_height,
            },
            main_panel: None,
            hotbar_slots,
            player_slot_rects,
            player_crafting_slots,
            player_crafting_output: None,
            nvcrafter_slots,
            nvcrafter_output: None,
            title_position: None,
        };
    }

    let gui_type = gui_type.expect("checked above");
    let inventory_panel_width = hotbar_panel_width;
    let inventory_grid_height = SLOT_SIZE * 3.0 + SLOT_GAP * 2.0;
    let crafting_columns = match gui_type {
        GuiType::Inventory => 2,
        GuiType::NVCrafter => 3,
    };
    let crafting_rows = crafting_columns;
    let crafting_grid_width = SLOT_SIZE * crafting_columns as f32 + SLOT_GAP * (crafting_columns as f32 - 1.0);
    let crafting_grid_height = SLOT_SIZE * crafting_rows as f32 + SLOT_GAP * (crafting_rows as f32 - 1.0);
    let inventory_panel_height = PANEL_PADDING * 2.0
        + TITLE_HEIGHT
        + PANEL_GAP
        + crafting_grid_height
        + PANEL_GAP
        + inventory_grid_height
        + PANEL_GAP
        + SLOT_SIZE;
    let inventory_panel_x = (screen_w - inventory_panel_width) * 0.5;
    let inventory_panel_y = (screen_h - inventory_panel_height) * 0.5;
    let inventory_grid_origin_x = inventory_panel_x + PANEL_PADDING;
    let crafting_section_width = crafting_grid_width + PANEL_GAP + SLOT_SIZE;
    let crafting_origin_x = inventory_panel_x + (inventory_panel_width - crafting_section_width) * 0.5;
    let crafting_origin_y = inventory_panel_y + PANEL_PADDING + TITLE_HEIGHT + PANEL_GAP;
    let output_rect = SlotRect {
        x: crafting_origin_x + crafting_grid_width + PANEL_GAP,
        y: crafting_origin_y + (crafting_grid_height - SLOT_SIZE) * 0.5,
        size: SLOT_SIZE,
    };
    let inventory_grid_origin_y = crafting_origin_y + crafting_grid_height + PANEL_GAP;

    for row in 0..3 {
        for col in 0..HOTBAR_SLOT_COUNT {
            let index = row * HOTBAR_SLOT_COUNT + col;
            player_slot_rects[index] = SlotRect {
                x: inventory_grid_origin_x + col as f32 * (SLOT_SIZE + SLOT_GAP),
                y: inventory_grid_origin_y + row as f32 * (SLOT_SIZE + SLOT_GAP),
                size: SLOT_SIZE,
            };
        }
    }

    let inventory_hotbar_y = inventory_grid_origin_y + inventory_grid_height + PANEL_GAP;
    for hotbar_slot in 0..HOTBAR_SLOT_COUNT {
        player_slot_rects[HOTBAR_START + hotbar_slot] = SlotRect {
            x: inventory_grid_origin_x + hotbar_slot as f32 * (SLOT_SIZE + SLOT_GAP),
            y: inventory_hotbar_y,
            size: SLOT_SIZE,
        };
    }

    let mut player_crafting_output = None;
    let mut nvcrafter_output = None;
    match gui_type {
        GuiType::Inventory => {
            for row in 0..2 {
                for col in 0..2 {
                    let index = row * 2 + col;
                    player_crafting_slots[index] = Some(SlotRect {
                        x: crafting_origin_x + col as f32 * (SLOT_SIZE + SLOT_GAP),
                        y: crafting_origin_y + row as f32 * (SLOT_SIZE + SLOT_GAP),
                        size: SLOT_SIZE,
                    });
                }
            }
            player_crafting_output = Some(output_rect);
        }
        GuiType::NVCrafter => {
            for row in 0..3 {
                for col in 0..3 {
                    let index = row * 3 + col;
                    nvcrafter_slots[index] = Some(SlotRect {
                        x: crafting_origin_x + col as f32 * (SLOT_SIZE + SLOT_GAP),
                        y: crafting_origin_y + row as f32 * (SLOT_SIZE + SLOT_GAP),
                        size: SLOT_SIZE,
                    });
                }
            }
            nvcrafter_output = Some(output_rect);
        }
    }

    InventoryLayout {
        hotbar_panel: PanelRect {
            x: hotbar_x,
            y: hotbar_y,
            width: hotbar_panel_width,
            height: hotbar_panel_height,
        },
        main_panel: Some(PanelRect {
            x: inventory_panel_x,
            y: inventory_panel_y,
            width: inventory_panel_width,
            height: inventory_panel_height,
        }),
        hotbar_slots,
        player_slot_rects,
        player_crafting_slots,
        player_crafting_output,
        nvcrafter_slots,
        nvcrafter_output,
        title_position: Some((inventory_panel_x + inventory_panel_width * 0.5, inventory_panel_y + PANEL_PADDING)),
    }
}

#[derive(Clone, Debug, Default)]
struct BreakState {
    target: Option<Vector3<i32>>,
    progress: f32,
    required_time: f32,
    tool: Option<BlockType>,
}

impl BreakState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn fraction(&self) -> f32 {
        if self.required_time <= f32::EPSILON {
            0.0
        } else {
            (self.progress / self.required_time).clamp(0.0, 1.0)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct MiningResolution {
    required_time: f32,
    harvest_allowed: bool,
    consume_tool_durability: bool,
    tool_block: Option<BlockType>,
}

pub struct InteractionController {
    inventory: Inventory,
    recipes: RecipeRegistry,
    target: Option<RaycastHit>,
    break_state: BreakState,
    open_gui: Option<OpenGui>,
    dragged_stack: Option<ItemStack>,
    dragged_from_slot: Option<UiSlotId>,
    hovered_slot: Option<UiSlotId>,
    cursor_position: Option<(f32, f32)>,
}

impl Default for InteractionController {
    fn default() -> Self {
        Self {
            inventory: Inventory::default(),
            recipes: RecipeRegistry::with_defaults(),
            target: None,
            break_state: BreakState::default(),
            open_gui: None,
            dragged_stack: None,
            dragged_from_slot: None,
            hovered_slot: None,
            cursor_position: None,
        }
    }
}

impl InteractionController {
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    pub fn gui_type(&self) -> Option<GuiType> {
        self.open_gui.map(OpenGui::gui_type)
    }

    pub fn inventory_open(&self) -> bool {
        self.open_gui.is_some()
    }

    pub fn toggle_inventory(&mut self, world: &mut World) -> bool {
        if matches!(self.open_gui, Some(OpenGui::Inventory)) {
            let _ = self.close_inventory(world);
            return false;
        }

        if self.open_gui.is_some() {
            let _ = self.close_inventory(world);
        }

        self.open_gui = Some(OpenGui::Inventory);
        self.break_state.reset();
        self.target = None;
        self.hovered_slot = None;
        true
    }

    pub fn close_inventory(&mut self, world: &mut World) -> bool {
        let Some(open_gui) = self.open_gui else {
            return false;
        };

        self.return_dragged_stack(world);
        self.return_open_crafting_items(world, open_gui);

        self.open_gui = None;
        self.target = None;
        self.hovered_slot = None;
        self.cursor_position = None;
        self.break_state.reset();
        true
    }

    pub fn target(&self) -> Option<RaycastHit> {
        self.target
    }

    pub fn break_fraction(&self) -> f32 {
        self.break_state.fraction()
    }

    pub fn hovered_slot(&self) -> Option<UiSlotId> {
        self.hovered_slot
    }

    pub fn dragged_stack(&self) -> Option<&ItemStack> {
        self.dragged_stack.as_ref()
    }

    pub fn cursor_position(&self) -> Option<(f32, f32)> {
        self.cursor_position
    }

    pub fn stack_for_slot(&self, world: &World, slot: UiSlotId) -> Option<ItemStack> {
        let crafter_pos = self.active_nvcrafter_position();
        match slot {
            UiSlotId::Inventory(index) => self.inventory.slot(index).cloned(),
            UiSlotId::PlayerCrafting(index) => self.inventory.crafting_slot(index).clone(),
            UiSlotId::PlayerCraftingOutput => self.inventory.crafting_output.clone(),
            UiSlotId::NVCrafter(index) => {
                let position = crafter_pos?;
                world.nvcrafter_state(position).and_then(|state| state.slot(index).clone())
            }
            UiSlotId::NVCrafterOutput => {
                let position = crafter_pos?;
                world.nvcrafter_state(position).and_then(|state| state.output.clone())
            }
        }
    }

    fn active_tool_can_break_block(&self, block: BlockType) -> bool {
        active_tool_satisfies_break_gate(block, self.inventory.active_stack())
    }

    fn active_nvcrafter_position(&self) -> Option<Vector3<i32>> {
        match self.open_gui {
            Some(OpenGui::NVCrafter { position }) => Some(position),
            _ => None,
        }
    }

    fn return_open_crafting_items(&mut self, world: &mut World, open_gui: OpenGui) {
        match open_gui {
            OpenGui::Inventory => self.return_player_crafting_items(),
            OpenGui::NVCrafter { position } => self.return_nvcrafter_items(world, position),
        }
    }

    fn return_player_crafting_items(&mut self) {
        let mut returned = Vec::new();

        for index in 0..PLAYER_CRAFTING_SLOT_COUNT {
            if let Some(stack) = self.inventory.take_crafting_slot(index, &self.recipes) {
                returned.push(stack);
            }
        }

        for stack in returned {
            let leftover = self.inventory.add_item(stack);
            debug_assert!(
                leftover.is_none(),
                "player crafting inputs should always fit back into inventory on close"
            );
        }
    }

    fn return_nvcrafter_items(&mut self, world: &mut World, position: Vector3<i32>) {
        let mut returned = Vec::new();

        if let Some(state) = world.nvcrafter_state_mut(position) {
            for index in 0..state.grid.active_len() {
                if let Some(stack) = state.take_slot(index, &self.recipes) {
                    returned.push(stack);
                }
            }
        }

        for stack in returned {
            let leftover = self.inventory.add_item(stack);
            debug_assert!(
                leftover.is_none(),
                "NVCrafter inputs should always fit back into inventory on close"
            );
            if let Some(leftover) = leftover {
                world.queue_item_drop(position, leftover);
            }
        }
    }

    fn open_nvcrafter_gui(&mut self, world: &mut World, position: Vector3<i32>) {
        if world.ensure_nvcrafter_state(position).is_none() {
            return;
        }

        self.open_gui = Some(OpenGui::NVCrafter { position });
        self.break_state.reset();
        self.target = None;
        self.hovered_slot = None;
        self.cursor_position = None;
    }

    fn enforce_active_tool_break_gate(
        &mut self,
        block: BlockType,
        active_stack: Option<&ItemStack>,
    ) -> bool {
        if active_tool_satisfies_break_gate(block, active_stack) {
            true
        } else {
            self.break_state.reset();
            false
        }
    }

    fn destroy_block_with_active_tool_gate(
        &mut self,
        world: &mut World,
        position: Vector3<i32>,
    ) -> Option<BlockType> {
        let block = world.get_block(position.x, position.y, position.z);
        if !self.active_tool_can_break_block(block) {
            return None;
        }

        world.destroy_block(position)
    }

    pub fn update(
        &mut self,
        world: &mut World,
        input_state: &InputState,
        camera: &Camera,
        dt: f32,
    ) {
        if self.open_gui.is_some() {
            self.target = None;
            self.break_state.reset();
            self.hovered_slot = None;
            self.cursor_position = None;
            return;
        }

        let scroll_steps = input_state.scroll_lines.round() as i32;
        if scroll_steps != 0 {
            self.inventory.scroll_hotbar(scroll_steps);
        }

        self.hovered_slot = None;
        self.cursor_position = None;
        self.target = world.raycast_block(camera.interaction_origin(), camera.look_direction());

        self.update_block_break(world, input_state, dt);
        if input_state.was_mouse_pressed(MouseButton::Right) {
            self.handle_right_click(world, camera);
        }
    }

    pub fn update_inventory_input(&mut self, world: &mut World, input: &InputState, screen_size: (u32, u32)) {
        if self.open_gui.is_none() {
            self.hovered_slot = None;
            self.cursor_position = None;
            return;
        }

        if let Some(position) = self.active_nvcrafter_position() {
            if world.get_block(position.x, position.y, position.z) != BlockType::NVCrafter {
                let _ = self.close_inventory(world);
                return;
            }
        }

        self.target = None;
        self.break_state.reset();
        self.update_inventory_drag(world, input, screen_size);
    }

    fn update_inventory_drag(&mut self, world: &mut World, input: &InputState, screen_size: (u32, u32)) {
        let layout = build_inventory_layout(screen_size, self.gui_type());
        self.cursor_position = input.cursor_position;
        self.hovered_slot = input
            .cursor_position
            .and_then(|(x, y)| layout.slot_at(x, y, self.gui_type()));

        if input.was_mouse_pressed(MouseButton::Left) {
            if let Some(slot) = self.hovered_slot {
                if slot.is_output() {
                    if self.dragged_stack.is_none() {
                        self.handle_output_click(world, slot);
                    }
                    return;
                }
            }

            if self.dragged_stack.is_none() {
                if let Some(slot_index) = self.hovered_slot {
                    self.dragged_stack = self.take_slot_stack(world, slot_index);
                    if self.dragged_stack.is_some() {
                        self.dragged_from_slot = Some(slot_index);
                    }
                }
            }
        }

        if input.was_mouse_released(MouseButton::Left) && self.dragged_stack.is_some() {
            self.commit_dragged_stack(world);
        }
    }

    fn handle_output_click(&mut self, world: &mut World, slot: UiSlotId) {
        match slot {
            UiSlotId::PlayerCraftingOutput => {
                let Some(result) = self.inventory.crafting_output.clone() else {
                    return;
                };
                if !self.inventory.can_accept_item(&result) {
                    return;
                }

                if let Some(result) = self.inventory.take_crafting_output(&self.recipes) {
                    let leftover = self.inventory.add_item(result);
                    debug_assert!(leftover.is_none());
                }
            }
            UiSlotId::NVCrafterOutput => {
                let Some(position) = self.active_nvcrafter_position() else {
                    return;
                };
                let Some(result) = world
                    .nvcrafter_state(position)
                    .and_then(|state| state.output.clone())
                else {
                    return;
                };
                if !self.inventory.can_accept_item(&result) {
                    return;
                }

                if let Some(state) = world.nvcrafter_state_mut(position) {
                    if let Some(result) = state.take_output(&self.recipes) {
                        let leftover = self.inventory.add_item(result);
                        debug_assert!(
                            leftover.is_none(),
                            "prevalidated NVCrafter output should always fit into inventory"
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn take_slot_stack(&mut self, world: &mut World, slot: UiSlotId) -> Option<ItemStack> {
        match slot {
            UiSlotId::Inventory(index) => self.inventory.take_slot(index),
            UiSlotId::PlayerCrafting(index) => self.inventory.take_crafting_slot(index, &self.recipes),
            UiSlotId::PlayerCraftingOutput | UiSlotId::NVCrafterOutput => None,
            UiSlotId::NVCrafter(index) => {
                let position = self.active_nvcrafter_position()?;
                world
                    .nvcrafter_state_mut(position)
                    .and_then(|state| state.take_slot(index, &self.recipes))
            }
        }
    }

    fn set_slot_stack(&mut self, world: &mut World, slot: UiSlotId, stack: Option<ItemStack>) -> bool {
        match slot {
            UiSlotId::Inventory(index) => {
                self.inventory.set_slot(index, stack);
                true
            }
            UiSlotId::PlayerCrafting(index) => {
                self.inventory.set_crafting_slot(index, stack, &self.recipes);
                true
            }
            UiSlotId::PlayerCraftingOutput | UiSlotId::NVCrafterOutput => false,
            UiSlotId::NVCrafter(index) => {
                let Some(position) = self.active_nvcrafter_position() else {
                    return false;
                };
                let Some(state) = world.nvcrafter_state_mut(position) else {
                    return false;
                };
                state.set_slot(index, stack, &self.recipes);
                true
            }
        }
    }

    fn slot_is_empty(&self, world: &World, slot: UiSlotId) -> bool {
        self.stack_for_slot(world, slot).is_none()
    }

    fn commit_dragged_stack(&mut self, world: &mut World) {
        let Some(stack) = self.dragged_stack.take() else {
            self.dragged_from_slot = None;
            return;
        };

        let Some(target_slot) = self.hovered_slot else {
            self.return_or_store_stack(world, stack);
            self.dragged_from_slot = None;
            return;
        };

        if target_slot.is_output() {
            self.return_or_store_stack(world, stack);
            self.dragged_from_slot = None;
            return;
        }

        if self.dragged_from_slot == Some(target_slot) {
            let _ = self.set_slot_stack(world, target_slot, Some(stack));
            self.dragged_from_slot = None;
            return;
        }

        let existing = self.take_slot_stack(world, target_slot);
        match existing {
            None => {
                let _ = self.set_slot_stack(world, target_slot, Some(stack));
            }
            Some(mut slot_stack) if slot_stack.can_stack_with(&stack) => {
                let transfer = (slot_stack.max_stack - slot_stack.count).min(stack.count);
                slot_stack.count += transfer;
                let remaining = stack.count - transfer;
                let _ = self.set_slot_stack(world, target_slot, Some(slot_stack));
                if remaining > 0 {
                    let mut leftover = stack;
                    leftover.count = remaining;
                    self.return_or_store_stack(world, leftover);
                }
            }
            Some(slot_stack) => {
                let _ = self.set_slot_stack(world, target_slot, Some(stack));
                self.return_or_store_stack(world, slot_stack);
            }
        }

        self.dragged_from_slot = None;
    }

    fn return_dragged_stack(&mut self, world: &mut World) {
        if let Some(stack) = self.dragged_stack.take() {
            self.return_or_store_stack(world, stack);
        }
        self.dragged_from_slot = None;
    }

    fn return_or_store_stack(&mut self, world: &mut World, stack: ItemStack) {
        if let Some(origin) = self.dragged_from_slot.take() {
            if self.slot_is_empty(world, origin) && self.set_slot_stack(world, origin, Some(stack.clone())) {
                return;
            }
        }

        if let Some(leftover) = self.inventory.add_item(stack) {
            if let Some(position) = self.active_nvcrafter_position() {
                world.queue_item_drop(position, leftover);
            }
        }
    }

    fn update_block_break(&mut self, world: &mut World, input: &InputState, dt: f32) {
        if !input.is_mouse_held(MouseButton::Left) {
            self.break_state.reset();
            return;
        }

        let Some(hit) = self.target else {
            self.break_state.reset();
            return;
        };

        let active_stack = self.inventory.active_stack().cloned();
        if !self.enforce_active_tool_break_gate(hit.block_type, active_stack.as_ref()) {
            return;
        }

        let Some(mining) = mining_resolution(hit.block_type, active_stack.as_ref()) else {
            self.break_state.reset();
            return;
        };

        if self.break_state.target != Some(hit.block_pos)
            || self.break_state.tool != mining.tool_block
            || (self.break_state.required_time - mining.required_time).abs() > f32::EPSILON
        {
            self.break_state.target = Some(hit.block_pos);
            self.break_state.progress = 0.0;
            self.break_state.required_time = mining.required_time;
            self.break_state.tool = mining.tool_block;
        }

        self.break_state.progress += dt;
        if self.break_state.progress < self.break_state.required_time {
            return;
        }

        let did_break = if hit.block_type == BlockType::TreeTrunk {
            if mining.harvest_allowed {
                self.harvest_connected_tree(world, hit.block_pos);
                true
            } else {
                self.destroy_block_with_active_tool_gate(world, hit.block_pos).is_some()
            }
        } else if let Some(block) = self.destroy_block_with_active_tool_gate(world, hit.block_pos) {
            if mining.harvest_allowed {
                self.collect_destroy_drop(block, hit.block_pos);
            }
            true
        } else {
            false
        };

        if did_break && hit.block_type == BlockType::NVCrafter {
            self.collect_world_drops_at(world, hit.block_pos);
        }

        if did_break && mining.consume_tool_durability {
            let _ = self.inventory.damage_active_tool(1);
        }
        self.break_state.reset();
    }

    fn harvest_connected_tree(&mut self, world: &mut World, root: Vector3<i32>) {
        let trunk_positions = collect_trunk_positions(world, root);
        let leaf_positions = collect_leaf_positions(world, &trunk_positions);

        for position in trunk_positions {
            if self.destroy_block_with_active_tool_gate(world, position).is_none() {
                continue;
            }

            self.collect_item_drop(BlockType::TreeTrunk);
        }

        for position in leaf_positions {
            if self.destroy_block_with_active_tool_gate(world, position).is_none() {
                continue;
            }

            self.collect_item_drop(BlockType::TreeLeaves);
            self.collect_leaf_bonus_drops(position);
        }
    }

    fn collect_leaf_bonus_drops(&mut self, position: Vector3<i32>) {
        if drop_roll(position, 0x9E37_79B9_7F4A_7C15) % SAPLING_DROP_ODDS == 0 {
            self.collect_item_drop(BlockType::Sapling);
        }

        if drop_roll(position, 0xD1B5_4A32_D192_ED03) % STICK_DROP_ODDS == 0 {
            self.collect_item_drop(BlockType::Stick);
        }
    }

    fn collect_destroy_drop(&mut self, block: BlockType, position: Vector3<i32>) {
        if let Some(drop) = destroyed_block_drop(block, position) {
            self.collect_item_drop(drop);
        }
    }

    fn collect_item_drop(&mut self, item: BlockType) {
        let Some(stack) = ItemStack::from_inventory_item(item) else {
            return;
        };
        let _ = self.inventory.add_item(stack);
    }

    fn collect_world_drops_at(&mut self, world: &mut World, position: Vector3<i32>) {
        for stack in world.drain_item_drops_at(position) {
            if let Some(leftover) = self.inventory.add_item(stack) {
                world.queue_item_drop(position, leftover);
            }
        }
    }

    fn handle_right_click(&mut self, world: &mut World, camera: &Camera) {
        let Some(hit) = self.target else {
            return;
        };

        if hit.block_type.has_gui() {
            if hit.block_type == BlockType::NVCrafter {
                self.open_nvcrafter_gui(world, hit.block_pos);
            }
            return;
        }

        self.place_selected_block(world, camera);
    }

    fn place_selected_block(&mut self, world: &mut World, camera: &Camera) {
        let Some(hit) = self.target else { return; };
        let Some(stack) = self.inventory.active_stack() else { return; };
        let Some(block) = stack.block_type else { return; };

        let place_pos = hit.block_pos + hit.face_normal;
        if camera.player_bounds().intersects_block(place_pos.x, place_pos.y, place_pos.z) {
            return;
        }

        if world.place_block(place_pos, block) {
            let _ = self.inventory.consume_active_item(1);
        }
    }
}

fn collect_trunk_positions(world: &World, root: Vector3<i32>) -> Vec<Vector3<i32>> {
    let mut queued = HashSet::new();
    let mut trunk_queue = VecDeque::new();
    let mut trunk_positions = Vec::new();

    queued.insert(block_key(root));
    trunk_queue.push_back(root);

    while let Some(position) = trunk_queue.pop_front() {
        if trunk_positions.len() >= TREE_TRUNK_SEARCH_LIMIT {
            break;
        }

        if world.get_block(position.x, position.y, position.z) != BlockType::TreeTrunk {
            continue;
        }

        trunk_positions.push(position);

        for offset in TRUNK_NEIGHBOR_OFFSETS {
            let neighbor = offset_position(position, offset);
            if queued.insert(block_key(neighbor))
                && world.get_block(neighbor.x, neighbor.y, neighbor.z) == BlockType::TreeTrunk
            {
                trunk_queue.push_back(neighbor);
            }
        }
    }

    trunk_positions
}

fn collect_leaf_positions(world: &World, trunk_positions: &[Vector3<i32>]) -> Vec<Vector3<i32>> {
    let mut leaf_positions = Vec::new();
    let mut collected_leaves = HashSet::new();

    for &trunk_position in trunk_positions {
        let mut queued = HashSet::new();
        let mut leaf_queue = VecDeque::new();
        let mut visited_leaf_count = 0usize;

        for offset in LEAF_NEIGHBOR_OFFSETS {
            let neighbor = offset_position(trunk_position, offset);
            if queued.insert(block_key(neighbor))
                && world.get_block(neighbor.x, neighbor.y, neighbor.z) == BlockType::TreeLeaves
            {
                leaf_queue.push_back(neighbor);
            }
        }

        while let Some(position) = leaf_queue.pop_front() {
            if visited_leaf_count >= TREE_LEAF_SEARCH_LIMIT {
                break;
            }

            if world.get_block(position.x, position.y, position.z) != BlockType::TreeLeaves {
                continue;
            }

            visited_leaf_count += 1;
            if collected_leaves.insert(block_key(position)) {
                leaf_positions.push(position);
            }

            for offset in LEAF_NEIGHBOR_OFFSETS {
                let neighbor = offset_position(position, offset);
                if queued.insert(block_key(neighbor))
                    && world.get_block(neighbor.x, neighbor.y, neighbor.z) == BlockType::TreeLeaves
                {
                    leaf_queue.push_back(neighbor);
                }
            }
        }
    }

    leaf_positions
}

fn offset_position(position: Vector3<i32>, offset: (i32, i32, i32)) -> Vector3<i32> {
    Vector3::new(position.x + offset.0, position.y + offset.1, position.z + offset.2)
}

fn block_key(position: Vector3<i32>) -> (i32, i32, i32) {
    (position.x, position.y, position.z)
}

fn held_tool_power(active_stack: Option<&ItemStack>) -> u8 {
    active_stack
        .map(ItemStack::tool_power)
        .unwrap_or(crate::world::block::ToolTier::Hand.power())
}

fn block_break_hardness(block: BlockType) -> u8 {
    block.hardness().min(u32::from(u8::MAX)) as u8
}

fn active_tool_satisfies_break_gate(block: BlockType, active_stack: Option<&ItemStack>) -> bool {
    let tool_power = held_tool_power(active_stack);
    if tool_power < block_break_hardness(block) {
        return false;
    }

    true
}

fn mining_resolution(block: BlockType, active_stack: Option<&ItemStack>) -> Option<MiningResolution> {
    let base_time = block.break_time_seconds()?;
    let tool = active_stack
        .and_then(|stack| stack.block_type)
        .and_then(|tool_block| block_tool(tool_block).map(|stats| (tool_block, stats)));
    let required_tier = block.required_tool_tier();
    let harvest_allowed = required_tier.map_or(true, |required| {
        tool.map_or(false, |(_, stats)| stats.tier >= required)
    });

    let required_time = match required_tier {
        Some(required) if tool.map_or(false, |(_, stats)| stats.tier >= required) => {
            base_time / tool.map(|(_, stats)| stats.speed_multiplier.max(1.0)).unwrap_or(1.0)
        }
        Some(_) => base_time * 2.5,
        None => base_time,
    };

    Some(MiningResolution {
        required_time,
        harvest_allowed,
        consume_tool_durability: required_tier.is_some() && tool.is_some(),
        tool_block: tool.map(|(tool_block, _)| tool_block),
    })
}

fn block_tool(block: BlockType) -> Option<crate::world::block::ToolStats> {
    block.tool_stats()
}

fn destroyed_block_drop(block: BlockType, position: Vector3<i32>) -> Option<BlockType> {
    if !block.is_inventory_item() {
        return None;
    }

    if block == BlockType::Gravel && gravel_drops_flint(position) {
        Some(BlockType::Flint)
    } else {
        Some(block)
    }
}

fn gravel_drops_flint(position: Vector3<i32>) -> bool {
    drop_roll(position, 0xA24B_AED4_963E_E40B) % FLINT_DROP_ODDS == 0
}

fn drop_roll(position: Vector3<i32>, salt: u64) -> u64 {
    let mut value = (position.x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (position.y as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ (position.z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB)
        ^ salt;

    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG_BLOCKS: [BlockType; 6] = [
        BlockType::TreeTrunk,
        BlockType::NeedleWood,
        BlockType::WarmWood,
        BlockType::WetWood,
        BlockType::PaleWood,
        BlockType::DarkWood,
    ];

    fn equip_tool(controller: &mut InteractionController, tool: BlockType) {
        controller.inventory.set_slot(
            HOTBAR_START,
            Some(ItemStack::from_inventory_item(tool).expect("tool should exist")),
        );
    }

    fn seed_tree(world: &mut World) -> (Vector3<i32>, Vector3<i32>, Vector3<i32>) {
        let root = Vector3::new(0, 64, 0);
        let neighbor_trunk = Vector3::new(1, 64, 0);
        let leaf = Vector3::new(0, 65, 0);
        world.set_block(root.x, root.y, root.z, BlockType::TreeTrunk);
        world.set_block(neighbor_trunk.x, neighbor_trunk.y, neighbor_trunk.z, BlockType::TreeTrunk);
        world.set_block(leaf.x, leaf.y, leaf.z, BlockType::TreeLeaves);
        (root, neighbor_trunk, leaf)
    }

    fn inventory_contains(controller: &InteractionController, block: BlockType) -> bool {
        (0..INVENTORY_SLOT_COUNT).any(|index| {
            controller
                .inventory
                .slot(index)
                .is_some_and(|stack| stack.block_type == Some(block))
        })
    }

    fn held_left_click() -> InputState {
        let mut input = InputState::default();
        input.mouse_buttons_held.insert(MouseButton::Left);
        input
    }

    fn hit_at(position: Vector3<i32>, block_type: BlockType) -> RaycastHit {
        RaycastHit {
            block_pos: position,
            face_normal: Vector3::new(0, 1, 0),
            block_type,
        }
    }

    #[test]
    fn trunk_collection_only_follows_cardinal_connections() {
        let mut world = World::new(123);
        let root = Vector3::new(0, 120, 0);

        world.set_block(root.x, root.y, root.z, BlockType::TreeTrunk);
        world.set_block(1, 120, 0, BlockType::TreeTrunk);
        world.set_block(0, 121, 0, BlockType::TreeTrunk);
        world.set_block(0, 120, 1, BlockType::TreeTrunk);
        world.set_block(1, 121, 1, BlockType::TreeTrunk);

        let trunks = collect_trunk_positions(&world, root);

        assert!(trunks.contains(&root));
        assert!(trunks.contains(&Vector3::new(1, 120, 0)));
        assert!(trunks.contains(&Vector3::new(0, 121, 0)));
        assert!(trunks.contains(&Vector3::new(0, 120, 1)));
        assert!(!trunks.contains(&Vector3::new(1, 121, 1)));
    }

    #[test]
    fn leaf_collection_never_searches_downward() {
        let mut world = World::new(456);
        let trunk = Vector3::new(0, 120, 0);

        world.set_block(trunk.x, trunk.y, trunk.z, BlockType::TreeTrunk);
        world.set_block(0, 121, 0, BlockType::TreeLeaves);
        world.set_block(1, 121, 0, BlockType::TreeLeaves);
        world.set_block(1, 120, 0, BlockType::TreeLeaves);
        world.set_block(1, 119, 0, BlockType::TreeLeaves);
        world.set_block(0, 119, 0, BlockType::TreeLeaves);

        let leaves = collect_leaf_positions(&world, &[trunk]);

        assert!(leaves.contains(&Vector3::new(0, 121, 0)));
        assert!(leaves.contains(&Vector3::new(1, 121, 0)));
        assert!(leaves.contains(&Vector3::new(1, 120, 0)));
        assert!(!leaves.contains(&Vector3::new(1, 119, 0)));
        assert!(!leaves.contains(&Vector3::new(0, 119, 0)));
    }

    #[test]
    fn mineral_breaks_require_matching_tool_tier_for_harvests() {
        let hand = mining_resolution(BlockType::Stone, None).expect("stone should be breakable");
        let flint_pickaxe =
            ItemStack::from_inventory_item(BlockType::FlintPickaxe).expect("tool should be inventory item");

        let with_tool =
            mining_resolution(BlockType::Stone, Some(&flint_pickaxe)).expect("stone should be breakable");

        assert!(!hand.harvest_allowed);
        assert!(with_tool.harvest_allowed);
        assert!(with_tool.required_time < hand.required_time);
    }

    #[test]
    fn higher_tier_ores_reject_lower_tier_tools() {
        let stone_pickaxe =
            ItemStack::from_inventory_item(BlockType::StonePickaxe).expect("tool should be inventory item");
        let iron_pickaxe =
            ItemStack::from_inventory_item(BlockType::IronPickaxe).expect("tool should be inventory item");

        assert!(!mining_resolution(BlockType::DiamondOre, Some(&stone_pickaxe))
            .expect("diamond ore should be breakable")
            .harvest_allowed);
        assert!(mining_resolution(BlockType::DiamondOre, Some(&iron_pickaxe))
            .expect("diamond ore should be breakable")
            .harvest_allowed);
    }

    #[test]
    fn hand_power_only_matches_hand_breakable_blocks() {
        assert_eq!(held_tool_power(None), 1);
        assert_eq!(block_break_hardness(BlockType::Dirt), 1);
        assert_eq!(block_break_hardness(BlockType::TreeTrunk), 5);
        assert_eq!(block_break_hardness(BlockType::Stone), 2);

        assert!(held_tool_power(None) >= block_break_hardness(BlockType::Dirt));
        assert!(held_tool_power(None) < block_break_hardness(BlockType::TreeTrunk));
        assert!(held_tool_power(None) < block_break_hardness(BlockType::Stone));
        assert!(held_tool_power(None) < block_break_hardness(BlockType::IronOre));
    }

    #[test]
    fn log_blocks_require_iron_or_better_power() {
        let flint_pickaxe =
            ItemStack::from_inventory_item(BlockType::FlintPickaxe).expect("tool should be inventory item");
        let stone_pickaxe =
            ItemStack::from_inventory_item(BlockType::StonePickaxe).expect("tool should be inventory item");
        let iron_pickaxe =
            ItemStack::from_inventory_item(BlockType::IronPickaxe).expect("tool should be inventory item");

        for block in LOG_BLOCKS {
            assert!(!active_tool_satisfies_break_gate(block, None), "hand should not break {}", block.name());
            assert!(
                !active_tool_satisfies_break_gate(block, Some(&flint_pickaxe)),
                "flint should not break {}",
                block.name()
            );
            assert!(
                !active_tool_satisfies_break_gate(block, Some(&stone_pickaxe)),
                "stone should not break {}",
                block.name()
            );
            assert!(
                active_tool_satisfies_break_gate(block, Some(&iron_pickaxe)),
                "iron should break {}",
                block.name()
            );
        }
    }

    #[test]
    fn flint_pickaxe_can_break_stone_but_not_logs_or_iron_ore() {
        let flint_pickaxe =
            ItemStack::from_inventory_item(BlockType::FlintPickaxe).expect("tool should be inventory item");

        let power = held_tool_power(Some(&flint_pickaxe));

        assert!(power >= block_break_hardness(BlockType::Stone));
        assert!(power < block_break_hardness(BlockType::TreeTrunk));
        assert!(power < block_break_hardness(BlockType::IronOre));
    }

    #[test]
    fn stone_pickaxe_cannot_break_logs() {
        let stone_pickaxe =
            ItemStack::from_inventory_item(BlockType::StonePickaxe).expect("tool should be inventory item");

        assert!(held_tool_power(Some(&stone_pickaxe)) < block_break_hardness(BlockType::TreeTrunk));
    }

    #[test]
    fn empty_inventory_defaults_to_hand_power() {
        let controller = InteractionController::default();

        assert_eq!(controller.inventory.active_tool_power(), 1);
        assert!(controller.active_tool_can_break_block(BlockType::Dirt));
        assert!(!controller.active_tool_can_break_block(BlockType::TreeTrunk));
        assert!(!controller.active_tool_can_break_block(BlockType::Stone));
    }

    #[test]
    fn update_block_break_prevents_hand_from_destroying_stone() {
        let mut world = World::new(789);
        let pos = Vector3::new(0, 60, 0);
        world.set_block(pos.x, pos.y, pos.z, BlockType::Stone);

        let mut controller = InteractionController::default();
        controller.target = Some(hit_at(pos, BlockType::Stone));

        controller.update_block_break(&mut world, &held_left_click(), 10.0);

        assert_eq!(world.get_block(pos.x, pos.y, pos.z), BlockType::Stone);
        assert_eq!(controller.break_state.target, None);
        assert_eq!(controller.break_state.progress, 0.0);
    }

    #[test]
    fn update_block_break_allows_hand_to_destroy_dirt() {
        let mut world = World::new(790);
        let pos = Vector3::new(0, 60, 0);
        world.set_block(pos.x, pos.y, pos.z, BlockType::Dirt);

        let mut controller = InteractionController::default();
        controller.target = Some(hit_at(pos, BlockType::Dirt));

        controller.update_block_break(&mut world, &held_left_click(), 1.0);

        assert_eq!(world.get_block(pos.x, pos.y, pos.z), BlockType::Air);
    }

    #[test]
    fn update_block_break_prevents_hand_from_destroying_logs_or_triggering_tree_fall() {
        let mut world = World::new(7901);
        let (root, neighbor_trunk, leaf) = seed_tree(&mut world);

        let mut controller = InteractionController::default();
        controller.target = Some(hit_at(root, BlockType::TreeTrunk));

        controller.update_block_break(&mut world, &held_left_click(), 10.0);

        assert_eq!(world.get_block(root.x, root.y, root.z), BlockType::TreeTrunk);
        assert_eq!(
            world.get_block(neighbor_trunk.x, neighbor_trunk.y, neighbor_trunk.z),
            BlockType::TreeTrunk
        );
        assert_eq!(world.get_block(leaf.x, leaf.y, leaf.z), BlockType::TreeLeaves);
        assert_eq!(controller.break_state.target, None);
        assert_eq!(controller.break_state.progress, 0.0);
    }

    #[test]
    fn update_block_break_prevents_flint_pickaxe_from_destroying_logs_or_triggering_tree_fall() {
        let mut world = World::new(7902);
        let (root, neighbor_trunk, leaf) = seed_tree(&mut world);

        let mut controller = InteractionController::default();
        equip_tool(&mut controller, BlockType::FlintPickaxe);
        controller.target = Some(hit_at(root, BlockType::TreeTrunk));

        controller.update_block_break(&mut world, &held_left_click(), 10.0);

        assert_eq!(world.get_block(root.x, root.y, root.z), BlockType::TreeTrunk);
        assert_eq!(
            world.get_block(neighbor_trunk.x, neighbor_trunk.y, neighbor_trunk.z),
            BlockType::TreeTrunk
        );
        assert_eq!(world.get_block(leaf.x, leaf.y, leaf.z), BlockType::TreeLeaves);
    }

    #[test]
    fn update_block_break_prevents_stone_pickaxe_from_destroying_logs_or_triggering_tree_fall() {
        let mut world = World::new(7903);
        let (root, neighbor_trunk, leaf) = seed_tree(&mut world);

        let mut controller = InteractionController::default();
        equip_tool(&mut controller, BlockType::StonePickaxe);
        controller.target = Some(hit_at(root, BlockType::TreeTrunk));

        controller.update_block_break(&mut world, &held_left_click(), 10.0);

        assert_eq!(world.get_block(root.x, root.y, root.z), BlockType::TreeTrunk);
        assert_eq!(
            world.get_block(neighbor_trunk.x, neighbor_trunk.y, neighbor_trunk.z),
            BlockType::TreeTrunk
        );
        assert_eq!(world.get_block(leaf.x, leaf.y, leaf.z), BlockType::TreeLeaves);
    }

    #[test]
    fn update_block_break_prevents_flint_pickaxe_from_destroying_iron_ore() {
        let mut world = World::new(791);
        let pos = Vector3::new(0, 60, 0);
        world.set_block(pos.x, pos.y, pos.z, BlockType::IronOre);

        let mut controller = InteractionController::default();
        equip_tool(&mut controller, BlockType::FlintPickaxe);
        let initial_durability = controller
            .inventory
            .active_stack()
            .and_then(|stack| stack.durability)
            .expect("tool should have durability");
        controller.target = Some(hit_at(pos, BlockType::IronOre));

        controller.update_block_break(&mut world, &held_left_click(), 10.0);

        assert_eq!(world.get_block(pos.x, pos.y, pos.z), BlockType::IronOre);
        assert_eq!(
            controller
                .inventory
                .active_stack()
                .and_then(|stack| stack.durability),
            Some(initial_durability)
        );
    }

    #[test]
    fn update_block_break_keeps_tree_fall_when_initial_trunk_is_breakable() {
        let mut world = World::new(792);
        let (root, neighbor_trunk, leaf) = seed_tree(&mut world);

        let mut controller = InteractionController::default();
        equip_tool(&mut controller, BlockType::IronPickaxe);
        controller.target = Some(hit_at(root, BlockType::TreeTrunk));

        controller.update_block_break(&mut world, &held_left_click(), 10.0);

        assert_eq!(world.get_block(root.x, root.y, root.z), BlockType::Air);
        assert_eq!(world.get_block(neighbor_trunk.x, neighbor_trunk.y, neighbor_trunk.z), BlockType::Air);
        assert_eq!(world.get_block(leaf.x, leaf.y, leaf.z), BlockType::Air);
    }

    #[test]
    fn gravel_drop_table_can_yield_flint_or_gravel() {
        let mut found_flint = false;
        let mut found_gravel = false;

        for x in 0..256 {
            match destroyed_block_drop(BlockType::Gravel, Vector3::new(x, 80, 0)) {
                Some(BlockType::Flint) => found_flint = true,
                Some(BlockType::Gravel) => found_gravel = true,
                _ => {}
            }

            if found_flint && found_gravel {
                break;
            }
        }

        assert!(found_flint);
        assert!(found_gravel);
    }

    #[test]
    fn player_crafting_output_click_does_not_consume_inputs_when_inventory_is_full() {
        let mut world = World::new(2026);
        let mut controller = InteractionController::default();

        for index in 0..INVENTORY_SLOT_COUNT {
            let mut stack = ItemStack::from_inventory_item(BlockType::Dirt).expect("dirt should exist");
            stack.count = stack.max_stack;
            controller.inventory.set_slot(index, Some(stack));
        }

        controller.inventory.set_crafting_slot(
            0,
            Some(ItemStack::from_inventory_item(BlockType::TreeTrunk).expect("log should exist")),
            &controller.recipes,
        );

        assert_eq!(
            controller.inventory.crafting_output.as_ref().and_then(|stack| stack.block_type),
            Some(BlockType::Planks)
        );

        controller.handle_output_click(&mut world, UiSlotId::PlayerCraftingOutput);

        assert_eq!(
            controller.inventory.crafting_output.as_ref().and_then(|stack| stack.block_type),
            Some(BlockType::Planks)
        );
        assert_eq!(
            controller.inventory.crafting_slot(0).as_ref().and_then(|stack| stack.block_type),
            Some(BlockType::TreeTrunk)
        );
        assert!(!inventory_contains(&controller, BlockType::Planks));
    }

    #[test]
    fn opening_nvcrafter_gui_creates_accessible_world_state() {
        let mut world = World::new(2026);
        let mut controller = InteractionController::default();
        let position = Vector3::new(6, 64, 6);
        assert!(world.place_block(position, BlockType::NVCrafter));

        controller.open_nvcrafter_gui(&mut world, position);

        assert!(controller.inventory_open());
        assert_eq!(controller.gui_type(), Some(GuiType::NVCrafter));
        assert!(world.nvcrafter_state(position).is_some());
    }

    #[test]
    fn nvcrafter_output_click_moves_result_into_player_inventory() {
        let mut world = World::new(2026);
        let mut controller = InteractionController::default();
        let position = Vector3::new(8, 64, 8);
        assert!(world.place_block(position, BlockType::NVCrafter));
        controller.open_nvcrafter_gui(&mut world, position);

        let recipe_slots = [0usize, 1, 2, 3, 5, 6, 7, 8];
        let crafter = world
            .nvcrafter_state_mut(position)
            .expect("crafter state should exist after opening gui");
        for slot in recipe_slots {
            crafter.set_slot(
                slot,
                Some(ItemStack::from_inventory_item(BlockType::Planks).expect("planks should exist")),
                &controller.recipes,
            );
        }
        crafter.set_slot(
            4,
            Some(ItemStack::from_inventory_item(BlockType::TreeTrunk).expect("log should exist")),
            &controller.recipes,
        );

        assert_eq!(
            world
                .nvcrafter_state(position)
                .and_then(|state| state.output.as_ref())
                .and_then(|stack| stack.block_type),
            Some(BlockType::NVCrafter)
        );

        controller.handle_output_click(&mut world, UiSlotId::NVCrafterOutput);

        assert!(inventory_contains(&controller, BlockType::NVCrafter));
        let state = world.nvcrafter_state(position).expect("crafter state should persist");
        assert!(state.output.is_none());
        assert!((0..state.grid.active_len()).all(|index| state.slot(index).is_none()));
    }
}