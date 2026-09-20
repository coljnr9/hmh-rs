use anyhow::Result;
use std::f64::consts;

use shared::{
    AudioBuffer, AudioBufferRaw, GameButtonId, GameInput, GameMemory, GraphicsBuffer,
    GraphicsBufferRaw, PlatformApi,
};

const TILE_MAP_COUNT_X: usize = 13;
const TILE_MAP_COUNT_Y: usize = 9;

const TILES_00: [[u8; TILE_MAP_COUNT_X]; TILE_MAP_COUNT_Y] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0],
    [1, 1, 0, 0, 0, 1, 1, 0, 1, 1, 1, 1, 1],
    [0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
];

const TILES_01: [[u8; TILE_MAP_COUNT_X]; TILE_MAP_COUNT_Y] = [
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

const TILES_10: [[u8; TILE_MAP_COUNT_X]; TILE_MAP_COUNT_Y] = [
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

const TILES_11: [[u8; TILE_MAP_COUNT_X]; TILE_MAP_COUNT_Y] = [
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

#[repr(C)]
#[derive(Default, Debug)]
pub struct GameState {
    x_offset: i64,
    y_offset: i64,
    // Temporarily used to track sine-wave angle between audio calls
    theta: f64,

    player_x: f64,
    player_y: f64,
    tile_map_idx: usize,
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
    let mut player_x_delta = 0.0;
    let mut player_y_delta = 0.0;

    if game_input.buttons[GameButtonId::Up].ended_down {
        player_y_delta = -100.0;
    }
    if game_input.buttons[GameButtonId::Down].ended_down {
        player_y_delta = 100.0;
    }
    if game_input.buttons[GameButtonId::Left].ended_down {
        player_x_delta = -100.0;
    }
    if game_input.buttons[GameButtonId::Right].ended_down {
        player_x_delta = 100.0;
    }

    // TODO(coljnr9): bounds checking

    let mut tile_map_00 = TileMap {
        map_count_x: 13,
        map_count_y: 9,
        upper_left_x: 0.0,
        upper_left_y: 0.0,
        width: 100.0,
        height: 100.0,
        tiles: TILES_00.as_flattened(),
    };

    let mut tile_map_01 = TileMap {
        map_count_x: 13,
        map_count_y: 9,
        upper_left_x: 0.0,
        upper_left_y: 0.0,
        width: 100.0,
        height: 100.0,
        tiles: TILES_01.as_flattened(),
    };
    let mut tile_map_10 = TileMap {
        map_count_x: 13,
        map_count_y: 9,
        upper_left_x: 0.0,
        upper_left_y: 0.0,
        width: 100.0,
        height: 100.0,
        tiles: TILES_10.as_flattened(),
    };
    let mut tile_map_11 = TileMap {
        map_count_x: 13,
        map_count_y: 9,
        upper_left_x: 0.0,
        upper_left_y: 0.0,
        width: 100.0,
        height: 100.0,
        tiles: TILES_11.as_flattened(),
    };

    let tile_maps = [tile_map_00, tile_map_01, tile_map_10, tile_map_11];
    let mut tile_map = &tile_maps[0];
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

    let tile_width = 100.0;
    let tile_height = 100.0;

    let player_width = 0.5 * tile_width;
    let player_height = 0.75 * tile_height;

    let new_player_x = game_state.player_x + game_input.dt * player_x_delta;
    let new_player_y = game_state.player_y + game_input.dt * player_y_delta;

    for y in 0..tile_map.map_count_y {
        for x in 0..tile_map.map_count_x {
            let min_x = x as f64 * tile_width;
            let min_y = y as f64 * tile_height;
            let max_x = min_x + tile_width;
            let max_y = min_y + tile_height;

            let grey = if tile_map.value_at(x as f64, y as f64) > 0 {
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
    let min_x = new_player_x - player_width / 2.0;
    let min_y = new_player_y - player_height;

    let max_x = new_player_x + player_width / 2.0;
    let max_y = new_player_y;

    match next_tile_map_dir(new_player_x, new_player_y, &tile_map) {
        NextTileMapDirection::North => game_state.tile_map_idx -= 2,
        NextTileMapDirection::South => game_state.tile_map_idx += 2,
        NextTileMapDirection::East => game_state.tile_map_idx += 1,
        NextTileMapDirection::West => game_state.tile_map_idx -= 1,
        NextTileMapDirection::Stay => {}
    };

    tile_map = &tile_maps[game_state.tile_map_idx];
    // if is_tile_map_point_empty(min_x, max_y, &tile_map)
    //     && is_tile_map_point_empty(max_x, max_y, &tile_map)
    // {
    game_state.player_x = new_player_x;
    game_state.player_y = new_player_y;
    // }

    draw_rectangle(graphics_buffer, min_x, min_y, max_x, max_y, 0.0, 0.0, 1.0);
    Ok(())
}

struct TileMap<'a> {
    map_count_x: usize,
    map_count_y: usize,
    upper_left_x: f64,
    upper_left_y: f64,
    width: f64,
    height: f64,

    tiles: &'a [u8],
}

impl<'a> TileMap<'a> {
    fn value_at(&self, x: f64, y: f64) -> u8 {
        let x = x.floor() as usize;
        let y = y.floor() as usize;
        let row = self.map_count_x * y;
        let col = x;
        self.tiles[row + col]
    }
}
fn is_tile_map_point_empty(x: f64, y: f64, tile_map: &TileMap) -> bool {
    let mut is_empty = false;
    let tile_x = (x / tile_map.width).floor() as usize;
    let tile_y = (y / tile_map.height).floor() as usize;

    if (tile_x < tile_map.map_count_x) && (tile_y < tile_map.map_count_y) {
        let tile_map_value = tile_map.value_at(tile_x as f64, tile_y as f64);
        if tile_map_value == 1 {
            is_empty = true;
        }
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
        let value = 2000.0
            * (2.0 * consts::PI * 440.0 * (game_state.theta + d_theta as f64)
                / audio_buffer.sample_rate as f64)
                .sin();
        let value = 0.0;
        frame.copy_from_slice(&[value as i16, value as i16]);
    }
    game_state.theta += (audio_buffer.samples_buf.len() / 2) as f64;
}

#[derive(Debug)]
enum NextTileMapDirection {
    North,
    South,
    East,
    West,
    Stay,
}
fn next_tile_map_dir(test_x: f64, test_y: f64, tile_map: &TileMap) -> NextTileMapDirection {
    let tile_idx_x = (test_x / tile_map.width).floor() as i64;
    let tile_idx_y = (test_y / tile_map.height).floor() as i64;
    println!("tile_idx_x: {}, tile_idx_y: {}", tile_idx_x, tile_idx_y);

    let next_tm_south = (tile_map.map_count_y) as i64;
    let next_tm_east = (tile_map.map_count_y) as i64;

    if tile_idx_x == -1 {
        return NextTileMapDirection::West;
    }
    if tile_idx_x == next_tm_east {
        return NextTileMapDirection::East;
    }
    if tile_idx_y == -1 {
        return NextTileMapDirection::North;
    }
    if tile_idx_y == next_tm_south {
        return NextTileMapDirection::South;
    }

    NextTileMapDirection::Stay
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

    let game_state = unsafe { &mut *(memory.permanent as *mut GameState) };
    if !memory.is_initialized {
        game_state.x_offset = 0;
        game_state.y_offset = 0;
        game_state.theta = 0.0;

        game_state.player_x = 50.0;
        game_state.player_y = 100.0;
        game_state.tile_map_idx = 0;
        memory.is_initialized = true;
    }
    game_state
}
