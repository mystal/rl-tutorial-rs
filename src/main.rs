extern crate tcod;

use tcod::{colors, BackgroundFlag, Console, RootConsole};
use tcod::input::KeyCode;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: u32 = 20;

fn main() {
    let mut root = RootConsole::initializer()
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust libtcod tutorial")
        .font("assets/arial10x10.png", tcod::FontLayout::Tcod)
        .font_type(tcod::FontType::Greyscale)
        .init();

    let mut player_x = 1;
    let mut player_y = 1;

    while !root.window_closed() {
        root.set_default_foreground(colors::WHITE);
        root.put_char(player_x, player_y, '@', BackgroundFlag::None);
        root.flush();

        match root.wait_for_keypress(true).code {
            KeyCode::Escape => break,
            _ => {},
        }
    }
}
