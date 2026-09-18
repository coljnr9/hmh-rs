pub const NUM_BUTTONS: usize = 12;

// Authoritative ordering for arrays of input codes
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

#[derive(Copy, Default, Clone, Debug)]
#[repr(C)]
pub struct GameButtonState {
    pub half_transition_count: usize,
    pub ended_down: bool,
}

#[derive(Default)]
#[repr(C)]
pub struct GameInput {
    pub buttons: [GameButtonState; NUM_BUTTONS],
}

impl GameInput {
    pub fn clear_half_transition_count(&mut self) {
        for key in &mut self.buttons {
            key.half_transition_count = 0;
        }
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
}

pub struct GraphicsBuffer<'a> {
    pub pixels: &'a mut [u8],
    pub width_pixels: usize,
    pub height_pixels: usize,
    pub pitch_bytes: usize,
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
        }
    }

    pub fn to_raw(self) -> GraphicsBufferRaw {
        GraphicsBufferRaw {
            pixels: self.pixels.as_mut_ptr(),
            width_pixels: self.width_pixels,
            height_pixels: self.height_pixels,
            pitch_bytes: self.pitch_bytes,
            bytes_per_pixel: self.bytes_per_pixel,
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

#[repr(C)]
pub struct PlatformApi;

pub type GameUpdateAndRenderFn =
    unsafe extern "C" fn(*mut GameMemory, *const GameInput, *mut GraphicsBufferRaw) -> bool;
pub type GameAudioRenderFn = unsafe extern "C" fn(*mut GameMemory, *mut AudioBufferRaw) -> bool;
