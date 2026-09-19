use anyhow::Result;
use std::{
    io::{Read, Write},
    ops::{Index, IndexMut},
};

pub const NUM_BUTTONS: usize = 12;
pub const NUM_POINTER_BUTTONS: usize = 2;

// Authoritative ordering for arrays of input codes
#[derive(Clone, Copy)]
#[repr(usize)]
pub enum GameButtonId {
    Up = 0,
    Down,
    Left,
    Right,
    LeftShoulder,
    RightShoulder,

    // Actions
    ActionUp,
    ActionDown,
    ActionLeft,
    ActionRight,

    // Menu
    Back,
    Start,
}

impl Index<GameButtonId> for [GameButtonState; NUM_BUTTONS] {
    type Output = GameButtonState;
    fn index(&self, b_idx: GameButtonId) -> &Self::Output {
        &self[b_idx as usize]
    }
}
impl IndexMut<GameButtonId> for [GameButtonState; NUM_BUTTONS] {
    fn index_mut(&mut self, b_idx: GameButtonId) -> &mut Self::Output {
        &mut self[b_idx as usize]
    }
}

impl From<GameButtonId> for usize {
    fn from(b_idx: GameButtonId) -> Self {
        b_idx as usize
    }
}
impl From<GameButtonId> for u32 {
    fn from(b_idx: GameButtonId) -> Self {
        b_idx as u32
    }
}

#[derive(Copy, Default, Clone, Debug)]
#[repr(C)]
pub struct GameButtonState {
    pub half_transition_count: usize,
    pub ended_down: bool,
}

#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct GameInput {
    pub buttons: [GameButtonState; NUM_BUTTONS],
    pub pointer: Pointer,
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct PointerButtonState {
    serial: u32,
    button: u32,
    pub ended_down: bool,
    pub half_transition_count: usize,
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct Pointer {
    pub x: f64,
    pub y: f64,
    pub buttons: [PointerButtonState; NUM_POINTER_BUTTONS],
}

impl GameInput {
    pub fn clear_half_transition_count(&mut self) {
        for key in &mut self.buttons {
            key.half_transition_count = 0;
        }

        for button in &mut self.pointer.buttons {
            button.half_transition_count = 0;
        }
    }
    pub fn as_bytes_unsafe(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const GameInput as *const u8,
                std::mem::size_of::<GameInput>(),
            )
        }
    }

    pub fn from_bytes_unsafe(bytes: &[u8]) -> Self {
        assert!(bytes.len() >= size_of::<Self>());
        unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const Self) }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GraphicsBufferRaw {
    // Actually just values for now, convert to u32 pixels soon
    pub pixels: *mut u8, // length = pitch_bytes * height_pixels
    pub width_pixels: usize,
    pub height_pixels: usize,
    pub pitch_bytes: usize,
    pub bytes_per_pixel: usize,
    pub pitch_pixels: usize,
}

pub struct GraphicsBuffer<'a> {
    pub pixels: &'a mut [u8],
    pub width_pixels: usize,
    pub height_pixels: usize,
    pub pitch_bytes: usize,
    pub pitch_pixels: usize,
    pub bytes_per_pixel: usize,
}

impl<'a> GraphicsBuffer<'a> {
    /// Safety: raw.pixels is valid for at least pitch_bytes * height_pixels.  Not aliased for 'a
    pub unsafe fn from_raw(raw: GraphicsBufferRaw) -> GraphicsBuffer<'a> {
        let len = raw.pitch_bytes * raw.height_pixels;
        let pixels = unsafe { std::slice::from_raw_parts_mut(raw.pixels, len) };

        Self {
            pixels,
            width_pixels: raw.width_pixels,
            height_pixels: raw.height_pixels,
            pitch_bytes: raw.pitch_bytes,
            bytes_per_pixel: raw.bytes_per_pixel,
            pitch_pixels: raw.pitch_pixels,
        }
    }

    pub fn to_raw(self) -> GraphicsBufferRaw {
        GraphicsBufferRaw {
            pixels: self.pixels.as_mut_ptr(),
            width_pixels: self.width_pixels,
            height_pixels: self.height_pixels,
            pitch_bytes: self.pitch_bytes,
            bytes_per_pixel: self.bytes_per_pixel,
            pitch_pixels: self.pitch_pixels,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AudioBufferRaw {
    pub samples_buf: *mut i16, // length = frame_count * 2
    pub sample_rate: u32,
    pub frame_count: usize,
}

pub struct AudioBuffer<'a> {
    pub samples_buf: &'a mut [i16],
    pub sample_rate: u32,
}

impl<'a> AudioBuffer<'a> {
    // Safety: raw.samples_buf is valid for frames_count*2, not aliased for 'a
    pub unsafe fn from_raw(raw: AudioBufferRaw) -> AudioBuffer<'a> {
        let len = raw.frame_count * 2;
        let samples_buf = unsafe { std::slice::from_raw_parts_mut(raw.samples_buf, len) };

        Self {
            samples_buf,
            sample_rate: raw.sample_rate,
        }
    }

    pub fn to_raw(self) -> AudioBufferRaw {
        AudioBufferRaw {
            samples_buf: self.samples_buf.as_mut_ptr(),
            sample_rate: self.sample_rate,
            frame_count: self.samples_buf.len() / 2,
        }
    }
}

#[repr(C)]
pub struct GameMemory {
    pub is_initialized: bool,
    pub permanent: *mut u8, // length = permanent_size
    pub permanent_size: usize,
    pub transient: *mut u8, // length = transient_size
    pub transient_size: usize,
}

impl GameMemory {
    pub fn write_to(&self, w: &mut impl Write) -> Result<()> {
        let bytes = unsafe {
            std::slice::from_raw_parts(self.permanent, self.permanent_size + self.transient_size)
        };
        w.write_all(bytes)?;
        Ok(())
    }

    pub fn read_from(&mut self, r: &mut impl Read) -> Result<()> {
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                self.permanent,
                self.permanent_size + self.transient_size,
            )
        };
        r.read_exact(bytes)?;
        Ok(())
    }
}

#[repr(C)]
pub struct PlatformApi;

pub type GameUpdateAndRenderFn =
    unsafe extern "C" fn(*mut GameMemory, *const GameInput, *mut GraphicsBufferRaw) -> bool;
pub type GameAudioRenderFn = unsafe extern "C" fn(*mut GameMemory, *mut AudioBufferRaw) -> bool;
