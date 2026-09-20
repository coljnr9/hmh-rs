use anyhow::Result;
use std::f64::consts;

use shared::{
    AudioBuffer, AudioBufferRaw, GameButtonId, GameInput, GameMemory, GraphicsBuffer,
    GraphicsBufferRaw, PlatformApi,
};

const TILE_MAP_COUNT_X: usize = 13;
const TILE_MAP_COUNT_Y: usize = 9;

const WORLD_COUNT_X: usize = 2;
const WORLD_COUNT_Y: usize = 2;

const TILE_WIDTH: f64 = 100.0;
const TILE_HEIGHT: f64 = 100.0;

type Tiles = [[u8; TILE_MAP_COUNT_X]; TILE_MAP_COUNT_Y];

const TILES_00: Tiles = [
    [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 1, 1, 0, 1, 1, 1, 1, 1],
    [0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
];

const TILES_01: Tiles = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
];

const TILES_10: Tiles = [
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 1, 0, 1, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 1, 0, 1, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

const TILES_11: Tiles = [
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

//      [00] [01]
//      [10] [11]
//

const TILE_MAPS: [TileMap<'static>; WORLD_COUNT_X * WORLD_COUNT_Y] = [
    TileMap {
        tiles: TILES_00.as_flattened(),
    },
    TileMap {
        tiles: TILES_01.as_flattened(),
    },
    TileMap {
        tiles: TILES_10.as_flattened(),
    },
    TileMap {
        tiles: TILES_11.as_flattened(),
    },
];

const PLAYER_WIDTH: f64 = 0.5 * TILE_WIDTH;
const PLAYER_HEIGHT: f64 = 0.75 * TILE_HEIGHT;
const SPEED_FACTOR: f64 = 500.0;

#[repr(C)]
#[derive(Default)]
pub struct GameState {
    x_offset: i64,
    y_offset: i64,
    // Temporarily used to track sine-wave angle between audio calls
    theta: f64,

    player_position: PlayerPosition,
}

#[derive(Debug, Default, Copy, Clone)]
struct PlayerPosition {
    tile_map_idx: usize,
    x: f64,
    y: f64,
}

#[derive(Default)]
struct World<'a> {
    num_maps_x: usize,
    num_maps_y: usize,

    tile_width: f64,
    tile_height: f64,

    num_map_tiles_x: usize,
    num_map_tiles_y: usize,

    tile_maps: [TileMap<'a>; WORLD_COUNT_X * WORLD_COUNT_Y],

    // TODO(coljnr9): The currently visible tilemap idx. Not sure if this _really_ lives in the
    // world
    current_tilemap_idx: usize,
}

fn draw_rectangle(
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

pub fn game_update_and_render_internal(
    game_state: &mut GameState,
    graphics_buffer: &mut GraphicsBuffer,
    game_input: &GameInput,
    _platform_api: &PlatformApi,
) -> Result<()> {
    let world = World {
        num_maps_x: WORLD_COUNT_X,
        num_maps_y: WORLD_COUNT_Y,
        tile_width: TILE_WIDTH,
        tile_height: TILE_HEIGHT,
        num_map_tiles_x: TILE_MAP_COUNT_X,
        num_map_tiles_y: TILE_MAP_COUNT_Y,
        tile_maps: TILE_MAPS,
        current_tilemap_idx: game_state.player_position.tile_map_idx,
    };
    draw_rectangle(
        graphics_buffer,
        0.0,
        0.0,
        graphics_buffer.width_pixels as f64,
        graphics_buffer.height_pixels as f64,
        1.0,
        0.0,
        1.0,
    );

    let map = &world.tile_maps[world.current_tilemap_idx];

    for y in 0..world.num_map_tiles_y {
        for x in 0..world.num_map_tiles_x {
            let min_x = x as f64 * TILE_WIDTH;
            let min_y = y as f64 * TILE_HEIGHT;
            let max_x = min_x + TILE_WIDTH;
            let max_y = min_y + TILE_HEIGHT;

            let grey = if map.value_at(x as f64, y as f64) > 0 {
                1.0
            } else {
                0.5
            };
            draw_rectangle(
                graphics_buffer,
                min_x,
                min_y,
                max_x,
                max_y,
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

    let new_player_x = game_state.player_position.x + game_input.dt * player_x_delta;
    let new_player_y = game_state.player_position.y + game_input.dt * player_y_delta;

    let min_x = new_player_x - PLAYER_WIDTH / 2.0;
    let min_y = new_player_y - PLAYER_HEIGHT;

    let max_x = new_player_x + PLAYER_WIDTH / 2.0;
    let max_y = new_player_y;

    let new_position = get_world_position(new_player_x, new_player_y, &world);
    let pos0 = get_world_position(min_x, max_y, &world);
    let pos1 = get_world_position(max_x, max_y, &world);

    if is_position_empty(pos0, &world) && is_position_empty(pos1, &world) {
        // world.current_tilemap_idx = new_position.tile_map_idx;
        game_state.player_position = new_position;
    }

    draw_rectangle(graphics_buffer, min_x, min_y, max_x, max_y, 1.0, 0.0, 1.0);
    Ok(())
}

fn get_world_position(test_x: f64, test_y: f64, world: &World) -> PlayerPosition {
    let mut new_position = PlayerPosition {
        tile_map_idx: world.current_tilemap_idx,
        x: test_x,
        y: test_y,
    };
    if test_x < 0.0 {
        new_position.tile_map_idx -= 1;
        new_position.x += world.tile_width * world.num_map_tiles_x as f64;
    } else if test_x >= world.tile_width * world.num_map_tiles_x as f64 {
        new_position.tile_map_idx += 1;
        new_position.x -= world.tile_width * world.num_map_tiles_x as f64;
    } else if test_y < 0.0 {
        new_position.tile_map_idx -= world.num_maps_x;
        new_position.y += world.tile_height * world.num_map_tiles_y as f64;
    } else if test_y >= world.tile_height * world.num_map_tiles_y as f64 {
        new_position.tile_map_idx += world.num_maps_x;
        new_position.y -= world.tile_height * world.num_map_tiles_y as f64;
    };

    new_position
}

#[derive(Default)]
struct TileMap<'a> {
    tiles: &'a [u8],
}

impl<'a> TileMap<'a> {
    fn value_at(&self, x: f64, y: f64) -> u8 {
        let x = x.floor() as usize;
        let y = y.floor() as usize;
        let row = TILE_MAP_COUNT_X * y;
        let col = x;
        self.tiles[row + col]
    }
}
fn is_position_empty(position: PlayerPosition, world: &World) -> bool {
    let mut is_empty = false;
    let PlayerPosition { x, y, tile_map_idx } = position;

    let tile_x = (x / world.tile_width).floor() as usize;
    let tile_y = (y / world.tile_height).floor() as usize;
    let tile_map_value = world.tile_maps[tile_map_idx].value_at(tile_x as f64, tile_y as f64);
    if tile_map_value == 1 {
        is_empty = true;
    }
    is_empty
}
pub fn game_audio_render_internal(game_state: &mut GameState, audio_buffer: &mut AudioBuffer) {
    for (d_theta, frame) in audio_buffer
        .samples_buf
        .as_chunks_mut::<2>()
        .0
        .iter_mut()
        .enumerate()
    {
        let _value = 2000.0
            * (2.0 * consts::PI * 440.0 * (game_state.theta + d_theta as f64)
                / audio_buffer.sample_rate as f64)
                .sin();
        let value = 0.0;
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
        let mut game_state = GameState::default();
        game_state.x_offset = 0;
        game_state.y_offset = 0;
        game_state.theta = 0.0;

        game_state.player_position = PlayerPosition {
            tile_map_idx: 0,
            x: 250.0,
            y: 250.0,
        };

        unsafe { state_ptr.write(game_state) };
        memory.is_initialized = true;
    }
    unsafe { &mut *state_ptr }
}
