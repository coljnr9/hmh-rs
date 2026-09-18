use anyhow::Result;
use std::f32::consts;

use shared::{
    AudioBuffer, AudioBufferRaw, GameButtonId, GameInput, GameMemory, GraphicsBuffer,
    GraphicsBufferRaw, PlatformApi,
};

// TODO(coljnr9): We probably want this in a place that doesn't get zeroed on hot-reload

#[repr(C)]
#[derive(Default)]
pub struct GameState {
    x_offset: i64,
    y_offset: i64,
    // Temporarily used to track sine-wave angle between audio calls
    theta: f32,

    player_x: i64,
    player_y: i64,
}

pub fn game_update_and_render_internal(
    game_state: &mut GameState,
    graphics_buffer: &mut GraphicsBuffer,
    game_input: &GameInput,
    _platform_api: &PlatformApi,
) -> Result<()> {
    if game_input.buttons[GameButtonId::Left as usize].ended_down {
        game_state.x_offset += 5;
        game_state.player_x += 2;
    }
    if game_input.buttons[GameButtonId::Right as usize].ended_down {
        game_state.x_offset -= 5
    }
    if game_input.buttons[GameButtonId::Up as usize].ended_down {
        game_state.y_offset += 5;
        game_state.player_x += 2;
    }
    if game_input.buttons[GameButtonId::Down as usize].ended_down {
        game_state.y_offset -= 5;
    }

    let rows = bytemuck::cast_slice_mut::<u8, u32>(graphics_buffer.pixels)
        .chunks_mut(graphics_buffer.pitch_bytes / 4);

    for (y, row) in rows.take(graphics_buffer.height_pixels).enumerate() {
        for (x, px) in row[..graphics_buffer.width_pixels].iter_mut().enumerate() {
            *px = u32::from_be_bytes([
                255,
                0,
                (y as i64 + game_state.y_offset) as u8,
                (x as i64 + game_state.x_offset) as u8,
            ]);
        }
    }

    Ok(())
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
            * (2.0 * consts::PI * 220.0 * (game_state.theta + d_theta as f32)
                / audio_buffer.sample_rate as f32)
                .sin();
        frame.copy_from_slice(&[value as i16, value as i16]);
    }
    game_state.theta += (audio_buffer.samples_buf.len() / 2) as f32;
}

/// # Safety: See game_state_from_memory
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

/// # Safety: See game_state_from_memory
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
        memory.is_initialized = true;
    }
    game_state
}
