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

const CAMERA_WIDTH: i32 = 80;
const CAMERA_HEIGHT: i32 = 45;

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color { r: 130, g: 110, b: 50 };
const COLOR_DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };
const COLOR_LIGHT_GROUND: Color = Color { r: 200, g: 180, b: 50 };

// Player will always be the first object.
const PLAYER: usize = 0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

struct GameState {
    objects: Vec<Object>,
    map: Map,
    fov_map: FovMap,
    camera_pos: (i32, i32),
    previous_player_pos: (i32, i32),
    disable_fov: bool,
}

impl GameState {
    fn new() -> Self {
        // Create the player.
        let mut player = Object::new(0, 0, '@', "player", colors::WHITE, true);
        player.alive = true;
        let mut objects = vec![player];
        let map = map::make_map(&mut objects);


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
            camera_pos: (0, 0),
            previous_player_pos: (-1, -1),
            disable_fov: false,
        }
    }

    fn is_blocked(&self, x: i32, y: i32) -> bool {
        map::is_blocked(x, y, &self.map, &self.objects)
    }

    /// Move an object by the given amount, if the destination is not blocked.
    fn move_object_by(&mut self, id: usize, dx: i32, dy: i32) {
        let (x, y) = self.objects[id].pos();
        if !self.is_blocked(x + dx, y + dy) {
            self.objects[id].set_pos(x + dx, y + dy);
        }
    }

    fn player_move_or_attack(&mut self, dx: i32, dy: i32) {
        // the coordinates the player is moving to/attacking
        let x = self.objects[PLAYER].x + dx;
        let y = self.objects[PLAYER].y + dy;

        // Try to find an attackable object there.
        let target_id = self.objects.iter().position(|object| {
            object.pos() == (x, y)
        });

        // Attack if target found, move otherwise.
        if let Some(target_id) = target_id {
            println!("The {} laughs at your puny efforts to attack!", self.objects[target_id].name);
        } else {
            self.move_object_by(PLAYER, dx, dy);
        }
    }

    fn handle_keys(&mut self, key_code: KeyCode) -> PlayerAction {
        // Don't move if the player is dead.
        if !self.objects[PLAYER].alive {
            return PlayerAction::DidntTakeTurn;
        }

        self.previous_player_pos = self.objects[PLAYER].pos();
        match key_code {
            KeyCode::Left => {
                self.player_move_or_attack(-1, 0);
                PlayerAction::TookTurn
            },
            KeyCode::Right => {
                self.player_move_or_attack(1, 0);
                PlayerAction::TookTurn
            },
            KeyCode::Up => {
                self.player_move_or_attack(0, -1);
                PlayerAction::TookTurn
            },
            KeyCode::Down => {
                self.player_move_or_attack(0, 1);
                PlayerAction::TookTurn
            },
            _ => PlayerAction::DidntTakeTurn,
        }
    }

    fn move_camera(&mut self, target_x: i32, target_y: i32) -> bool {
        let mut fov_recompute = false;

        // New camera coordinates (top-left corner of the screen relative to the map).
        // Coordinates so that the target is at the center of the screen.
        let mut x = target_x - CAMERA_WIDTH / 2;
        let mut y = target_y - CAMERA_HEIGHT / 2;

        // Clamp the viewport to the map edges.
        if x < 0 {
            x = 0;
        } else if x > map::MAP_WIDTH - CAMERA_WIDTH - 1 {
            x = map::MAP_WIDTH - CAMERA_WIDTH - 1;
        }
        if y < 0 {
            y = 0;
        } else if y > map::MAP_HEIGHT - CAMERA_HEIGHT - 1 {
            y = map::MAP_HEIGHT - CAMERA_HEIGHT - 1;
        }

        if x != self.camera_pos.0 || y != self.camera_pos.1 {
            fov_recompute = true;
        }

        self.camera_pos = (x, y);

        fov_recompute
    }

    fn to_camera_coordinates(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        // Convert coordinates on the map to coordinates on the screen.
        let (x, y) = (x - self.camera_pos.0, y - self.camera_pos.1);

        // Check that the coordinates are inside the view.
        if x < 0 || y < 0 || x >= CAMERA_WIDTH || y >= CAMERA_HEIGHT {
            None
        } else {
            Some((x, y))
        }
    }

    fn render_all(&mut self, root: &mut Root, con: &mut Offscreen) {
        let (player_x, player_y) = (self.objects[PLAYER].x, self.objects[PLAYER].y);
        let fov_recompute = self.move_camera(player_x, player_y) ||
            self.previous_player_pos != (player_x, player_y);

        if fov_recompute {
            // Recompute FOV if needed (the player moved or something).
            self.fov_map.compute_fov(player_x, player_y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);

            // Go through all tiles, and update their background color.
            for y in 0..CAMERA_HEIGHT {
                for x in 0..CAMERA_WIDTH {
                    let (map_x, map_y) = (self.camera_pos.0 + x, self.camera_pos.1 + y);
                    let visible = self.fov_map.is_in_fov(map_x, map_y);
                    let wall = self.map[map_x as usize][map_y as usize].block_sight;
                    let color = match (visible, wall) {
                        // Outside of field of view:
                        (false, true) => COLOR_DARK_WALL,
                        (false, false) => COLOR_DARK_GROUND,
                        // Inside fov:
                        (true, true) => COLOR_LIGHT_WALL,
                        (true, false) => COLOR_LIGHT_GROUND,
                    };

                    let explored = &mut self.map[map_x as usize][map_y as usize].explored;
                    if visible {
                        // Since it's visible, explore it.
                        *explored = true;
                    }
                    if self.disable_fov || *explored {
                        // Show explored tiles only (any visible tile is explored already).
                        con.set_char_background(x, y, color, BackgroundFlag::Set);
                    } else {
                        // Clear the tile.
                        con.set_char_background(x, y, colors::BLACK, BackgroundFlag::Set);
                    }
                }
            }
        }

        // Draw all objects.
        for object in &self.objects {
            if self.disable_fov || self.fov_map.is_in_fov(object.x, object.y) {
                if let Some((x, y)) = self.to_camera_coordinates(object.x, object.y) {
                    con.set_default_foreground(object.color);
                    con.put_char(x, y, object.char, BackgroundFlag::None);
                }
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
            if let Some((x, y)) = game_state.to_camera_coordinates(object.x, object.y) {
                con.put_char(x, y, ' ', BackgroundFlag::None);
            }
        }

        let player_action = match root.wait_for_keypress(true) {
            Key { code: KeyCode::Escape, .. } => PlayerAction::Exit,
            Key { code: KeyCode::Enter, left_alt: true, .. } => {
                let fullscreen = !root.is_fullscreen();
                root.set_fullscreen(fullscreen);
                PlayerAction::DidntTakeTurn
            },
            Key { code: KeyCode::Number0 , .. } => {
                game_state.disable_fov = !game_state.disable_fov;
                PlayerAction::DidntTakeTurn
            },
            Key { code, .. } => game_state.handle_keys(code),
        };

        if player_action == PlayerAction::Exit {
            break;
        }

        // Let monsters take their turn.
        if game_state.objects[PLAYER].alive && player_action != PlayerAction::DidntTakeTurn {
            // Skip the first object, which should be the player.
            for object in game_state.objects.iter().skip(1) {
                println!("The {} growls!", object.name);
            }
        }
    }
}
