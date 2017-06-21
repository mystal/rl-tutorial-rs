extern crate tcod;

use tcod::{colors, BackgroundFlag, Console, RootConsole};
use tcod::input::{Key, KeyCode};

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: u32 = 20;

struct GameState {
    player_x: i32,
    player_y: i32,
}

impl GameState {
    fn new() -> Self {
        GameState {
            player_x: SCREEN_WIDTH / 2,
            player_y: SCREEN_HEIGHT / 2,
        }
    }

    fn handle_keys(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Left => self.player_x -= 1,
            KeyCode::Right => self.player_x += 1,
            KeyCode::Up => self.player_y -= 1,
            KeyCode::Down => self.player_y += 1,
            _ => {},
        }
    }
}

fn main() {
    let mut root = RootConsole::initializer()
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust libtcod tutorial")
        .font("assets/arial10x10.png", tcod::FontLayout::Tcod)
        .font_type(tcod::FontType::Greyscale)
        .init();

    let mut game_state = GameState::new();

    while !root.window_closed() {
        root.set_default_foreground(colors::WHITE);
        root.put_char(game_state.player_x, game_state.player_y, '@', BackgroundFlag::None);
        root.flush();

        root.put_char(game_state.player_x, game_state.player_y, ' ', BackgroundFlag::None);

        match root.wait_for_keypress(true) {
            Key { code: KeyCode::Escape, .. } => break,
            Key { code: KeyCode::Enter, left_alt: true, .. } => {
                let fullscreen = !root.is_fullscreen();
                root.set_fullscreen(fullscreen);
            },
            Key { code, .. } => game_state.handle_keys(code),
        }
    }
}
