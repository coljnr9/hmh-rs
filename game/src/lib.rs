use anyhow::Result;
use std::{f64::consts, ops::Add};

use shared::{
    AudioBuffer, AudioBufferRaw, GameButtonId, GameInput, GameMemory, GraphicsBuffer,
    GraphicsBufferRaw, PlatformApi,
};

const CHUNK_DIM: usize = 64;

const TILE_SIDE_IN_METERS: f64 = 1.4;
const TILE_SIDE_IN_PIXELS: usize = 50;

const PLAYER_WIDTH: f64 = 0.5 * TILE_SIDE_IN_METERS;
const PLAYER_HEIGHT: f64 = 0.75 * TILE_SIDE_IN_METERS;

const SPEED_FACTOR: f64 = 20.0; // m/s

#[repr(C)]
#[derive(Default)]
pub struct GameState {
    x_offset: i64,
    y_offset: i64,
    // Temporarily used to track sine-wave angle between audio calls
    theta: f64,

    player_position: CannonicalPosition,
}

/// Player position as a combination of which tile chunk they are in as well as where within that
/// tile they are.
#[derive(Debug, Default, Copy, Clone)]
struct CannonicalPosition {
    // Tile coordinates w/in the world
    global_x: TileCoord,
    global_y: TileCoord,

    // Coord w/in a tile
    tile_rel_x: f64,
    tile_rel_y: f64,
}

impl CannonicalPosition {
    fn is_empty(&self, world: &World) -> bool {
        let mut empty = false;

        if let Some(tile_chunk) = world.get_tile_chunk(self.global_x, self.global_y) {
            let val = tile_chunk.value_at(self.global_x.tile, self.global_y.tile);
            empty = val == 0;
        }

        empty
    }

    // I don't think this is chunk relative, it's global?
    fn to_meters_chunk_relative(self) -> (f64, f64) {
        let x = self.global_x.chunk as f64 * CHUNK_DIM as f64 * TILE_SIDE_IN_METERS
            + self.global_x.tile as f64 * TILE_SIDE_IN_METERS
            + self.tile_rel_x;
        let y = self.global_y.chunk as f64 * CHUNK_DIM as f64 * TILE_SIDE_IN_METERS
            + self.global_y.tile as f64 * TILE_SIDE_IN_METERS
            + self.tile_rel_y;
        (x, y)
    }
}

impl Add<(f64, f64)> for CannonicalPosition {
    type Output = Self;

    fn add(self, rhs: (f64, f64)) -> Self::Output {
        let mut out = Self { ..self };
        let x_offset = rhs.0;
        let y_offset = rhs.1;
        out.tile_rel_x += x_offset;
        out.tile_rel_y += y_offset;

        out
    }
}

#[derive(Default)]
struct World {
    tile_chunk_count_x: i64,

    #[allow(unused)]
    tile_chunk_count_y: i64,

    tile_side_in_meters: f64,
    pixels_per_meter: f64,

    tile_chunks: [TileChunk; N_CHUNKS],
}

impl World {
    fn get_tile_chunk(&self, abs_tile_x: TileCoord, abs_tile_y: TileCoord) -> Option<&TileChunk> {
        let valid_range = 0..OUTPUT_DIM as i64;
        let all_valid =
            valid_range.contains(&abs_tile_x.chunk) && valid_range.contains(&abs_tile_y.chunk);
        if !all_valid {
            return None;
        }

        // Guaranteed to be valid given the check above.  tile_chunks has size OUTPUT_DIM *
        // OUTPUT_DIM
        let idx = abs_tile_y.chunk * self.tile_chunk_count_x + abs_tile_x.chunk;

        Some(&self.tile_chunks[idx as usize])
    }
}

/// Draw a rectangle given _pixel_ coordinates
#[allow(clippy::too_many_arguments)]
fn draw_rectangle_pixels(
    graphics_buffer: &mut GraphicsBuffer,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    red: f64,
    green: f64,
    blue: f64,
) {
    // TODO(coljnr9) Want to assert that mins are <= maxes?

    let min_x = min_x
        .clamp(0.0, graphics_buffer.width_pixels as f64)
        .round() as usize;
    let max_x = max_x
        .clamp(0.0, graphics_buffer.width_pixels as f64)
        .round() as usize;
    let min_y = min_y
        .clamp(0.0, graphics_buffer.height_pixels as f64)
        .round() as usize;
    let max_y = max_y
        .clamp(0.0, graphics_buffer.height_pixels as f64)
        .round() as usize;

    let rows = &mut bytemuck::cast_slice_mut::<u8, u32>(graphics_buffer.pixels)
        [min_y * graphics_buffer.pitch_pixels..(max_y) * graphics_buffer.pitch_pixels]
        .chunks_mut(graphics_buffer.pitch_pixels);

    let color = u32::from_be_bytes([
        255,
        (red * 255.0).round() as u8,
        (green * 255.0).round() as u8,
        (blue * 255.0).round() as u8,
    ]);

    for row in rows {
        row[min_x..(max_x)].iter_mut().for_each(|p| *p = color);
    }
}

const OUTPUT_DIM: usize = MAP_H / CHUNK_DIM;
const N_CHUNKS: usize = (MAP_W * MAP_H) / (CHUNK_DIM * CHUNK_DIM);
pub fn game_update_and_render_internal(
    game_state: &mut GameState,
    graphics_buffer: &mut GraphicsBuffer,
    game_input: &GameInput,
    _platform_api: &PlatformApi,
) -> Result<()> {
    let mut tile_chunks = [TileChunk {
        tiles: [0u8; CHUNK_DIM * CHUNK_DIM],
    }; N_CHUNKS];

    let mut all_chunks = [[0u8; CHUNK_DIM * CHUNK_DIM]; N_CHUNKS];
    let mut chunk_idx = 0;

    // 2d-array to chunks
    for r_i in 0..OUTPUT_DIM {
        for c_i in 0..OUTPUT_DIM {
            let c_start = c_i * CHUNK_DIM;
            let c_end = (c_i + 1) * CHUNK_DIM;

            let r_start = r_i * CHUNK_DIM;
            let r_end = (r_i + 1) * CHUNK_DIM;

            for r in r_start..r_end {
                for c in c_start..c_end {
                    let v = TILEMAP[r][c];
                    let idx = (r - r_start) * CHUNK_DIM + (c - c_start);
                    all_chunks[chunk_idx][idx] = v;
                }
            }
            tile_chunks[chunk_idx] = TileChunk {
                tiles: all_chunks[chunk_idx],
            };
            chunk_idx += 1;
        }
    }

    let world = World {
        tile_chunk_count_x: OUTPUT_DIM as i64,
        tile_chunk_count_y: OUTPUT_DIM as i64,
        tile_side_in_meters: TILE_SIDE_IN_METERS,
        pixels_per_meter: TILE_SIDE_IN_PIXELS as f64 / TILE_SIDE_IN_METERS,
        tile_chunks,
    };
    draw_rectangle_pixels(
        graphics_buffer,
        0.0,
        0.0,
        graphics_buffer.width_pixels as f64,
        graphics_buffer.height_pixels as f64,
        1.0,
        0.0,
        1.0,
    );

    let x_start = game_state.player_position.global_x + -15;
    let x_end = game_state.player_position.global_x + 15;

    let y_start = game_state.player_position.global_y + -15;
    let y_end = game_state.player_position.global_y + 15;

    let x_start = cannonicalize_coordinate(x_start, 0.0, &world).0;
    let y_start = cannonicalize_coordinate(y_start, 0.0, &world).0;

    let x_end = cannonicalize_coordinate(x_end, 0.0, &world).0;
    let y_end = cannonicalize_coordinate(y_end, 0.0, &world).0;

    dbg!(x_start, x_end, y_start, y_end);
    for y in -15..15 {
        for x in -15..15 {
            let x_coord_ =
                cannonicalize_coordinate(game_state.player_position.global_x + x, 0.0, &world).0;
            let y_coord_ =
                cannonicalize_coordinate(game_state.player_position.global_y + y, 0.0, &world).0;

            let tile_chunk = match world.get_tile_chunk(x_coord_, y_coord_) {
                Some(t) => t,
                None => continue,
            };

            let grey = if tile_chunk.value_at(x_coord_.tile, y_coord_.tile) > 0 {
                1.0
            } else {
                0.5
            };

            let upper_corner = CannonicalPosition {
                global_x: x_coord_,
                global_y: y_coord_,
                tile_rel_x: 0.0,
                tile_rel_y: 0.0,
            };

            let (x, y) = upper_corner.to_meters_chunk_relative();

            let min_x = x;
            let min_y = y;
            let max_x = x + world.tile_side_in_meters;
            let max_y = y + world.tile_side_in_meters;

            draw_rectangle_pixels(
                graphics_buffer,
                min_x * world.pixels_per_meter,
                min_y * world.pixels_per_meter,
                max_x * world.pixels_per_meter,
                max_y * world.pixels_per_meter,
                grey,
                grey,
                grey,
            );
        }
    }

    let mut player_x_delta = 0.0;
    let mut player_y_delta = 0.0;

    if game_input.buttons[GameButtonId::Up].ended_down {
        player_y_delta = -SPEED_FACTOR;
    }
    if game_input.buttons[GameButtonId::Down].ended_down {
        player_y_delta = 1.0 * SPEED_FACTOR;
    }
    if game_input.buttons[GameButtonId::Left].ended_down {
        player_x_delta = -SPEED_FACTOR;
    }
    if game_input.buttons[GameButtonId::Right].ended_down {
        player_x_delta = 1.0 * SPEED_FACTOR;
    }

    let new_position = game_state.player_position
        + (
            (game_input.dt * player_x_delta),
            (game_input.dt * player_y_delta),
        );

    let mut left = new_position;
    left.tile_rel_x -= PLAYER_WIDTH / 2.0;

    let mut right = new_position;
    right.tile_rel_x += PLAYER_WIDTH / 2.0;

    let new_position = cannonicalize_position(new_position, &world);
    let pos0 = cannonicalize_position(left, &world);
    let pos1 = cannonicalize_position(right, &world);

    if pos0.is_empty(&world) && pos1.is_empty(&world) {
        game_state.player_position = new_position;
    }

    let (x, y) = new_position.to_meters_chunk_relative();

    draw_rectangle_pixels(
        graphics_buffer,
        (x - PLAYER_WIDTH / 2.0) * world.pixels_per_meter,
        (y - PLAYER_HEIGHT) * world.pixels_per_meter,
        (x + PLAYER_WIDTH / 2.0) * world.pixels_per_meter,
        y * world.pixels_per_meter,
        0.0,
        1.0,
        0.0,
    );

    Ok(())
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
struct TileCoord {
    chunk: i64,
    tile: i64,
}

impl Add<i64> for TileCoord {
    type Output = Self;

    fn add(self, rhs: i64) -> Self::Output {
        let total = self.tile + rhs;

        Self {
            chunk: self.chunk + total.div_euclid(CHUNK_DIM as i64),
            tile: total.rem_euclid(CHUNK_DIM as i64),
        }
    }
}

fn cannonicalize_coordinate(coord: TileCoord, tile_rel: f64, world: &World) -> (TileCoord, f64) {
    let offset = (tile_rel / world.tile_side_in_meters).floor() as i64;
    let tile_coord = coord + offset;

    let rem = tile_rel - offset as f64 * world.tile_side_in_meters;

    assert!(tile_coord.tile >= 0);
    assert!(tile_coord.tile < CHUNK_DIM as i64);
    // assert!(tile_coord.chunk >= 0);
    // assert!(tile_coord.chunk < OUTPUT_DIM as i64);

    assert!(rem >= 0.0);
    assert!(rem <= TILE_SIDE_IN_METERS);
    (tile_coord, rem)
}

fn cannonicalize_position(position: CannonicalPosition, world: &World) -> CannonicalPosition {
    let (global_x, tile_rel_x) =
        cannonicalize_coordinate(position.global_x, position.tile_rel_x, world);
    let (global_y, tile_rel_y) =
        cannonicalize_coordinate(position.global_y, position.tile_rel_y, world);

    CannonicalPosition {
        global_x,
        global_y,
        tile_rel_x,
        tile_rel_y,
    }
}

#[derive(Copy, Clone)]
struct TileChunk {
    tiles: [u8; CHUNK_DIM * CHUNK_DIM],
}

impl Default for TileChunk {
    fn default() -> Self {
        Self {
            tiles: [0u8; CHUNK_DIM * CHUNK_DIM],
        }
    }
}

impl TileChunk {
    fn value_at(&self, x: i64, y: i64) -> u8 {
        let row = CHUNK_DIM as i64 * y;
        let col = x;

        self.tiles[(row + col) as usize]
    }
}

pub fn game_audio_render_internal(game_state: &mut GameState, audio_buffer: &mut AudioBuffer) {
    for (d_theta, frame) in audio_buffer
        .samples_buf
        .as_chunks_mut::<2>()
        .0
        .iter_mut()
        .enumerate()
    {
        let value = 200.0
            * (2.0 * consts::PI * (1.0 * 440.0) * (game_state.theta + d_theta as f64)
                / audio_buffer.sample_rate as f64)
                .sin();
        frame.copy_from_slice(&[value as i16, value as i16]);
    }
    game_state.theta += (audio_buffer.samples_buf.len() / 2) as f64;
}

/// # Safety
///
/// See game_state_from_memory
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_update_and_render(
    memory: *mut GameMemory,
    input: *const GameInput,
    buffer: *mut GraphicsBufferRaw,
) -> bool {
    let game_state = unsafe { game_state_from_memory(&mut *memory) };
    let mut graphics_buffer = unsafe { GraphicsBuffer::from_raw(*buffer) };
    let game_input = unsafe { &*input };

    // TODO(deferring this)
    let platform_api = &PlatformApi;

    let res =
        game_update_and_render_internal(game_state, &mut graphics_buffer, game_input, platform_api);
    res.is_ok()
}

/// # Safety
///
/// See game_state_from_memory
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_audio_render(
    memory: *mut GameMemory,
    audio_buffer: *mut AudioBufferRaw,
) -> bool {
    let game_state = unsafe { game_state_from_memory(&mut *memory) };
    let mut audio_buffer = unsafe { AudioBuffer::from_raw(*audio_buffer) };

    game_audio_render_internal(game_state, &mut audio_buffer);
    true
}

/// Safety: Requires that memory.permanent is:
/// 1 large enough for GameState
/// 2 aligned
/// 3 zeroed when uninitialized
///
/// 2 and 3 must be enforced by the allocation semantics in the platform layer
unsafe fn game_state_from_memory(memory: &mut GameMemory) -> &mut GameState {
    // Enforce Safety #1
    assert!(memory.permanent_size >= size_of::<GameState>());

    let state_ptr = memory.permanent as *mut GameState;
    if !memory.is_initialized {
        let game_state = GameState {
            x_offset: 0,
            y_offset: 0,
            theta: 0.0,
            player_position: CannonicalPosition {
                tile_rel_x: 1.0,
                tile_rel_y: 1.0,
                global_x: TileCoord { chunk: 0, tile: 1 },
                global_y: TileCoord { chunk: 0, tile: 1 },
            },
        };

        unsafe { state_ptr.write(game_state) };
        memory.is_initialized = true;
    }
    unsafe { &mut *state_ptr }
}

pub const MAP_W: usize = 256;
pub const MAP_H: usize = 256;

pub static TILEMAP: [[u8; MAP_W]; MAP_H] = generate_simple();
// pub static TILEMAP: [[u8; MAP_W]; MAP_H] = generate();

const fn generate() -> [[u8; MAP_W]; MAP_H] {
    let mut map = [[0u8; MAP_W]; MAP_H];
    let mut seed: u32 = 0x9E37_79B9; // change for a different layout

    let mut y = 0;
    while y < MAP_H {
        let mut x = 0;
        while x < MAP_W {
            // LCG step
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);

            let border = x == 0 || y == 0 || x == MAP_W - 1 || y == MAP_H - 1;
            let wall = (seed >> 22) < 64; // top byte < 64 → ~25% chance

            map[y][x] = (border || wall) as u8;
            x += 1;
        }
        y += 1;
    }
    map
}

const fn generate_simple() -> [[u8; MAP_W]; MAP_H] {
    let mut map = [[0u8; MAP_W]; MAP_H];

    let mut chunk_y = 0;
    while chunk_y < OUTPUT_DIM {
        let mut chunk_x = 0;
        while chunk_x < OUTPUT_DIM {
            map[chunk_y * CHUNK_DIM + chunk_y][chunk_x * CHUNK_DIM + chunk_x] = 1;
            map[chunk_y * CHUNK_DIM][chunk_x * CHUNK_DIM] = 1;
            chunk_x += 1;
        }
        chunk_y += 1;
    }
    map
}

#[inline]
pub fn tile(x: usize, y: usize) -> u8 {
    TILEMAP[y][x]
}
