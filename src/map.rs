use std::cmp;

use rand::{self, Rng, ThreadRng};
use tcod::colors;

use object::*;

pub const MAP_WIDTH: i32 = 100;
pub const MAP_HEIGHT: i32 = 100;
pub const ROOM_MAX_SIZE: i32 = 10;
pub const ROOM_MIN_SIZE: i32 = 6;
pub const MAX_ROOMS: i32 = 30;
pub const MAX_ROOM_MONSTERS: i32 = 3;
pub const MAX_ROOM_ITEMS: i32 = 2;

// TODO: Make this a 1D Vec with coordinate accessors.
pub type Map = Vec<Vec<Tile>>;

#[derive(Clone, Copy, Debug)]
pub struct Tile {
    pub blocked: bool,
    pub block_sight: bool,
    pub explored: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile {
            blocked: false,
            block_sight: false,
            explored: false,
        }
    }

    pub fn wall() -> Self {
        Tile {
            blocked: true,
            block_sight: true,
            explored: false,
        }
    }
}

pub fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    // First test the map tile.
    if map[x as usize][y as usize].blocked {
        return true;
    }

    // Now check for any blocking objects.
    objects.iter().any(|object| {
        object.blocks && object.pos() == (x, y)
    })
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Rect {
            x1: x,
            y1: y,
            x2: x + width,
            y2: y + height,
        }
    }

    pub fn center(&self) -> (i32, i32) {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;
        (center_x, center_y)
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2) && (self.x2 >= other.x1) &&
            (self.y1 <= other.y2) && (self.y2 >= other.y1)
    }
}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

pub fn make_map(objects: &mut Vec<Object>) -> Map {
    let mut rng = rand::thread_rng();

    // Fill the map with "unblocked" tiles.
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        // Random width and height.
        let width = rng.gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let height = rng.gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        // Random position without going out of the boundaries of the map.
        let x = rng.gen_range(0, MAP_WIDTH - width);
        let y = rng.gen_range(0, MAP_HEIGHT - height);

        let new_room = Rect::new(x, y, width, height);

        // Run through the other rooms and see if they intersect with this one.
        let failed = rooms.iter()
            .any(|other_room| new_room.intersects(other_room));

        if !failed {
            // No intersections, so this room is valid.

            // "Paint" it to the map's tiles.
            create_room(new_room, &mut map);

            // Center coordinates of the new room, will be useful later.
            let (new_x, new_y) = new_room.center();

            if rooms.is_empty() {
                // This is the first room, where the player starts at.
                objects[0].set_pos(new_x, new_y);
            } else {
                // All rooms after the first:
                // Connect it to the previous room with a tunnel.

                // Center coordinates of the previous room.
                let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

                // Flip a coin (random bool value -- either true or false).
                if rng.gen() {
                    // First move horizontally, then vertically.
                    create_h_tunnel(prev_x, new_x, prev_y, &mut map);
                    create_v_tunnel(prev_y, new_y, new_x, &mut map);
                } else {
                    // First move vertically, then horizontally.
                    create_v_tunnel(prev_y, new_y, prev_x, &mut map);
                    create_h_tunnel(prev_x, new_x, new_y, &mut map);
                }
            }

            // Add some content to this room, such as monsters.
            // NOTE: Do it after placing the player, so new objects don't overlap the player.
            place_objects(new_room, &map, objects, &mut rng);

            // Finally, append the new room to the list.
            rooms.push(new_room);
        }
    }

    map
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>, rng: &mut ThreadRng) {
    // Choose random number of monsters.
    let num_monsters = rng.gen_range(0, MAX_ROOM_MONSTERS + 1);

    for _ in 0..num_monsters {
        // Choose random spot for this monster.
        let x = rng.gen_range(room.x1 + 1, room.x2);
        let y = rng.gen_range(room.y1 + 1, room.y2);


        // Only place it if the tile is not blocked.
        if !is_blocked(x, y, map, objects) {
            let orc_chance = 0.8; // 80% chance of getting an orc.
            let mut monster = if rng.gen::<f32>() < orc_chance {
                // Create an orc.
                let mut orc = Object::new(x, y, 'o', "orc", colors::DESATURATED_GREEN, true);
                orc.fighter = Some(Fighter {
                    max_hp: 10,
                    hp: 10,
                    defense: 0,
                    power: 3,
                    on_death: DeathCallback::Monster,
                });
                orc.ai = Some(Ai::Basic);
                orc
            } else {
                // Create a troll.
                let mut troll = Object::new(x, y, 'T', "troll", colors::DARKER_GREEN, true);
                troll.fighter = Some(Fighter {
                    max_hp: 16,
                    hp: 16,
                    defense: 1,
                    power: 4,
                    on_death: DeathCallback::Monster,
                });
                troll.ai = Some(Ai::Basic);
                troll
            };
            monster.alive = true;

            objects.push(monster);
        }
    }

    // Choose random number of items.
    let num_items = rng.gen_range(0, MAX_ROOM_ITEMS + 1);

    for _ in 0..num_items {
        // Choose random spot for this item.
        let x = rng.gen_range(room.x1 + 1, room.x2);
        let y = rng.gen_range(room.y1 + 1, room.y2);

        // TODO: Consider using WeightedChoice here.

        // Only place it if the tile is not blocked.
        if !is_blocked(x, y, map, objects) {
            let dice = rng.gen::<f32>();
            let item = if dice < 0.7 {
                // Create a healing potion (70% chance).
                let mut object = Object::new(x, y, '!', "healing potion", colors::VIOLET, false);
                object.item = Some(Item::Heal);
                object
            } else if dice < 0.7 + 0.1 {
                // Create a lightning bolt scroll (10% chance).
                let mut object = Object::new(x, y, '#', "scroll of lightning bolt",
                                             colors::LIGHT_YELLOW, false);
                object.item = Some(Item::Lightning);
                object
            } else if dice < 0.7 + 0.1 + 0.1 {
                // Create a fireball scroll (10% chance).
                let mut object = Object::new(x, y, '#', "scroll of fireball",
                                             colors::LIGHT_YELLOW, false);
                object.item = Some(Item::Fireball);
                object
            } else {
                // Create a confuse scroll (10% chance).
                let mut object = Object::new(x, y, '#', "scroll of confusion",
                                             colors::LIGHT_YELLOW, false);
                object.item = Some(Item::Confuse);
                object
            };
            objects.push(item);
        }
    }
}
