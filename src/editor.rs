mod brush_size;
pub mod key_combo;
mod map;
mod notification;
pub mod serialize;
mod sprite;
mod undo_redo;

use crate::app::ElmApp;
use crate::editor::notification::Notification;
use crate::runtime::flags::Flags;
use crate::runtime::map::Map;
use crate::runtime::sprite_sheet::{Color, Sprite, SpriteSheet};
use crate::ui::button::{self, Button};
use crate::ui::{
    cursor::{self, Cursor},
    text::Text,
};
use crate::ui::{DrawFn, Element, Tree};
use crate::Resources;
use crate::{Event, Key, KeyState, KeyboardEvent};
use brush_size::BrushSize;
use serialize::serialize;

use self::key_combo::KeyCombos;
use self::serialize::{Ppm, Serialize};
use self::undo_redo::{Command, Commands};

#[derive(Debug)]
pub(crate) struct Editor {
    cursor: cursor::State,
    tab: Tab,
    selected_sprite_page: usize,
    sprite_button_state: button::State,
    map_button_state: button::State,
    tab_buttons: [button::State; 4],
    sprite_buttons: Vec<button::State>,
    selected_tool: usize,
    tool_buttons: Vec<button::State>,
    bottom_bar_text: String,
    notification: notification::State,
    key_combos: KeyCombos<KeyComboAction>,
    clipboard: Clipboard,
    commands: Commands,
    editor_sprites: SpriteSheet,
    map_editor: map::Editor,
    sprite_editor: sprite::Editor,
    brush_size: BrushSize,
    selected_sprite: usize,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Tab {
    SpriteEditor,
    MapEditor,
}

impl Tab {
    fn previous(self) -> Self {
        match self {
            Self::SpriteEditor => Self::MapEditor,
            Self::MapEditor => Self::SpriteEditor,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::SpriteEditor => Self::MapEditor,
            Self::MapEditor => Self::SpriteEditor,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Msg {
    SpriteTabClicked,
    MapButtonClicked,
    ColorHovered(Color),
    SpritePageSelected(usize),
    SpriteButtonClicked(usize),
    FlagToggled(usize),
    FlagHovered { bit_number: usize },
    SpriteEdited { x: usize, y: usize, color: Color }, // TODO: Improve
    ToolSelected(usize),
    ClickedMapTile { x: usize, y: usize },
    KeyboardEvent(KeyboardEvent),
    BrushSizeSliderHovered,
    BrushSizeSelected(BrushSize),
    MapEditorMsg(map::Msg),
    SpriteEditorMsg(sprite::Msg),
}

impl Editor {
    fn shift_sprite(&mut self, shift_direction: ShiftDirection, sprite_sheet: &mut SpriteSheet) {
        let sprite = sprite_sheet.get_sprite_mut(self.selected_sprite);
        shift_direction.shift(sprite);
    }

    fn handle_key_combos(&mut self, key_event: KeyboardEvent, resources: &mut Resources) {
        self.key_combos.on_event(key_event, |action| {
            handle_key_combo(
                action,
                self.selected_sprite,
                &mut self.notification,
                &mut self.clipboard,
                resources,
                &mut self.commands,
                &mut self.tab,
            );
        })
    }
}

#[derive(Debug)]
struct Clipboard {
    data: Vec<Color>,
}
impl Clipboard {
    fn new() -> Self {
        Self { data: vec![0; 64] }
    }

    fn copy_sprite(&mut self, sprite: &Sprite) {
        self.data = sprite.to_owned();
    }

    fn paste_into(&self, sprite: &mut Sprite) {
        for (sprite_pixel, clipboard_pixel) in sprite.iter_mut().zip(self.data.iter().copied()) {
            *sprite_pixel = clipboard_pixel;
        }
    }
}

fn handle_key_combo(
    key_combo: KeyComboAction,
    selected_sprite: usize,
    notification: &mut notification::State,
    clipboard: &mut Clipboard,
    resources: &mut Resources,
    commands: &mut Commands,
    tab: &mut Tab,
) {
    match key_combo {
        KeyComboAction::Copy => {
            let sprite = resources.sprite_sheet.get_sprite(selected_sprite);
            notification.alert("COPIED 1 X 1 SPRITES".to_owned());
            clipboard.copy_sprite(sprite);
        }
        KeyComboAction::Paste => {
            let sprite = resources.sprite_sheet.get_sprite_mut(selected_sprite);
            notification.alert("PASTED 1 X 1 SPRITES".to_owned());

            clipboard.paste_into(sprite);
        }
        KeyComboAction::FlipVertically => {
            let sprite = resources.sprite_sheet.get_sprite_mut(selected_sprite);

            sprite.flip_vertically()
        }
        KeyComboAction::FlipHorizontally => {
            let sprite = resources.sprite_sheet.get_sprite_mut(selected_sprite);

            sprite.flip_horizontally()
        }
        KeyComboAction::Undo => {
            commands.undo(notification, &mut resources.sprite_sheet);
        }
        KeyComboAction::Redo => {
            commands.redo(notification, &mut resources.sprite_sheet);
        }
        KeyComboAction::Save => {
            save(notification, resources);
        }
        KeyComboAction::PreviousTab => {
            *tab = tab.previous();
        }
        KeyComboAction::NextTab => {
            *tab = tab.next();
        }
    }
}

fn save(notification: &mut notification::State, resources: &Resources) {
    notification.alert("SAVED".to_owned());

    let map_ppm = Ppm::from_map(&resources.map, &resources.sprite_sheet);
    let sprite_sheet_ppm = Ppm::from_sprite_sheet(&resources.sprite_sheet);
    let to_serialize: &[(&str, &dyn Serialize)] = &[
        (&Flags::file_name(), &resources.sprite_flags),
        (&SpriteSheet::file_name(), &resources.sprite_sheet),
        (&Map::file_name(), &resources.map),
        ("map.ppm", &map_ppm),
        ("sprite_sheet.ppm", &sprite_sheet_ppm),
    ];

    for (name, serializable) in to_serialize.iter() {
        serialize(&resources.assets_path, name, serializable);
    }
}

#[derive(Copy, Clone, Debug)]
enum KeyComboAction {
    Copy,
    Paste,
    FlipVertically,
    FlipHorizontally,
    Undo,
    Redo,
    Save,
    PreviousTab,
    NextTab,
}

fn load_editor_sprite_sheet() -> Result<SpriteSheet, String> {
    let editor_sprites = std::fs::read_to_string("./src/editor/sprite_sheet.txt")
        .map_err(|_| "Couldn't find editor sprite sheet file.".to_owned())?;

    SpriteSheet::deserialize(&editor_sprites)
        .map_err(|_| "Couldn't parse editor sprite sheet.".to_owned())
}

impl ElmApp for Editor {
    type Msg = Msg;

    fn init() -> Self {
        Self {
            cursor: cursor::State::new(),
            sprite_button_state: button::State::new(),
            map_button_state: button::State::new(),
            tab: Tab::SpriteEditor,
            selected_sprite_page: 0,
            tab_buttons: [
                button::State::new(),
                button::State::new(),
                button::State::new(),
                button::State::new(),
            ],
            sprite_buttons: vec![button::State::new(); 64],
            selected_tool: 0,
            tool_buttons: vec![button::State::new(); 2],
            bottom_bar_text: "".to_owned(),
            notification: notification::State::new(),
            key_combos: KeyCombos::new()
                .push(KeyComboAction::Copy, Key::C, &[Key::Control])
                .push(KeyComboAction::Paste, Key::V, &[Key::Control])
                .push(KeyComboAction::Undo, Key::Z, &[Key::Control])
                .push(KeyComboAction::Redo, Key::Y, &[Key::Control])
                .push(KeyComboAction::Save, Key::S, &[Key::Control])
                .push(KeyComboAction::FlipVertically, Key::V, &[])
                .push(KeyComboAction::FlipHorizontally, Key::F, &[])
                .push(KeyComboAction::PreviousTab, Key::LeftArrow, &[Key::Alt])
                .push(KeyComboAction::NextTab, Key::RightArrow, &[Key::Alt]),
            clipboard: Clipboard::new(),
            commands: Commands::new(),
            editor_sprites: load_editor_sprite_sheet()
                // TODO: Change this to actually crash if it failed.
                .unwrap_or_else(|_| SpriteSheet::new()),
            map_editor: map::Editor::new(),
            sprite_editor: sprite::Editor::new(),
            brush_size: BrushSize::tiny(),
            selected_sprite: 0,
        }
    }

    fn update(&mut self, msg: &Msg, resources: &mut Resources) {
        match msg {
            &Msg::SpriteEditorMsg(sprite_msg) => {
                self.sprite_editor.update(sprite_msg);
            }
            &Msg::MapEditorMsg(map_msg) => {
                self.map_editor.update(map_msg);
            }
            &Msg::KeyboardEvent(event) => {
                self.handle_key_combos(event, resources);

                match event {
                    KeyboardEvent {
                        key,
                        state: KeyState::Down,
                    } => {
                        if let Some(shift_direction) = ShiftDirection::from_key(&key) {
                            self.shift_sprite(shift_direction, &mut resources.sprite_sheet)
                        }
                    }
                    _ => {}
                }
            }
            Msg::SpriteTabClicked => {
                self.tab = Tab::SpriteEditor;
                println!("Sprite button clicked");
            }
            Msg::MapButtonClicked => {
                self.tab = Tab::MapEditor;
                println!("Map button clicked");
            }
            Msg::SpritePageSelected(selected_sprite_page) => {
                self.selected_sprite_page = *selected_sprite_page;
            }
            Msg::SpriteButtonClicked(selected_sprite) => {
                self.selected_sprite = *selected_sprite;
            }
            Msg::FlagHovered { bit_number } => {
                self.bottom_bar_text = format!("FLAG {} (0X{:X})", bit_number, 1 << bit_number);
            }
            Msg::FlagToggled(flag_index) => {
                let flag_index = *flag_index;

                let flag_value = resources
                    .sprite_flags
                    .fget_n(self.selected_sprite, flag_index as u8);
                resources
                    .sprite_flags
                    .fset(self.selected_sprite, flag_index, !flag_value);
            }
            &Msg::SpriteEdited { x, y, color } => {
                let sprite = resources.sprite_sheet.get_sprite_mut(self.selected_sprite);
                let x = x as isize;
                let y = y as isize;
                let previous_color = sprite.pget(x, y);

                self.commands.push(Command::pixel_changed(
                    self.selected_sprite,
                    x,
                    y,
                    previous_color,
                    color,
                ));

                for (x, y) in self
                    .brush_size
                    .iter()
                    .map(|(local_x, local_y)| (local_x + x, local_y + y))
                {
                    sprite.pset(x, y, color);
                }
            }
            &Msg::ToolSelected(selected_tool) => {
                self.selected_tool = selected_tool;
            }
            &Msg::ColorHovered(color) => {
                self.bottom_bar_text = format!("COLOUR {}", color);
            }

            &Msg::ClickedMapTile { x, y } => {
                resources.map.mset(x, y, self.selected_sprite as u8);
            }
            &Msg::BrushSizeSelected(brush_size) => {
                self.brush_size = brush_size;
                self.bottom_bar_text =
                    format!("BRUSH SIZE: {}", self.brush_size.to_human_readable());
            }
            &Msg::BrushSizeSliderHovered => {
                self.bottom_bar_text =
                    format!("BRUSH SIZE: {}", self.brush_size.to_human_readable());
            }
        }
    }

    fn view(&mut self, resources: &Resources) -> Element<'_, Msg> {
        const BACKGROUND: u8 = 5;

        Tree::new()
            .push(DrawFn::new(|draw| {
                draw.rectfill(0, 0, 127, 127, BACKGROUND)
            }))
            .push(top_bar(
                &mut self.sprite_button_state,
                &mut self.map_button_state,
                self.tab,
            ))
            .push(match self.tab {
                Tab::SpriteEditor => {
                    let selected_sprite_flags =
                        resources.sprite_flags.get(self.selected_sprite).unwrap();
                    let selected_sprite = resources.sprite_sheet.get_sprite(self.selected_sprite);

                    self.sprite_editor.view(
                        selected_sprite_flags,
                        selected_sprite,
                        &self.editor_sprites,
                        self.brush_size,
                        &Msg::SpriteEditorMsg,
                    )
                }
                Tab::MapEditor => Tree::new()
                    .push(self.map_editor.view(
                        &resources.map,
                        0,
                        8,
                        &|x, y| Msg::ClickedMapTile { x, y },
                        &Msg::MapEditorMsg,
                    ))
                    .into(),
            })
            .push(tools_row(
                76,
                self.selected_sprite,
                self.selected_sprite_page,
                &mut self.tab_buttons,
                self.selected_tool,
                &mut self.tool_buttons,
            ))
            .push(sprite_view(
                self.selected_sprite,
                self.selected_sprite_page,
                &mut self.sprite_buttons,
                87,
            ))
            .push(bottom_bar(&self.bottom_bar_text))
            .push(Cursor::new(&mut self.cursor))
            .push(Notification::new(&mut self.notification))
            .into()
    }

    fn subscriptions(&self, event: &Event) -> Vec<Msg> {
        match event {
            Event::Mouse(_) => None,
            Event::Keyboard(event) => Some(Msg::KeyboardEvent(*event)),
            Event::Tick { .. } => None,
        }
        .into_iter()
        .chain(match self.tab {
            Tab::MapEditor => map::Editor::subscriptions(event)
                .map(Msg::MapEditorMsg)
                .into_iter(),
            _ => None.into_iter(),
        })
        .collect()
    }
}

fn top_bar<'a>(
    sprite_button_state: &'a mut button::State,
    map_button_state: &'a mut button::State,
    tab: Tab,
) -> Element<'a, Msg> {
    Tree::new()
        .push(DrawFn::new(|draw| {
            draw.rectfill(0, 0, 127, 7, 8);
        }))
        .push(sprite_editor_button(sprite_button_state, tab))
        .push(map_editor_button(map_button_state, tab))
        .into()
}

fn sprite_editor_button(state: &mut button::State, tab: Tab) -> Element<'_, Msg> {
    let selected = tab == Tab::SpriteEditor;

    editor_button(state, 63, 110, 0, Msg::SpriteTabClicked, selected)
}

fn map_editor_button(state: &mut button::State, tab: Tab) -> Element<'_, Msg> {
    let selected = tab == Tab::MapEditor;

    editor_button(state, 62, 118, 0, Msg::MapButtonClicked, selected)
}

fn editor_button(
    state: &mut button::State,
    sprite: usize,
    x: i32,
    y: i32,
    msg: Msg,
    selected: bool,
) -> Element<'_, Msg> {
    Button::new(
        x,
        y,
        8,
        8,
        Some(msg),
        state,
        DrawFn::new(move |draw| {
            let color = if selected { 15 } else { 2 };

            draw.pal(15, color);
            draw.spr(sprite, 0, 0);
            draw.pal(15, 15);
        }),
    )
    .into()
}

fn tools_row<'a>(
    y: i32,
    sprite: usize,
    selected_tab: usize,
    tab_buttons: &'a mut [button::State],
    selected_tool: usize,
    tool_buttons: &'a mut [button::State],
) -> Element<'a, Msg> {
    let mut children = vec![DrawFn::new(move |draw| {
        const HEIGHT: i32 = 11;
        draw.rectfill(0, y, 127, y + HEIGHT - 1, 5)
    })
    .into()];

    const TOOLS: &[usize] = &[15, 31];

    for (tool_index, tool_button) in tool_buttons.iter_mut().enumerate() {
        let spr = TOOLS[tool_index];

        let x = (9 + 8 * tool_index) as i32;
        let y = y + 2;
        children.push(
            Button::new(
                x,
                y,
                8,
                8,
                Some(Msg::ToolSelected(tool_index)),
                tool_button,
                DrawFn::new(move |draw| {
                    draw.palt(Some(0));
                    if selected_tool == tool_index {
                        draw.pal(13, 7);
                    }
                    draw.spr(spr, 0, 0);
                    draw.pal(13, 13);
                }),
            )
            .into(),
        );
    }

    for (sprite_tab, tab_button_state) in tab_buttons.iter_mut().enumerate() {
        let base_sprite = if selected_tab == sprite_tab { 33 } else { 17 };

        let x = 96 + sprite_tab as i32 * 8;

        children.push(
            Button::new(
                x,
                y + 3,
                8,
                8,
                Some(Msg::SpritePageSelected(sprite_tab)),
                tab_button_state,
                DrawFn::new(move |draw| {
                    draw.palt(Some(0));
                    draw.spr(base_sprite + sprite_tab, 0, 0);
                }),
            )
            .into(),
        );
    }

    const X: i32 = 70;
    let sprite_preview = DrawFn::new(move |draw| {
        draw.palt(None);
        draw.spr(sprite, X, y + 2);
        draw.palt(Some(0));
    });
    children.push(sprite_preview.into());

    let spr_str = format!("{:0>3}", sprite);
    let sprite_number = DrawFn::new(move |draw| {
        let y = y + 2;
        draw.rectfill(X + 9, y + 1, X + 9 + 13 - 1, y + 7, 6);
        draw.print(&spr_str, X + 10, y + 2, 13);
    })
    .into();
    children.push(sprite_number);

    Tree::with_children(children).into()
}

/// The 4 rows of sprites at the bottom of the sprite editor
fn sprite_view(
    selected_sprite: usize,
    selected_tab: usize,
    sprite_buttons: &mut [button::State],
    y: i32,
) -> Element<'_, Msg> {
    let mut children = vec![DrawFn::new(move |draw| {
        draw.palt(None);
        draw.rectfill(0, y, 127, y + 32 + 1, 0);
    })
    .into()];

    let sprite_position = |sprite| {
        let index = sprite % 64;
        let row = (index / 16) as i32;
        let col = (index % 16) as i32;

        (col * 8, y + 1 + row * 8)
    };

    for (index, sprite_state) in sprite_buttons.iter_mut().enumerate() {
        let sprite = index + selected_tab * 64;

        let (x, y) = sprite_position(sprite);
        children.push(
            Button::new(
                x,
                y,
                8,
                8,
                Some(Msg::SpriteButtonClicked(sprite)),
                sprite_state,
                DrawFn::new(move |draw| {
                    draw.palt(None);
                    draw.spr(sprite, 0, 0);
                }),
            )
            .event_on_press()
            .into(),
        );
    }

    // Draw selected sprite highlight
    {
        // TODO: Fix (wrong highlight when switching pages)
        let (x, y) = sprite_position(selected_sprite);
        children.push(
            DrawFn::new(move |draw| {
                draw.rect(x - 1, y - 1, x + 8, y + 8, 7);
            })
            .into(),
        )
    }

    Tree::with_children(children).into()
}

fn bottom_bar(text: &str) -> Element<'_, Msg> {
    const X: i32 = 0;
    const Y: i32 = 121;
    const BAR_WIDTH: i32 = 128;
    const BAR_HEIGHT: i32 = 7;

    Tree::new()
        .push(DrawFn::new(|draw| {
            draw.rectfill(X, Y, X + BAR_WIDTH - 1, Y + BAR_HEIGHT - 1, 8)
        }))
        .push(Text::new(text, X + 1, Y + 1, 2))
        .into()
}

pub(crate) static MOUSE_SPRITE: &[Color] = &[
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 1, 0, 0, 0, 0, //
    0, 0, 1, 7, 1, 0, 0, 0, //
    0, 0, 1, 7, 7, 1, 0, 0, //
    0, 0, 1, 7, 7, 7, 1, 0, //
    0, 0, 1, 7, 7, 7, 7, 1, //
    0, 0, 1, 7, 7, 1, 1, 0, //
    0, 0, 0, 1, 1, 7, 1, 0, //
];

// static MOUSE_TARGET_SPRITE: &[Color] = &[
//     0, 0, 0, 1, 0, 0, 0, 0, //
//     0, 0, 1, 7, 1, 0, 0, 0, //
//     0, 1, 0, 0, 0, 1, 0, 0, //
//     1, 7, 0, 0, 0, 7, 1, 0, //
//     0, 1, 0, 0, 0, 1, 0, 0, //
//     0, 0, 1, 7, 1, 0, 0, 0, //
//     0, 0, 0, 1, 0, 0, 0, 0, //
//     0, 0, 0, 0, 0, 0, 0, 0, //
// ];

#[derive(Clone, Copy, Debug)]
enum ShiftDirection {
    Up,
    Down,
    Left,
    Right,
}

impl ShiftDirection {
    fn from_key(key: &Key) -> Option<Self> {
        use ShiftDirection::*;

        match key {
            Key::W => Some(Up),
            Key::D => Some(Right),
            Key::S => Some(Down),
            Key::A => Some(Left),
            _ => None,
        }
    }
    fn shift(&self, sprite: &mut Sprite) {
        match self {
            ShiftDirection::Up => sprite.shift_up(),
            ShiftDirection::Down => sprite.shift_down(),
            ShiftDirection::Left => sprite.shift_left(),
            ShiftDirection::Right => sprite.shift_right(),
        }
    }
}
