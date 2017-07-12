use std::cmp;

use rand::{self, Rng};
use tcod::bsp::{Bsp, TraverseOrder};

pub const MAP_WIDTH: i32 = 80;
pub const MAP_HEIGHT: i32 = 45;

pub const ROOM_MAX_SIZE: i32 = 10;
pub const ROOM_MIN_SIZE: i32 = 6;
pub const MAX_ROOMS: i32 = 30;

pub const BSP_DEPTH: i32 = 10;
pub const BSP_MIN_SIZE: i32 = 5;
pub const BSP_FULL_ROOMS: bool = true;

// TODO: Make this a 1D Vec with coordinate accessors.
pub type Map = Vec<Vec<Tile>>;

#[derive(Clone, Copy, Debug)]
pub struct Tile {
    pub blocked: bool,
    pub block_sight: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile {
            blocked: false,
            block_sight: false,
        }
    }

    pub fn wall() -> Self {
        Tile {
            blocked: true,
            block_sight: true,
        }
    }
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

fn create_room(map: &mut Map, room: Rect) {
    // NOTE: Room are bounded by walls, thus we exclude the start/end coordinates.
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(map: &mut Map, x1: i32, x2: i32, y: i32) {
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_h_tunnel_left(map: &mut Map, mut x: i32, y: i32) {
    while x >= 0 && map[x as usize][y as usize].blocked {
        map[x as usize][y as usize] = Tile::empty();
        x -= 1;
    }
}

fn create_h_tunnel_right(map: &mut Map, mut x: i32, y: i32) {
    while x < MAP_WIDTH && map[x as usize][y as usize].blocked {
        map[x as usize][y as usize] = Tile::empty();
        x += 1;
    }
}

fn create_v_tunnel(map: &mut Map, x: i32, y1: i32, y2: i32) {
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel_up(map: &mut Map, x: i32, mut y: i32) {
    while y >= 0 && map[x as usize][y as usize].blocked {
        map[x as usize][y as usize] = Tile::empty();
        y -= 1;
    }
}

fn create_v_tunnel_down(map: &mut Map, x: i32, mut y: i32) {
    while y < MAP_HEIGHT && map[x as usize][y as usize].blocked {
        map[x as usize][y as usize] = Tile::empty();
        y += 1;
    }
}

pub fn make_map() -> (Map, (i32, i32)) {
    let mut rng = rand::thread_rng();

    // Fill the map with "unblocked" tiles.
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    let mut rooms = vec![];

    let mut starting_position = (0, 0);

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
            create_room(&mut map, new_room);

            // Center coordinates of the new room, will be useful later.
            let (new_x, new_y) = new_room.center();

            if rooms.is_empty() {
                // This is the first room, where the player starts at.
                starting_position = (new_x, new_y);
            } else {
                // All rooms after the first:
                // Connect it to the previous room with a tunnel.

                // Center coordinates of the previous room.
                let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

                // Flip a coin (random bool value -- either true or false).
                if rand::random() {
                    // First move horizontally, then vertically.
                    create_h_tunnel(&mut map, prev_x, new_x, prev_y);
                    create_v_tunnel(&mut map, new_x, prev_y, new_y);
                } else {
                    // First move vertically, then horizontally.
                    create_v_tunnel(&mut map, prev_x, prev_y, new_y);
                    create_h_tunnel(&mut map, prev_x, new_x, new_y);
                }
            }

            // Finally, append the new room to the list.
            rooms.push(new_room);
        }
    }

    (map, starting_position)
}

//fn vline(map, x, y1, y2) {
//    if y1 > y2 {
//        y1,y2 = y2,y1
//    }
//
//    for y in range(y1,y2+1) {
//        map[x][y].blocked = False
//        map[x][y].block_sight = False
//    }
//}
//
//fn vline_up(map, x, y) {
//    while y >= 0 and map[x][y].blocked == True {
//        map[x][y].blocked = False
//        map[x][y].block_sight = False
//        y -= 1
//    }
//}
//
//fn vline_down(map, x, y) {
//    while y < MAP_HEIGHT and map[x][y].blocked == True {
//        map[x][y].blocked = False
//        map[x][y].block_sight = False
//        y += 1
//    }
//}
//
//fn hline(map, x1, y, x2) {
//    if x1 > x2 {
//        x1,x2 = x2,x1
//    }
//    for x in range(x1,x2+1) {
//        map[x][y].blocked = False
//        map[x][y].block_sight = False
//    }
//}
//
//fn hline_left(map, x, y) {
//    while x >= 0 and map[x][y].blocked == True {
//        map[x][y].blocked = False
//        map[x][y].block_sight = False
//        x -= 1
//    }
//}
//
//fn hline_right(map, x, y) {
//    while x < MAP_WIDTH and map[x][y].blocked == True {
//        map[x][y].blocked = False
//        map[x][y].block_sight = False
//        x += 1
//    }
//}

pub fn make_bsp_map() -> (Map, (i32, i32)) {
    let mut rng = rand::thread_rng();

    // Fill the map with "unblocked" tiles.
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    let mut rooms = vec![];

    // New root node.
    let mut bsp = Bsp::new_with_size(0, 0, MAP_WIDTH, MAP_HEIGHT);

    // Split into nodes.
    bsp.split_recursive(None, BSP_DEPTH, BSP_MIN_SIZE + 1, BSP_MIN_SIZE + 1, 1.5, 1.5);

    // Traverse the nodes and create rooms.
    bsp.traverse(TraverseOrder::InvertedLevelOrder, |node| {
        if node.is_leaf() {
            // Create rooms.
            let mut min_x = node.x;
            let mut max_x = node.x + node.w - 1;
            let mut min_y = node.y;
            let mut max_y = node.y + node.h - 1;
            println!("Min: ({}, {}), Max: ({}, {})", min_x, min_y, max_x, max_y);

            if max_x == MAP_WIDTH {
                max_x -= 1;
            }
            if max_y == MAP_HEIGHT {
                max_y -= 1;
            }

            // If it's False the rooms sizes are random, else the rooms are filled to the node's size
            if !BSP_FULL_ROOMS {
                //println!("Min: ({}, {}), Max: ({}, {})", min_x, min_y, max_x, max_y);
                min_x = rng.gen_range(min_x, max_x - BSP_MIN_SIZE + 1);
                min_y = rng.gen_range(min_y, max_y - BSP_MIN_SIZE + 1);
                max_x = rng.gen_range(min_x + BSP_MIN_SIZE - 1, max_x);
                max_y = rng.gen_range(min_y + BSP_MIN_SIZE - 1, max_y);
            }

            node.x = min_x;
            node.y = min_y;
            node.w = max_x - min_x + 1;
            node.h = max_y - min_y + 1;

            // Dig room.
            let room = Rect { x1: min_x, y1: min_y, x2: max_x, y2: max_y };
            create_room(&mut map, room);

            // Add center coordinates to the list of rooms
            rooms.push(room.center());
        } else {
            // Create corridors.
            let left = node.left()
                .expect("Inner node should have a left child.");
            let right = node.right()
                .expect("Inner node should have a right child.");

            node.x = cmp::min(left.x, right.x);
            node.y = cmp::min(left.y, right.y);
            node.w = cmp::max(left.x + left.w, right.x + right.w) - node.x;
            node.h = cmp::max(left.y + left.h, right.y + right.h) - node.y;

            if node.horizontal {
                if left.x + left.w - 1 < right.x || right.x + right.w - 1 < left.x {
                    let x1 = rng.gen_range(left.x, left.x + left.w - 1);
                    let x2 = rng.gen_range(right.x, right.x + right.w - 1);
                    let y = rng.gen_range(left.y + left.h, right.y);
                    create_v_tunnel_up(&mut map, x1, y - 1);
                    create_h_tunnel(&mut map, x1, x2, y);
                    create_v_tunnel_down(&mut map, x2, y + 1);
                } else {
                    let minx = cmp::max(left.x, right.x);
                    let maxx = cmp::min(left.x + left.w - 1, right.x + right.w - 1);
                    let x = rng.gen_range(minx, maxx);
                    create_v_tunnel_down(&mut map, x, right.y);
                    create_v_tunnel_up(&mut map, x, right.y - 1);
                }
            } else {
                if left.y + left.h - 1 < right.y || right.y + right.h - 1 < left.y {
                    let y1 = rng.gen_range(left.y, left.y + left.h - 1);
                    let y2 = rng.gen_range(right.y, right.y + right.h - 1);
                    let x = rng.gen_range(left.x + left.w, right.x);
                    create_h_tunnel_left(&mut map, x - 1, y1);
                    create_v_tunnel(&mut map, x, y1, y2);
                    create_h_tunnel_right(&mut map, x + 1, y2);
                } else {
                    let miny = cmp::max(left.y, right.y);
                    let maxy = cmp::min(left.y + left.h - 1, right.y + right.h - 1);
                    let y = rng.gen_range(miny, maxy);
                    create_h_tunnel_left(&mut map, right.x - 1, y);
                    create_h_tunnel_right(&mut map, right.x, y);
                }
            }
        }
        true
    });

    // Random room for player start.
    let starting_position = rand::sample(&mut rng, rooms, 1)[0];

    (map, starting_position)
}
