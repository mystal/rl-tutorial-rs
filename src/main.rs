extern crate rand;
extern crate tcod;

use tcod::{BackgroundFlag, Console};
use tcod::colors::{self, Color};
use tcod::console::{self, Root, Offscreen};
use tcod::input::{Key, KeyCode};
use tcod::map::{Map as FovMap, FovAlgorithm};

use object::Object;
use map::{Map, Tile};

mod map;
mod object;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: u32 = 20;

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color { r: 130, g: 110, b: 50 };
const COLOR_DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };
const COLOR_LIGHT_GROUND: Color = Color { r: 200, g: 180, b: 50 };

struct GameState {
    objects: [Object; 2],
    map: Map,
    fov_map: FovMap,
    previous_player_pos: (i32, i32),
}

impl GameState {
    fn new() -> Self {
        let (map, player_position) = map::make_map();

        // Place the player inside the first room.
        let player = Object::new(player_position.0, player_position.1, '@', colors::WHITE);
        let npc = Object::new(map::MAP_WIDTH / 2 - 5, map::MAP_HEIGHT / 2, '@', colors::YELLOW);
        let mut objects = [player, npc];

        let mut fov_map = FovMap::new(map::MAP_WIDTH, map::MAP_HEIGHT);
        for y in 0..map::MAP_HEIGHT {
            for x in 0..map::MAP_WIDTH {
                fov_map.set(x, y,
                            !map[x as usize][y as usize].block_sight,
                            !map[x as usize][y as usize].blocked);
            }
        }

        GameState {
            objects,
            map,
            fov_map,
            previous_player_pos: (-1, -1),
        }
    }

    fn handle_keys(&mut self, key_code: KeyCode) {
        let mut player = &mut self.objects[0];
        self.previous_player_pos = (player.x, player.y);
        match key_code {
            KeyCode::Left => player.move_by(-1, 0, &self.map),
            KeyCode::Right => player.move_by(1, 0, &self.map),
            KeyCode::Up => player.move_by(0, -1, &self.map),
            KeyCode::Down => player.move_by(0, 1, &self.map),
            _ => {},
        }
    }

    fn render_all(&mut self, root: &mut Root, con: &mut Offscreen) {
        let fov_recompute = self.previous_player_pos != (self.objects[0].x, self.objects[0].y);

        if fov_recompute {
            // Recompute FOV if needed (the player moved or something).
            let player = &self.objects[0];
            self.fov_map.compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);

            // Go through all tiles, and update their background color.
            for y in 0..map::MAP_HEIGHT {
                for x in 0..map::MAP_WIDTH {
                    let visible = self.fov_map.is_in_fov(x, y);
                    let wall = self.map[x as usize][y as usize].block_sight;
                    let color = match (visible, wall) {
                        // Outside of field of view:
                        (false, true) => COLOR_DARK_WALL,
                        (false, false) => COLOR_DARK_GROUND,
                        // Inside fov:
                        (true, true) => COLOR_LIGHT_WALL,
                        (true, false) => COLOR_LIGHT_GROUND,
                    };

                    let explored = &mut self.map[x as usize][y as usize].explored;
                    if visible {
                        // Since it's visible, explore it.
                        *explored = true;
                    }
                    if *explored {
                        // Show explored tiles only (any visible tile is explored already).
                        con.set_char_background(x, y, color, BackgroundFlag::Set);
                    }
                }
            }
        }

        // Draw all objects.
        for object in &self.objects {
            if self.fov_map.is_in_fov(object.x, object.y) {
                object.draw(con);
            }
        }

        console::blit(con, (0, 0), (SCREEN_WIDTH, SCREEN_HEIGHT), root, (0, 0), 1.0, 1.0);
    }
}

fn main() {
    let mut root = Root::initializer()
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust libtcod tutorial")
        .font("assets/arial10x10.png", tcod::FontLayout::Tcod)
        .font_type(tcod::FontType::Greyscale)
        .init();
    let mut con = Offscreen::new(map::MAP_WIDTH, map::MAP_HEIGHT);

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
