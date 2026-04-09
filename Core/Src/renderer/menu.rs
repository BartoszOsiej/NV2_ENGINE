use anyhow::Result;

use super::{
    text::{TextAlignment, TextRenderer},
    UiPanel,
    UiMode,
};

const MENU_TITLE_MAIN: &str = "NVENGINE";
const MENU_TITLE_PAUSE: &str = "PAUSED";
const MENU_ITEMS_MAIN: [&str; 3] = ["NEW GAME", "LOAD/SAVE", "QUIT"];
const MENU_ITEMS_PAUSE: [&str; 4] = ["RESUME", "SAVE", "SAVE + EXIT", "EXIT"];

const WHITE: [u8; 4] = [255, 255, 255, 255];
const SELECTED_TEXT: [u8; 4] = [22, 24, 28, 255];
const MENU_HIGHLIGHT_FILL: [f32; 4] = [0.32, 0.27, 0.10, 0.72];
const MENU_HIGHLIGHT_BORDER: [f32; 4] = [1.0, 0.95, 0.55, 0.96];

pub struct MenuRenderer {
    item_spacing_px: f32,
}

impl MenuRenderer {
    pub fn new() -> Self {
        Self {
            item_spacing_px: 40.0,
        }
    }

    pub fn render_menu(
        &self,
        text_renderer: &mut TextRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mode: UiMode,
        selected_index: Option<usize>,
    ) -> Result<()> {
        let Some(menu) = MenuDefinition::for_mode(mode) else {
            return Ok(());
        };

        let (screen_w, screen_h) = text_renderer.screen_size();
        let center_x = screen_w as f32 * 0.5;
        let title_y = (screen_h as f32 * 0.18).round();
        text_renderer.draw_text_tinted(
            device,
            queue,
            center_x,
            title_y,
            2.2,
            menu.title,
            TextAlignment::Center,
            WHITE,
        )?;

        let selected = selected_index.unwrap_or(0).min(menu.items.len().saturating_sub(1));
        let total_height = self.item_spacing_px * menu.items.len().saturating_sub(1) as f32;
        let mut y = screen_h as f32 * 0.5 - total_height * 0.5;

        for (index, item) in menu.items.iter().enumerate() {
            let color = if index == selected { SELECTED_TEXT } else { WHITE };
            text_renderer.draw_text_tinted(
                device,
                queue,
                center_x,
                y.round(),
                1.2,
                item,
                TextAlignment::Center,
                color,
            )?;
            y += self.item_spacing_px;
        }

        Ok(())
    }

    pub fn build_menu_panels(
        &self,
        text_renderer: &TextRenderer,
        mode: UiMode,
        selected_index: Option<usize>,
    ) -> Vec<UiPanel> {
        let Some(menu) = MenuDefinition::for_mode(mode) else {
            return Vec::new();
        };

        let (screen_w, screen_h) = text_renderer.screen_size();
        let selected = selected_index.unwrap_or(0).min(menu.items.len().saturating_sub(1));
        let mut max_width: f32 = 260.0;
        let mut item_height: f32 = 34.0;
        for item in menu.items {
            if let Some((width, height)) = text_renderer.measure_text_size(item, 1.2) {
                max_width = max_width.max(width);
                item_height = item_height.max(height);
            }
        }

        let total_height = self.item_spacing_px * menu.items.len().saturating_sub(1) as f32;
        let start_y = screen_h as f32 * 0.5 - total_height * 0.5;
        let item_y = start_y + selected as f32 * self.item_spacing_px;
        let width = (max_width + 84.0).min(screen_w as f32 - 80.0);
        let height = (item_height + 18.0).max(48.0);

        vec![UiPanel {
            x: screen_w as f32 * 0.5 - width * 0.5,
            y: item_y - 9.0,
            width,
            height,
            fill: MENU_HIGHLIGHT_FILL,
            border_color: MENU_HIGHLIGHT_BORDER,
            border_thickness: 2.0,
        }]
    }
}

struct MenuDefinition {
    title: &'static str,
    items: &'static [&'static str],
}

impl MenuDefinition {
    fn for_mode(mode: UiMode) -> Option<Self> {
        match mode {
            UiMode::MainMenu => Some(Self {
                title: MENU_TITLE_MAIN,
                items: &MENU_ITEMS_MAIN,
            }),
            UiMode::PauseMenu => Some(Self {
                title: MENU_TITLE_PAUSE,
                items: &MENU_ITEMS_PAUSE,
            }),
            UiMode::None => None,
        }
    }
}