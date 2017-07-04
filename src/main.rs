extern crate tcod;

use tcod::{BackgroundFlag, Console};
use tcod::colors::{self, Color};
use tcod::console::{self, Root, Offscreen};
use tcod::input::{Key, KeyCode};

use object::Object;
use map::{Map, Tile};

mod map;
mod object;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: u32 = 20;

const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };

#[derive(Debug)]
struct GameState {
    objects: [Object; 2],
    map: Map,
}

impl GameState {
    fn new() -> Self {
        let player = Object::new(MAP_WIDTH / 2, MAP_HEIGHT / 2, '@', colors::WHITE);
        let npc = Object::new(MAP_WIDTH / 2 - 5, MAP_HEIGHT / 2, '@', colors::YELLOW);
        let mut objects = [player, npc];

        let map = make_map();

        GameState {
            objects,
            map,
        }
    }

    fn handle_keys(&mut self, key_code: KeyCode) {
        let mut player = &mut self.objects[0];
        match key_code {
            KeyCode::Left => player.move_by(-1, 0, &self.map),
            KeyCode::Right => player.move_by(1, 0, &self.map),
            KeyCode::Up => player.move_by(0, -1, &self.map),
            KeyCode::Down => player.move_by(0, 1, &self.map),
            _ => {},
        }
    }

    fn render_all(&self, root: &mut Root, con: &mut Offscreen) {
        // Draw all objects.
        for object in &self.objects {
            object.draw(con);
        }

        // Draw the map tiles.
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let wall = self.map[x as usize][y as usize].block_sight;
                if wall {
                    con.set_char_background(x, y, COLOR_DARK_WALL, BackgroundFlag::Set);
                } else {
                    con.set_char_background(x, y, COLOR_DARK_GROUND, BackgroundFlag::Set);
                }
            }
        }

        console::blit(con, (0, 0), (SCREEN_WIDTH, SCREEN_HEIGHT), root, (0, 0), 1.0, 1.0);
    }
}

fn make_map() -> Map {
    // Fill the map with "unblocked" tiles.
    let mut map = vec![vec![Tile::empty(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];

    map[30][22] = Tile::wall();
    map[50][22] = Tile::wall();

    map
}

fn main() {
    let mut root = Root::initializer()
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust libtcod tutorial")
        .font("assets/arial10x10.png", tcod::FontLayout::Tcod)
        .font_type(tcod::FontType::Greyscale)
        .init();
    let mut con = Offscreen::new(MAP_WIDTH, MAP_HEIGHT);

    let mut game_state = GameState::new();

    while !root.window_closed() {
        game_state.render_all(&mut root, &mut con);
        root.flush();

        for object in &game_state.objects {
            object.clear(&mut con);
        }

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
