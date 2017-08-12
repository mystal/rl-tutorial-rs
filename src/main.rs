extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json as json;
extern crate tcod;

use std::ascii::AsciiExt;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};

use rand::Rng;
use tcod::{BackgroundFlag, Console, TextAlignment};
use tcod::colors::{self, Color};
use tcod::console::{self, Root, Offscreen};
use tcod::input::{self, Event, Key, KeyCode, Mouse};
use tcod::map::{Map as FovMap, FovAlgorithm};
use tcod::pathfinding::AStar;

use map::Map;
use message::Messages;
use object::*;

mod map;
mod message;
mod object;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 60;

const CAMERA_WIDTH: i32 = 80;
const CAMERA_HEIGHT: i32 = 43;

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color { r: 130, g: 110, b: 50 };
const COLOR_DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };
const COLOR_LIGHT_GROUND: Color = Color { r: 200, g: 180, b: 50 };

// Sizes and coordinates relevant for the GUI.
const BAR_WIDTH: i32 = 20;
const PANEL_HEIGHT: i32 = 7;
const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;

const MSG_X: i32 = BAR_WIDTH + 2;
const MSG_WIDTH: i32 = SCREEN_WIDTH - BAR_WIDTH - 2;
const MSG_HEIGHT: usize = PANEL_HEIGHT as usize - 1;

const INVENTORY_WIDTH: i32 = 50;

// Item constants.
const HEAL_AMOUNT: i32 = 4;
const LIGHTNING_DAMAGE: i32 = 20;
const LIGHTNING_RANGE: i32 = 5;
const CONFUSE_RANGE: i32 = 8;
const CONFUSE_NUM_TURNS: i32 = 10;
const FIREBALL_RADIUS: i32 = 3;
const FIREBALL_DAMAGE: i32 = 12;

// Experience and level-ups.
const LEVEL_UP_BASE: i32 = 200;
const LEVEL_UP_FACTOR: i32 = 150;
const LEVEL_SCREEN_WIDTH: i32 = 40;
const CHARACTER_SCREEN_WIDTH: i32 = 30;

// Player will always be the first object.
const PLAYER: usize = 0;

/// Mutably borrow two *separate* elements from the given slice.
/// Panics when the indexes are equal or out of bounds.
fn mut_two<T>(first_index: usize, second_index: usize, items: &mut [T]) -> (&mut T, &mut T) {
    assert!(first_index != second_index);
    let split_at_index = std::cmp::max(first_index, second_index);
    let (first_slice, second_slice) = items.split_at_mut(split_at_index);
    if first_index < second_index {
        (&mut first_slice[first_index], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second_index])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

enum UseResult {
    UsedUp,
    Cancelled,
}

struct Tcod {
    root: Root,
    con: Offscreen,
    panel: Offscreen,
}

fn default_fov_map() -> FovMap {
    FovMap::new(map::MAP_WIDTH, map::MAP_HEIGHT)
}

#[derive(Serialize, Deserialize)]
struct GameState {
    // Serialized state.
    objects: Vec<Object>,
    map: Map,
    // TODO: Rename to log.
    messages: Messages,
    inventory: Vec<Object>,
    dungeon_level: u32,

    #[serde(skip, default = "default_fov_map")]
    fov_map: FovMap,
    #[serde(skip)]
    camera_pos: (i32, i32),
    #[serde(skip)]
    previous_player_pos: (i32, i32),
    #[serde(skip)]
    mouse: Mouse,
    #[serde(skip)]
    disable_fov: bool,
}

impl GameState {
    fn new() -> Self {
        // Create the player.
        let mut player = Object::new(0, 0, '@', "player", colors::WHITE, true);
        player.alive = true;
        player.fighter = Some(Fighter {
            max_hp: 30,
            hp: 30,
            defense: 2,
            power: 5,
            xp: 0,
            on_death: DeathCallback::Player,
        });
        let mut objects = vec![player];
        let map = map::make_map(&mut objects);

        let mut messages = Messages::new(MSG_HEIGHT);

        // A warm welcoming message!
        messages.message("Welcome stranger! Prepare to perish in the Tombs of the Ancient Kings.", colors::RED);

        let mut game_state = GameState {
            objects,
            map,
            messages,
            inventory: Vec::new(),
            dungeon_level: 1,

            fov_map: default_fov_map(),
            camera_pos: (0, 0),
            previous_player_pos: (-1, -1),
            mouse: Default::default(),
            disable_fov: false,
        };
        game_state.initialize_fov();
        game_state
    }

    fn from_save() -> Result<Self, Box<Error>> {
        let mut json_save_state = String::new();
        let mut file = File::open("savegame")?;
        file.read_to_string(&mut json_save_state)?;
        let mut result: Self = json::from_str(&json_save_state)?;
        result.initialize_fov();
        Ok(result)
    }

    fn save(&self) -> Result<(), Box<Error>> {
        let save_data = json::to_string(self)?;
        let mut file = File::create("savegame")?;
        file.write_all(save_data.as_bytes())?;
        Ok(())
    }

    fn initialize_fov(&mut self) {
        // Initialize the FOV map.
        for y in 0..map::MAP_HEIGHT {
            for x in 0..map::MAP_WIDTH {
                self.fov_map.set(x, y,
                                 !self.map[x as usize][y as usize].block_sight,
                                 !self.map[x as usize][y as usize].blocked);
            }
        }
    }

    /// Advance to the next level
    fn next_level(&mut self) {
        self.messages.message("You take a moment to rest, and recover your strength.", colors::VIOLET);
        let heal_hp = self.objects[PLAYER].fighter.map_or(0, |f| f.max_hp / 2);
        self.objects[PLAYER].heal(heal_hp);

        self.messages.message("After a rare moment of peace, you descend deeper into \
                               the heart of the dungeon...", colors::RED);
        self.dungeon_level += 1;
        self.map = map::make_map(&mut self.objects);
        self.initialize_fov();
    }

    fn is_blocked(&self, x: i32, y: i32) -> bool {
        map::is_blocked(x, y, &self.map, &self.objects)
    }

    /// Find closest enemy, up to a maximum range, and in the player's FOV.
    fn closest_monster(&self, max_range: i32) -> Option<usize> {
        let mut closest_enemy = None;
        // Start with (slightly more than) maximum range.
        let mut closest_dist = (max_range + 1) as f32;

        for (id, object) in self.objects.iter().enumerate() {
            if (id != PLAYER) && object.fighter.is_some() && object.ai.is_some() &&
                self.fov_map.is_in_fov(object.x, object.y) {
                // Calculate distance between this object and the player.
                let dist = self.objects[PLAYER].distance_to(object);
                if dist < closest_dist {
                    // It's closer, so remember it.
                    closest_enemy = Some(id);
                    closest_dist = dist;
                }
            }
        }
        closest_enemy
    }

    fn level_up(&mut self, tcod: &mut Tcod) {
        let player = &mut self.objects[PLAYER];
        let level_up_xp = LEVEL_UP_BASE + player.level * LEVEL_UP_FACTOR;

        // See if the player's experience is enough to level-up.
        if player.fighter.as_ref().map_or(0, |f| f.xp) >= level_up_xp {
            // It is! Level up.
            player.level += 1;
            self.messages.message(
                format!("Your battle skills grow stronger! You reached level {}!", player.level),
                colors::YELLOW,
            );

            // Increase the player's stats!
            let fighter = player.fighter.as_mut().unwrap();
            let mut choice = None;
            // Keep asking until a choice is made.
            while choice.is_none() {
                choice = menu(
                    "Level up! Choose a stat to raise:\n",
                    &[format!("Constitution (+20 HP, from {})", fighter.max_hp),
                      format!("Strength (+1 attack, from {})", fighter.power),
                      format!("Agility (+1 defense, from {})", fighter.defense)],
                    LEVEL_SCREEN_WIDTH, &mut tcod.root,
                );
            };
            fighter.xp -= level_up_xp;
            match choice.unwrap() {
                0 => {
                    fighter.max_hp += 20;
                    fighter.hp += 20;
                }
                1 => {
                    fighter.power += 1;
                }
                2 => {
                    fighter.defense += 1;
                }
                _ => unreachable!(),
            }
        }
    }

    /// Move an object by the given amount, if the destination is not blocked.
    fn move_object_by(&mut self, id: usize, dx: i32, dy: i32) {
        let (x, y) = self.objects[id].pos();
        if !self.is_blocked(x + dx, y + dy) {
            self.objects[id].set_pos(x + dx, y + dy);
        }
    }

    fn move_towards(&mut self, id: usize, target_x: i32, target_y: i32) {
        // Vector from this object to the target, and distance.
        let dx = target_x - self.objects[id].x;
        let dy = target_y - self.objects[id].y;
        let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

        // Normalize it to length 1 (preserving direction), then round it and
        // convert to integer so the movement is restricted to the map grid.
        let dx = (dx as f32 / distance).round() as i32;
        let dy = (dy as f32 / distance).round() as i32;

        // TODO: Prevent moving diagonally.

        self.move_object_by(id, dx, dy);
    }

    fn move_astar(&mut self, id: usize, target_id: usize) {
        // Create a FOV map that has the dimensions of the map.
        let mut fov_map = FovMap::new(map::MAP_WIDTH, map::MAP_HEIGHT);

        // Scan the current map each turn and set all the walls as unwalkable.
        for y in 0..map::MAP_HEIGHT {
            for x in 0..map::MAP_WIDTH {
                fov_map.set(x, y,
                            !self.map[x as usize][y as usize].block_sight,
                            !self.map[x as usize][y as usize].blocked);
            }
        }

        // Scan all the objects to see if there are objects that must be navigated around
        // Check also that the object isn't self or the target (so that the start and the end points are free)
        // The AI class handles the situation if self is next to the target so it will not use this A* function anyway
        for (i, object) in self.objects.iter().enumerate() {
            if object.blocks && i != id && i != target_id {
                // Set the tile as a wall so it must be navigated around.
                fov_map.set(object.x, object.y, true, false);
            }
        }

        // Allocate a A* path
        // The 1.41 is the normal diagonal cost of moving, it can be set as 0.0 if diagonal moves are prohibited
        let ENEMIES_MOVE_DIAGONAL = false;
        let cost = if ENEMIES_MOVE_DIAGONAL {
            1.41
        } else {
            0.0
        };
        let mut my_path = AStar::new_from_map(fov_map, cost);

        // Compute the path between self's coordinates and the target's coordinates
        let (object_x, object_y) = (self.objects[id].x, self.objects[id].y);
        let (target_x, target_y) = (self.objects[target_id].x, self.objects[target_id].y);
        my_path.find((object_x, object_y), (target_x, target_y));

        // Check if the path exists, and in this case, also the path is shorter than 25 tiles
        // The path size matters if you want the monster to use alternative longer paths (for example through other rooms) if for example the player is in a corridor
        // It makes sense to keep path size relatively low to keep the monsters from running around the map if there's an alternative path really far away
        if !my_path.is_empty() && my_path.len() < 25 {
            // Find the next coordinates in the computed full path.
            if let Some((x, y)) = my_path.walk_one_step(true) {
                // Set object's coordinates to the next path tile.
                self.objects[id].x = x;
                self.objects[id].y = y;
            }
        } else {
            // Keep the old move function as a backup so that if there are no paths (for example another monster blocks a corridor)
            // it will still try to move towards the player (closer to the corridor opening)
            self.move_towards(id, target_x, target_y)
        }
    }

    fn ai_take_turn(&mut self, monster_id: usize) {
        if let Some(ai) = self.objects[monster_id].ai.take() {
            let new_ai = match ai {
                Ai::Basic => self.ai_basic(monster_id),
                Ai::Confused { previous_ai, num_turns } =>
                    self.ai_confused(monster_id, previous_ai, num_turns),
            };
            self.objects[monster_id].ai = Some(new_ai);
        }
    }

    fn ai_basic(&mut self, monster_id: usize) -> Ai {
        // A basic monster takes its turn. If you can see it, it can see you.
        let (monster_x, monster_y) = self.objects[monster_id].pos();
        if self.fov_map.is_in_fov(monster_x, monster_y) {
            if self.objects[monster_id].distance_to(&self.objects[PLAYER]) > 1.0 {
                // Move towards player if not adjacent.
                self.move_astar(monster_id, PLAYER);
            } else if self.objects[PLAYER].fighter.map_or(false, |f| f.hp > 0) {
                // Close enough, attack! (if the player is still alive.)
                let (monster, player) = mut_two(monster_id, PLAYER, &mut self.objects);
                monster.attack(player, &mut self.messages);
            }
        }
        Ai::Basic
    }

    fn ai_confused(&mut self, monster_id: usize, previous_ai: Box<Ai>, num_turns: i32) -> Ai {
        if num_turns >= 0 {
            // Still confused, so move in a random direction, and decrease the number of turns confused.
            let possible_movements = [
                (0, 0),
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
            ];
            let (x, y) = *rand::thread_rng().choose(&possible_movements)
                .expect("Confused enemy could not get a movement direction.");

            self.move_object_by(monster_id, x, y);
            Ai::Confused {
                previous_ai: previous_ai,
                num_turns: num_turns - 1,
            }
        } else {
            // Restore the previous AI (this one will be deleted).
            self.messages.message(
                format!("The {} is no longer confused!", self.objects[monster_id].name),
                colors::RED,
            );
            *previous_ai
        }
    }

    fn player_move_or_attack(&mut self, dx: i32, dy: i32) {
        // the coordinates the player is moving to/attacking
        let x = self.objects[PLAYER].x + dx;
        let y = self.objects[PLAYER].y + dy;

        // Try to find an attackable object there.
        let target_id = self.objects.iter().position(|object| {
            object.fighter.is_some() && object.pos() == (x, y)
        });

        // Attack if target found, move otherwise.
        if let Some(target_id) = target_id {
            let (player, target) = mut_two(PLAYER, target_id, &mut self.objects);
            player.attack(target, &mut self.messages);
        } else {
            self.move_object_by(PLAYER, dx, dy);
        }
    }

    /// Add to the player's inventory and remove from the map.
    fn pick_item_up(&mut self, object_id: usize) {
        if self.inventory.len() >= 26 {
            self.messages.message(
                format!("Your inventory is full, cannot pick up {}.", self.objects[object_id].name),
                colors::RED,
            );
        } else {
            let item = self.objects.swap_remove(object_id);
            self.messages.message(format!("You picked up a {}!", item.name), colors::GREEN);
            self.inventory.push(item);
        }
    }

    fn drop_item(&mut self, inventory_id: usize) {
        let mut item = self.inventory.remove(inventory_id);
        item.set_pos(self.objects[PLAYER].x, self.objects[PLAYER].y);
        self.messages.message(format!("You dropped a {}.", item.name), colors::YELLOW);
        self.objects.push(item);
    }

    fn use_item(&mut self, inventory_id: usize, tcod: &mut Tcod) {
        use Item::*;
        // Just call the "use_function" if it is defined.
        if let Some(item) = self.inventory[inventory_id].item {
            let on_use = match item {
                Heal => Self::cast_heal,
                Lightning => Self::cast_lightning,
                Confuse => Self::cast_confuse,
                Fireball => Self::cast_fireball,
            };
            match on_use(self, inventory_id, tcod) {
                UseResult::UsedUp => {
                    // Destroy after use, unless it was cancelled for some reason.
                    self.inventory.remove(inventory_id);
                },
                UseResult::Cancelled => self.messages.message("Cancelled", colors::WHITE),
            }
        } else {
            self.messages.message(
                format!("The {} cannot be used.", self.inventory[inventory_id].name),
                colors::WHITE,
            );
        }
    }

    fn cast_heal(&mut self, _inventory_id: usize, _tcod: &mut Tcod) -> UseResult {
        // Heal the player.
        if let Some(fighter) = self.objects[PLAYER].fighter {
            if fighter.hp == fighter.max_hp {
                self.messages.message("You are already at full health.", colors::RED);
                return UseResult::Cancelled;
            }
            self.messages.message("Your wounds start to feel better!", colors::LIGHT_VIOLET);
            self.objects[PLAYER].heal(HEAL_AMOUNT);
            return UseResult::UsedUp;
        }
        UseResult::Cancelled
    }

    fn cast_lightning(&mut self, _inventory_id: usize, _tcod: &mut Tcod) -> UseResult {
        // Find closest enemy (inside a maximum range) and damage it.
        if let Some(monster_id) = self.closest_monster(LIGHTNING_RANGE) {
            // Zap it!
            self.messages.message(
                format!("A lightning bolt strikes the {} with a loud thunder! \
                         The damage is {} hit points.",
                        self.objects[monster_id].name, LIGHTNING_DAMAGE),
                colors::LIGHT_BLUE,
            );
            if let Some(xp) = self.objects[monster_id].take_damage(LIGHTNING_DAMAGE, &mut self.messages) {
                self.objects[PLAYER].fighter.as_mut().unwrap().xp += xp;
            }
            UseResult::UsedUp
        } else {
            // No enemy found within maximum range.
            self.messages.message("No enemy is close enough to strike.", colors::RED);
            UseResult::Cancelled
        }
    }

    fn cast_confuse(&mut self, _inventory_id: usize, tcod: &mut Tcod) -> UseResult {
        // Ask the player for a target to confuse.
        self.messages.message(
            "Left-click an enemy to confuse it, or right-click to cancel.",
            colors::LIGHT_CYAN,
        );
        if let Some(monster_id) = self.target_monster(tcod, Some(CONFUSE_RANGE as f32)) {
            let old_ai = self.objects[monster_id].ai.take()
                .unwrap_or(Ai::Basic);
            // Replace the monster's AI with a "confused" one.
            // After some turns it will restore the old AI
            self.objects[monster_id].ai = Some(Ai::Confused {
                previous_ai: Box::new(old_ai),
                num_turns: CONFUSE_NUM_TURNS,
            });
            self.messages.message(
                format!("The eyes of {} look vacant, as it starts to stumble around!",
                        self.objects[monster_id].name),
                colors::LIGHT_GREEN);
            UseResult::UsedUp
        } else {
            UseResult::Cancelled
        }
    }

    fn cast_fireball(&mut self, _inventory_id: usize, tcod: &mut Tcod) -> UseResult {
        // Ask the player for a target tile to throw a fireball at.
        self.messages.message(
            "Left-click a target tile for the fireball, or right-click to cancel.",
            colors::LIGHT_CYAN,
        );
        let (x, y) = match self.target_tile(tcod, None) {
            Some(tile_pos) => tile_pos,
            None => return UseResult::Cancelled,
        };
        self.messages.message(
            format!("The fireball explodes, burning everything within {} tiles!", FIREBALL_RADIUS),
            colors::ORANGE,
        );

        let mut xp_to_gain = 0;
        for (id, obj) in self.objects.iter_mut().enumerate() {
            if obj.distance(x, y) <= FIREBALL_RADIUS as f32 && obj.fighter.is_some() {
                self.messages.message(
                    format!("The {} gets burned for {} hit points.", obj.name, FIREBALL_DAMAGE),
                    colors::ORANGE,
                );
                if let Some(xp) = obj.take_damage(FIREBALL_DAMAGE, &mut self.messages) {
                    // Don't reward the player for burning themself!
                    if id != PLAYER {
                        xp_to_gain += xp;
                    }
                }
            }
        }
        self.objects[PLAYER].fighter.as_mut().unwrap().xp += xp_to_gain;

        UseResult::UsedUp
    }

    fn handle_keys(&mut self, key: Key, tcod: &mut Tcod) -> PlayerAction {
        // Don't move if the player is dead.
        if !self.objects[PLAYER].alive {
            return PlayerAction::DidntTakeTurn;
        }

        self.previous_player_pos = self.objects[PLAYER].pos();
        match key {
            Key { printable: 'c', .. } => {
                // Show character information.
                let player = &self.objects[PLAYER];
                let level = player.level;
                let level_up_xp = LEVEL_UP_BASE + player.level * LEVEL_UP_FACTOR;
                if let Some(fighter) = player.fighter.as_ref() {
                    let msg = format!(
"Character information

Level: {}
Experience: {}
Experience to level up: {}

Maximum HP: {}
Attack: {}
Defense: {}",
                        level, fighter.xp, level_up_xp, fighter.max_hp, fighter.power, fighter.defense);
                    msgbox(&msg, CHARACTER_SCREEN_WIDTH, &mut tcod.root);
                }

                PlayerAction::DidntTakeTurn
            }
            Key { printable: 'g', .. } => {
                // Pick up an item.
                let item_id = self.objects.iter().position(|object| {
                    object.pos() == self.objects[PLAYER].pos() && object.item.is_some()
                });
                if let Some(item_id) = item_id {
                    self.pick_item_up(item_id);
                }
                PlayerAction::DidntTakeTurn
            },
            Key { printable: 'i', .. } => {
                // Show the inventory.
                let inventory_index = inventory_menu(
                    &self.inventory,
                    "Press the key next to an item to use it, or any other to cancel.\n",
                    &mut tcod.root);
                if let Some(inventory_index) = inventory_index {
                    self.use_item(inventory_index, tcod);
                }
                PlayerAction::DidntTakeTurn
            },
            Key { printable: 'd', .. } => {
                // Show the inventory. If an item is selected, drop it.
                let inventory_index = inventory_menu(
                    &self.inventory,
                    "Press the key next to an item to drop it, or any other to cancel.\n'",
                    &mut tcod.root);
                if let Some(inventory_index) = inventory_index {
                    self.drop_item(inventory_index);
                }
                PlayerAction::DidntTakeTurn
            },
            Key { printable: '<', .. } => {
                // Go down stairs, if the player is on them.
                let player_on_stairs = self.objects.iter().any(|object|
                    object.pos() == self.objects[PLAYER].pos() && object.name == "stairs"
                );
                if player_on_stairs {
                    self.next_level();
                }
                PlayerAction::DidntTakeTurn
            },
            Key { printable: '.', .. } => {
                // Simply wait a turn.
                PlayerAction::TookTurn
            }
            Key { code: KeyCode::Left, .. } => {
                self.player_move_or_attack(-1, 0);
                PlayerAction::TookTurn
            },
            Key { code: KeyCode::Right, .. } => {
                self.player_move_or_attack(1, 0);
                PlayerAction::TookTurn
            },
            Key { code: KeyCode::Up, .. } => {
                self.player_move_or_attack(0, -1);
                PlayerAction::TookTurn
            },
            Key { code: KeyCode::Down, .. } => {
                self.player_move_or_attack(0, 1);
                PlayerAction::TookTurn
            },
            _ => PlayerAction::DidntTakeTurn,
        }
    }

    /// Return the position of a tile left-clicked in player's FOV (optionally in a
    /// range), or None if cancelled.
    fn target_tile(&mut self, tcod: &mut Tcod, max_range: Option<f32>) -> Option<(i32, i32)> {
        loop {
            // Render the screen. This erases the inventory and shows the names of
            // objects under the mouse.
            tcod.root.flush();
            let event = input::check_for_event(input::KEY_PRESS | input::MOUSE)
                .map(|e| e.1);
            let mut key = None;
            match event {
                Some(Event::Mouse(m)) => self.mouse = m,
                Some(Event::Key(k)) => key = Some(k),
                None => {},
            }
            self.render_all(tcod);

            let (x, y) = self.to_world_coordinates(self.mouse.cx as i32, self.mouse.cy as i32);

            // Accept the target if the player clicked in FOV, and in case a range
            // is specified, if it's in that range.
            let in_fov = (x < map::MAP_WIDTH) && (y < map::MAP_HEIGHT) && self.fov_map.is_in_fov(x, y);
            let in_range = max_range.map_or(
                true, |range| self.objects[PLAYER].distance(x, y) <= range);
            if self.mouse.lbutton_pressed && in_fov && in_range {
                return Some((x, y));
            }

            let escape = key.map_or(false, |k| k.code == KeyCode::Escape);
            // Cancel if the player right-clicked or pressed Escape.
            if self.mouse.rbutton_pressed || escape {
                return None;
            }
        }
    }

    /// Returns a clicked monster inside FOV up to a range, or None if right-clicked.
    fn target_monster(&mut self, tcod: &mut Tcod, max_range: Option<f32>) -> Option<usize> {
        loop {
            match self.target_tile(tcod, max_range) {
                Some((x, y)) => {
                    // Return the first clicked monster, otherwise continue looping.
                    for (id, obj) in self.objects.iter().enumerate() {
                        if obj.pos() == (x, y) && obj.fighter.is_some() && id != PLAYER {
                            return Some(id);
                        }
                    }
                },
                None => return None,
            }
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

    fn to_world_coordinates(&self, x: i32, y: i32) -> (i32, i32) {
        // Convert coordinates on the screen to coordinates on the map.
        (x + self.camera_pos.0, y + self.camera_pos.1)
    }

    /// Return a string with the names of all objects under the mouse.
    fn get_names_under_mouse(&self) -> String {
        let (x, y) = self.to_world_coordinates(self.mouse.cx as i32, self.mouse.cy as i32);

        // Create a list with the names of all objects at the mouse's coordinates and in FOV.
        let names = self.objects.iter()
            .filter(|obj| obj.pos() == (x, y) &&
                    (self.disable_fov || self.fov_map.is_in_fov(obj.x, obj.y) ||
                     (obj.always_visible && self.map[obj.x as usize][obj.y as usize].explored)))
            .map(|obj| obj.name.as_ref())
            .collect::<Vec<_>>();

        // Join the names, separated by commas.
        names.join(", ")
    }

    fn render_all(&mut self, tcod: &mut Tcod) {
        let (player_x, player_y) = (self.objects[PLAYER].x, self.objects[PLAYER].y);
        let fov_recompute = self.move_camera(player_x, player_y) ||
            self.previous_player_pos != (player_x, player_y);

        if fov_recompute {
            // Recompute FOV if needed (the player moved or something).
            self.fov_map.compute_fov(player_x, player_y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
        }

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
                    tcod.con.set_char_background(x, y, color, BackgroundFlag::Set);
                } else {
                    // Clear the tile.
                    tcod.con.set_char_background(x, y, colors::BLACK, BackgroundFlag::Set);
                }
            }
        }

        // Filter out visible objects and sort them so that non-blocking ones come first.
        let mut to_draw: Vec<_> = if self.disable_fov {
            self.objects.iter()
                .collect()
        } else {
            self.objects.iter()
                .filter(|obj| self.fov_map.is_in_fov(obj.x, obj.y) ||
                        (obj.always_visible && self.map[obj.x as usize][obj.y as usize].explored))
                .collect()
        };
        to_draw.sort_by(|o1, o2| o1.blocks.cmp(&o2.blocks));

        // Draw all objects.
        for object in to_draw {
            if let Some((x, y)) = self.to_camera_coordinates(object.x, object.y) {
                tcod.con.set_default_foreground(object.color);
                tcod.con.put_char(x, y, object.char, BackgroundFlag::None);
            }
        }

        console::blit(&tcod.con, (0, 0), (SCREEN_WIDTH, SCREEN_HEIGHT), &mut tcod.root, (0, 0), 1.0, 1.0);

        // Prepare to render the GUI panel.
        tcod.panel.set_default_background(colors::BLACK);
        tcod.panel.clear();

        // Print the game messages, one line at a time.
        let mut y = MSG_HEIGHT as i32;
        for &(ref msg, color) in self.messages.iter().rev() {
            let msg_height = tcod.panel.get_height_rect(MSG_X, y, MSG_WIDTH, 0, msg);
            y -= msg_height;
            if y < 0 {
                break;
            }
            tcod.panel.set_default_foreground(color);
            tcod.panel.print_rect(MSG_X, y, MSG_WIDTH, 0, msg);
        }

        // Show the player's stats.
        let hp = self.objects[PLAYER].fighter.map_or(0, |f| f.hp);
        let max_hp = self.objects[PLAYER].fighter.map_or(0, |f| f.max_hp);
        render_bar(&mut tcod.panel, 1, 1, BAR_WIDTH, "HP", hp, max_hp, colors::LIGHT_RED, colors::DARKER_RED);

        tcod.panel.print_ex(1, 3, BackgroundFlag::None, TextAlignment::Left,
                            format!("Dungeon level: {}", self.dungeon_level));

        // Display names of objects under the mouse.
        tcod.panel.set_default_foreground(colors::LIGHT_GREY);
        tcod.panel.print_ex(1, 0, BackgroundFlag::None, TextAlignment::Left,
                            self.get_names_under_mouse());

        // Blit the contents of `panel` to the root console.
        console::blit(&tcod.panel, (0, 0), (SCREEN_WIDTH, PANEL_HEIGHT), &mut tcod.root, (0, PANEL_Y), 1.0, 1.0);
    }
}

fn menu<T: AsRef<str>>(header: &str, options: &[T], width: i32,
                       root: &mut Root) -> Option<usize> {
    assert!(options.len() <= 26, "Cannot have a menu with more than 26 options.");

    // Calculate total height for the header (after auto-wrap) and one line per option.
    let header_height = if header.is_empty() {
        0
    } else {
        root.get_height_rect(0, 0, width, SCREEN_HEIGHT, header)
    };
    let height = options.len() as i32 + header_height;

    // Create an off-screen console that represents the menu's window.
    let mut window = Offscreen::new(width, height);

    // Print the header, with auto-wrap.
    window.set_default_foreground(colors::WHITE);
    window.print_rect_ex(0, 0, width, height, BackgroundFlag::None, TextAlignment::Left, header);

    // Print all the options.
    for (index, option_text) in options.iter().enumerate() {
        let menu_letter = (b'a' + index as u8) as char;
        let text = format!("({}) {}", menu_letter, option_text.as_ref());
        window.print_ex(0, header_height + index as i32,
                        BackgroundFlag::None, TextAlignment::Left, text);
    }

    // Blit the contents of "window" to the root console.
    let x = SCREEN_WIDTH / 2 - width / 2;
    let y = SCREEN_HEIGHT / 2 - height / 2;
    console::blit(&mut window, (0, 0), (width, height), root, (x, y), 1.0, 0.7);

    // Present the root console to the player and wait for a key-press.
    root.flush();
    // TODO: Include this in the main loop!
    let key = root.wait_for_keypress(true);

    // Convert the ASCII code to an index; if it corresponds to an option, return it.
    if key.printable.is_alphabetic() {
        let index = key.printable.to_ascii_lowercase() as usize - 'a' as usize;
        if index < options.len() {
            Some(index)
        } else {
            None
        }
    } else {
        None
    }
}

fn inventory_menu(inventory: &[Object], header: &str, root: &mut Root) -> Option<usize> {
    // How a menu with each item of the inventory as an option.
    let options = if inventory.len() == 0 {
        vec!["Inventory is empty."]
    } else {
        inventory.iter().map(|item| item.name.as_ref()).collect()
    };

    let inventory_index = menu(header, &options, INVENTORY_WIDTH, root);

    // If an item was chosen, return it.
    if inventory.len() > 0 {
        inventory_index
    } else {
        None
    }
}

fn render_bar(panel: &mut Offscreen,
              x: i32,
              y: i32,
              total_width: i32,
              name: &str,
              value: i32,
              maximum: i32,
              bar_color: Color,
              back_color: Color)
{
    // Render a bar (HP, experience, etc). First calculate the width of the bar.
    let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

    // Render the background first.
    panel.set_default_background(back_color);
    panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);

    // Now render the bar on top.
    panel.set_default_background(bar_color);
    if bar_width > 0 {
        panel.rect(x, y, bar_width, 1, false, BackgroundFlag::Screen);
    }

    // Finally, some centered text with the values.
    panel.set_default_foreground(colors::WHITE);
    panel.print_ex(x + total_width / 2, y, BackgroundFlag::None, TextAlignment::Center,
                   &format!("{}: {}/{}", name, value, maximum));
}

fn play_game(game_state: &mut GameState, tcod: &mut Tcod) {
    let mut key = Default::default();

    while !tcod.root.window_closed() {
        match input::check_for_event(input::MOUSE | input::KEY_PRESS) {
           Some((_, Event::Mouse(m))) => game_state.mouse = m,
           Some((_, Event::Key(k))) => key = k,
           _ => key = Default::default(),
        }

        game_state.render_all(tcod);
        tcod.root.flush();

        // Level up if needed.
        game_state.level_up(tcod);

        // Clear all objects.
        for object in &game_state.objects {
            if let Some((x, y)) = game_state.to_camera_coordinates(object.x, object.y) {
                tcod.con.put_char(x, y, ' ', BackgroundFlag::None);
            }
        }

        let player_action = match key {
            Key { code: KeyCode::Escape, .. } => PlayerAction::Exit,
            Key { code: KeyCode::Enter, left_alt: true, .. } => {
                let fullscreen = !tcod.root.is_fullscreen();
                tcod.root.set_fullscreen(fullscreen);
                PlayerAction::DidntTakeTurn
            },
            Key { code: KeyCode::Number0, .. } => {
                game_state.disable_fov = !game_state.disable_fov;
                PlayerAction::DidntTakeTurn
            },
            key => game_state.handle_keys(key, tcod),
        };

        if player_action == PlayerAction::Exit {
            game_state.save()
                .expect("Failed to save the game.");
            break;
        }

        // Let monsters take their turn.
        if game_state.objects[PLAYER].alive && player_action != PlayerAction::DidntTakeTurn {
            // Skip the first object, which should be the player.
            for id in 0..game_state.objects.len() {
                if game_state.objects[id].ai.is_some() {
                    game_state.ai_take_turn(id);
                }
            }
        }
    }
}

fn msgbox(text: &str, width: i32, root: &mut Root) {
    let options: &[&str] = &[];
    menu(text, options, width, root);
}

fn main_menu(tcod: &mut Tcod) {
    let img = tcod::image::Image::from_file("assets/menu_background.png")
        .ok().expect("Background image not found");

    while !tcod.root.window_closed() {
        // Show the background image, at twice the regular console resolution.
        tcod::image::blit_2x(&img, (0, 0), (-1, -1), &mut tcod.root, (0, 0));

        tcod.root.set_default_foreground(colors::LIGHT_YELLOW);
        tcod.root.print_ex(SCREEN_WIDTH / 2, SCREEN_HEIGHT / 2 - 4,
                           BackgroundFlag::None, TextAlignment::Center,
                           "TOMBS OF THE ANCIENT KINGS");
        tcod.root.print_ex(SCREEN_WIDTH / 2, SCREEN_HEIGHT - 2,
                           BackgroundFlag::None, TextAlignment::Center,
                           "By Mystal");

        // Show options and wait for the player's choice.
        let choices = &["Play a new game", "Continue last game", "Quit"];
        let choice = menu("", choices, 24, &mut tcod.root);

        match choice {
            // New game.
            Some(0) => {
                let mut game_state = GameState::new();
                play_game(&mut game_state, tcod);
            },
            // Load game.
            Some(1) => match GameState::from_save() {
                Ok(mut game_state) => play_game(&mut game_state, tcod),
                Err(_) => {
                    msgbox("\nNo saved game to load.\n", 24, &mut tcod.root);
                    continue;
                }
            },
            // Quit.
            Some(2) => break,
            _ => {}
        }
    }
}

fn main() {
    tcod::system::set_fps(LIMIT_FPS);

    let root = Root::initializer()
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust libtcod tutorial")
        .font("assets/arial10x10.png", tcod::FontLayout::Tcod)
        .font_type(tcod::FontType::Greyscale)
        .init();

    let mut tcod = Tcod {
        root: root,
        con: Offscreen::new(map::MAP_WIDTH, map::MAP_HEIGHT),
        panel: Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT),
    };

    main_menu(&mut tcod);
}
