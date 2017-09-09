use std::cmp;

use rand::{self, Rng, ThreadRng};
use rand::distributions::{IndependentSample, Weighted, WeightedChoice};
use tcod::colors;

use equipment::*;
use object::*;

pub const MAP_WIDTH: i32 = 100;
pub const MAP_HEIGHT: i32 = 100;
pub const ROOM_MAX_SIZE: i32 = 10;
pub const ROOM_MIN_SIZE: i32 = 6;
pub const MAX_ROOMS: i32 = 30;

struct Transition {
    level: u32,
    value: u32,
}

/// Returns a value that depends on level. the table specifies what
/// value occurs after each level, default is 0.
fn from_dungeon_level(table: &[Transition], level: u32) -> u32 {
    table.iter()
        .rev()
        .find(|transition| level >= transition.level)
        .map_or(0, |transition| transition.value)
}

// TODO: Make this a 1D Vec with coordinate accessors.
pub type Map = Vec<Vec<Tile>>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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

pub fn make_map(objects: &mut Vec<Object>, level: u32) -> Map {
    // Player is the first element, remove everything else.
    objects.truncate(1);

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

                // Add some content to this room, such as monsters.
                // NOTE: No objects are placed in the player's starting room.
                place_objects(new_room, &map, objects, level, &mut rng);
            }

            // Finally, append the new room to the list.
            rooms.push(new_room);
        }
    }

    // Create stairs at the center of the last room.
    let (last_room_x, last_room_y) = rooms[rooms.len() - 1].center();
    let mut stairs = Object::new(last_room_x, last_room_y, '>', "stairs", colors::WHITE, false);
    stairs.always_visible = true;
    objects.push(stairs);

    map
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>, level: u32, rng: &mut ThreadRng) {
    let max_monsters = from_dungeon_level(&[
        Transition {level: 1, value: 2},
        Transition {level: 4, value: 3},
        Transition {level: 6, value: 5},
    ], level);

    // Choose random number of monsters.
    let num_monsters = rng.gen_range(0, max_monsters + 1);

    // Monster random table.
    let troll_chance = from_dungeon_level(&[
        Transition {level: 3, value: 15},
        Transition {level: 5, value: 30},
        Transition {level: 7, value: 60},
    ], level);

    let monster_chances = &mut [
        Weighted {weight: 80, item: "orc"},
        Weighted {weight: troll_chance, item: "troll"},
    ];

    let monster_choice = WeightedChoice::new(monster_chances);

    for _ in 0..num_monsters {
        // Choose random spot for this monster.
        let x = rng.gen_range(room.x1 + 1, room.x2);
        let y = rng.gen_range(room.y1 + 1, room.y2);


        // Only place it if the tile is not blocked.
        if !is_blocked(x, y, map, objects) {
            let mut monster = match monster_choice.ind_sample(rng) {
                "orc" => {
                    // Create an orc.
                    let mut orc = Object::new(x, y, 'o', "orc", colors::DESATURATED_GREEN, true);
                    orc.fighter = Some(Fighter {
                        max_hp: 20,
                        hp: 20,
                        defense: 0,
                        power: 4,
                        xp: 35,
                        on_death: DeathCallback::Monster,
                    });
                    orc.ai = Some(Ai::Basic);
                    orc
                }
                "troll" => {
                    // Create a troll.
                    let mut troll = Object::new(x, y, 'T', "troll", colors::DARKER_GREEN, true);
                    troll.fighter = Some(Fighter {
                        max_hp: 30,
                        hp: 30,
                        defense: 2,
                        power: 8,
                        xp: 100,
                        on_death: DeathCallback::Monster,
                    });
                    troll.ai = Some(Ai::Basic);
                    troll
                }
                _ => unreachable!(),
            };
            monster.alive = true;

            objects.push(monster);
        }
    }

    // Maximum number of items per room.
    let max_items = from_dungeon_level(&[
        Transition {level: 1, value: 1},
        Transition {level: 4, value: 2},
    ], level);

    // Choose random number of items.
    let num_items = rng.gen_range(0, max_items + 1);

    // Item random table.
    let item_chances = &mut [
        // healing potion always shows up, even if all other items have 0 chance
        Weighted {weight: 35, item: Item::Heal},
        Weighted {weight: 1000, item: Item::Equipment},
        Weighted {weight: from_dungeon_level(&[Transition{level: 4, value: 25}], level),
                  item: Item::Lightning},
        Weighted {weight: from_dungeon_level(&[Transition{level: 6, value: 25}], level),
                  item: Item::Fireball},
        Weighted {weight: from_dungeon_level(&[Transition{level: 2, value: 10}], level),
                  item: Item::Confuse},
    ];

    let item_choice = WeightedChoice::new(item_chances);

    for _ in 0..num_items {
        // Choose random spot for this item.
        let x = rng.gen_range(room.x1 + 1, room.x2);
        let y = rng.gen_range(room.y1 + 1, room.y2);

        // Only place it if the tile is not blocked.
        if !is_blocked(x, y, map, objects) {
            let mut item = match item_choice.ind_sample(rng) {
                Item::Heal => {
                    // Create a healing potion.
                    let mut object = Object::new(x, y, '!', "healing potion", colors::VIOLET, false);
                    object.item = Some(Item::Heal);
                    object
                }
                Item::Lightning => {
                    // Create a lightning bolt scroll.
                    let mut object = Object::new(x, y, '#', "scroll of lightning bolt",
                                                 colors::LIGHT_YELLOW, false);
                    object.item = Some(Item::Lightning);
                    object
                }
                Item::Fireball => {
                    // Create a fireball scroll.
                    let mut object = Object::new(x, y, '#', "scroll of fireball",
                                                 colors::LIGHT_YELLOW, false);
                    object.item = Some(Item::Fireball);
                    object
                }
                Item::Confuse => {
                    // Create a confuse scroll.
                    let mut object = Object::new(x, y, '#', "scroll of confusion",
                                                 colors::LIGHT_YELLOW, false);
                    object.item = Some(Item::Confuse);
                    object
                }
                Item::Equipment => {
                    // Create a sword.
                    let mut object = Object::new(x, y, '/', "sword", colors::SKY, false);
                    object.item = Some(Item::Equipment);
                    object.equippable = Some(Equippable{equipped: false, slot: Slot::RightHand});
                    object
                }
            };
            item.always_visible = true;
            objects.push(item);
        }
    }
}
