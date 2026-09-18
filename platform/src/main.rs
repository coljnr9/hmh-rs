use std::{
    alloc::{Layout, alloc_zeroed},
    cmp::min,
    env,
    fs::{self, File},
    os::fd::AsFd,
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

use alsa::{
    Direction, PCM,
    pcm::{self, HwParams},
};
use anyhow::{Context, Result, bail};
use libloading::Library;
use memmap2::MmapMut;
use rustix::{
    fs::{MemfdFlags, Mode, OFlags, memfd_create},
    io::Errno,
};
use shared::{
    AudioBuffer, AudioBufferRaw, GameAudioRenderFn, GameInput, GameMemory, GameUpdateAndRenderFn,
    GraphicsBuffer, GraphicsBufferRaw, PlatformApi,
};
use tracing::{debug, debug_span, error, info};
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    globals::GlobalListContents,
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_callback::{self, WlCallback},
        wl_compositor::WlCompositor,
        wl_display::WlDisplay,
        wl_keyboard::{
            self,
            KeyState::{self},
            WlKeyboard,
        },
        wl_output::{self, WlOutput},
        wl_pointer::{self, WlPointer},
        wl_registry::{self},
        wl_seat::WlSeat,
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};

const KILOBYTE: usize = 1024;
const MEGABYTE: usize = 1024 * KILOBYTE;
const GIGABYTE: usize = 1024 * MEGABYTE;
const DEFAULT_WINDOW_WIDTH: usize = 1920;
const DEFAULT_WINDOW_HEIGHT: usize = 1080;

const POOL_WIDTH_PIXELS: usize = 3840;
const POOL_HEIGHT_PIXELS: usize = 2160 * 2; // Top and bottom split
const BYTES_PER_PIXEL: usize = 4;
const POOL_STRIDE_BYTES: usize = POOL_WIDTH_PIXELS * BYTES_PER_PIXEL;
const FORMAT: wl_shm::Format = wl_shm::Format::Xrgb8888;

const W_KEY_CODE: u32 = 17;
const A_KEY_CODE: u32 = 30;
const S_KEY_CODE: u32 = 31;
const D_KEY_CODE: u32 = 32;
const Q_KEY_CODE: u32 = 16;
const E_KEY_CODE: u32 = 18;
const I_KEY_CODE: u32 = 23;
const J_KEY_CODE: u32 = 36;
const K_KEY_CODE: u32 = 37;
const L_KEY_CODE: u32 = 38;

const SPACE_KEY_CODE: u32 = 57;
const ESC_KEY_CODE: u32 = 1;

const HOT_RELOAD_KEYCODE: u32 = 27 - 8;
const GAME_LIB_PATH: &'static str = "target/release/libgame.so";

// Positionally map from GAmeButtonId to the keycode
const KEYBOARD_MAPPING: [u32; shared::NUM_BUTTONS] = [
    W_KEY_CODE,
    S_KEY_CODE,
    A_KEY_CODE,
    D_KEY_CODE,
    Q_KEY_CODE,
    E_KEY_CODE,
    I_KEY_CODE,
    K_KEY_CODE,
    J_KEY_CODE,
    L_KEY_CODE,
    SPACE_KEY_CODE,
    ESC_KEY_CODE,
];
const ALSA_CHANNELS: u32 = 2;
const ALSA_SAMPLE_RATE: u32 = 48_000;
const LATENCY_TARGET_FRAMES: u32 = ALSA_SAMPLE_RATE / 10;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum RegionState {
    Busy,
    Free,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(usize)]
enum RegionIndex {
    A = 0,
    B = 1,
}

struct BufferParams {
    offset: i32,
    width: i32,
    height: i32,
    stride: i32,
    format: wl_shm::Format,
    index: RegionIndex,
}

struct PoolRegion {
    offset: usize,
    state: RegionState,
    index: RegionIndex,
    buffer: WlBuffer,
    current_buffer_width: usize,
    current_buffer_height: usize,
}

impl PoolRegion {
    fn buffer_params(&self, x: usize, y: usize, width: usize, height: usize) -> BufferParams {
        let offset = self.offset + y * POOL_STRIDE_BYTES + x * BYTES_PER_PIXEL;
        BufferParams {
            offset: offset as i32,
            width: width as i32,
            height: height as i32,
            stride: POOL_STRIDE_BYTES as i32,
            format: FORMAT,
            index: self.index,
        }
    }

    fn ensure_size(
        &mut self,
        width: usize,
        height: usize,
        wl_pool: &WlShmPool,
        qh: &QueueHandle<AppData>,
    ) {
        let _enter = debug_span!("ensure_size", height = height, width = width);
        let BufferParams {
            offset,
            width,
            height,
            stride,
            format,
            index,
        } = self.buffer_params(0, 0, width, height);
        self.buffer.destroy();
        self.buffer = wl_pool.create_buffer(offset, width, height, stride, format, qh, index);
        self.current_buffer_width = width as usize;
        self.current_buffer_height = height as usize;
    }
}

fn update_keyboard_input(
    keyboard_controller: &mut GameInput,
    incoming_key_code: u32,
    is_down: bool,
) {
    if let Some(button_idx) = KEYBOARD_MAPPING
        .iter()
        .position(|&k| k == incoming_key_code)
    {
        let button = &mut keyboard_controller.buttons[button_idx];
        if button.ended_down != is_down {
            button.half_transition_count += 1;
        }
        button.ended_down = is_down;
    };
}

fn debug_sync_display(_buffer: &mut [u8]) {
    // Draw to visualize the audio buffer stuff
}

struct Proxies {
    connection: Connection,
    wl_compositor: WlCompositor,
    wl_display: WlDisplay,
    wl_surface: WlSurface,
    wl_shm: WlShm,
    wl_shm_pool: WlShmPool,
    xdg_wm_base: XdgWmBase,
    xdg_surface: XdgSurface,
    xdg_toplevel: XdgToplevel,
}

struct Memory {
    file: File,
    backbuffer: MmapMut,
    regions: [PoolRegion; 2],
}

struct Inbox {
    // New size request
    configure_width: usize,
    configure_height: usize,
    configure_serial: Option<u32>,
    frame_done: bool,
    close: bool,
    reload_requested: bool,
}

struct AppData {
    proxies: Proxies,
    memory: Memory,
    // For things that can't/shouldn't be processed in the Dispatch handler, but rather in the game
    // loop
    inbox: Inbox,
    window_width_pixels: usize,
    window_height_pixels: usize,
    monitor_refresh_hz: i32,
    controller: GameInput,
}

impl AppData {}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for AppData {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        info!("Registry/Globals dispatch");
    }
}
wayland_client::delegate_noop!(AppData: ignore WlCompositor);
wayland_client::delegate_noop!(AppData: ignore WlShm);
wayland_client::delegate_noop!(AppData: ignore WlSurface);
wayland_client::delegate_noop!(AppData: ignore WlShmPool);

impl Dispatch<WlOutput, ()> for AppData {
    fn event(
        state: &mut Self,
        _proxy: &WlOutput,
        event: <WlOutput as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Mode { refresh, .. } = event {
            state.monitor_refresh_hz = refresh / 1000
        }
    }
}
impl Dispatch<XdgWmBase, ()> for AppData {
    fn event(
        state: &mut Self,
        _proxy: &XdgWmBase,
        event: <XdgWmBase as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            state.proxies.xdg_wm_base.pong(serial);
            debug!("pong sent");
        }
    }
}

impl Dispatch<XdgSurface, ()> for AppData {
    fn event(
        state: &mut Self,
        _proxy: &XdgSurface,
        event: <XdgSurface as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            debug!(serial = serial, "Got xdg_surface.configure event");
            state.inbox.configure_serial = Some(serial);
        }
    }
}

impl Dispatch<WlBuffer, RegionIndex> for AppData {
    fn event(
        state: &mut Self,
        _proxy: &WlBuffer,
        event: <WlBuffer as wayland_client::Proxy>::Event,
        buffer_index: &RegionIndex,
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            state.memory.regions[*buffer_index as usize].state = RegionState::Free;
        }
    }
}

impl Dispatch<XdgToplevel, ()> for AppData {
    fn event(
        state: &mut Self,
        _proxy: &XdgToplevel,
        event: <XdgToplevel as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states: _,
            } => {
                if width != 0 {
                    state.window_width_pixels = POOL_WIDTH_PIXELS.min(width as usize);
                }

                if height != 0 {
                    state.window_height_pixels = (POOL_HEIGHT_PIXELS / 2).min(height as usize);
                }
            }
            xdg_toplevel::Event::Close => state.inbox.close = true,
            _ => info!(event = ?event, "Not handling event"),
        }
    }
}

impl Dispatch<WlCallback, ()> for AppData {
    fn event(
        state: &mut Self,
        _proxy: &wl_callback::WlCallback,
        _event: <wl_callback::WlCallback as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        state.inbox.frame_done = true;
    }
}

impl Dispatch<WlSeat, ()> for AppData {
    fn event(
        _state: &mut Self,
        _proxy: &WlSeat,
        event: <WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        debug!(event=?event, "Got wl_seat event");
    }
}

impl Dispatch<WlKeyboard, ()> for AppData {
    fn event(
        state: &mut Self,
        _proxy: &WlKeyboard,
        event: <WlKeyboard as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap {
                format: _,
                fd: _,
                size: _,
            } => {}
            wl_keyboard::Event::Key {
                key: HOT_RELOAD_KEYCODE,
                state: WEnum::Value(KeyState::Pressed),
                ..
            } => {
                state.inbox.reload_requested = true;
                //  I think this means "R" is not available in the game at all. PRobaly change this
                //  to a modifier or something
            }
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(key_state),
                ..
            } => {
                let is_down = key_state == KeyState::Pressed;
                update_keyboard_input(&mut state.controller, key, is_down);
            }
            wl_keyboard::Event::Enter {
                serial: _,
                surface: _,
                keys,
            } => {
                info!(keys = ?keys, "got wl_keyboard enter");
                let keys = keys
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|k| bytemuck::pod_read_unaligned::<u32>(k))
                    .collect::<Vec<u32>>();

                // Invariant: keyboard mapping is correctly ordered (matching GameButtonId
                // ordering/values)
                for (i, key_code) in KEYBOARD_MAPPING.iter().enumerate() {
                    state.controller.buttons[i].ended_down = keys.contains(key_code);
                }
            }
            _ => {}
        }

        // Hacky for now
    }
}

impl Dispatch<WlPointer, ()> for AppData {
    fn event(
        _state: &mut Self,
        _proxy: &WlPointer,
        event: <WlPointer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x: _,
                surface_y: _,
            } => {
                debug!(serial = serial, surface = ?surface, "Got enter event");
            }
            wl_pointer::Event::Leave { serial, surface } => {
                debug!(serial = serial, surface = ?surface, "Got leave event")
            }
            _ => {}
        }
    }
}

pub(crate) fn platform_read_entire_file(file_name: &str) -> Result<Box<[u8]>> {
    let fd = rustix::fs::open(file_name, OFlags::RDONLY, Mode::empty())?;
    let stat = rustix::fs::fstat(&fd)?;
    let size = usize::try_from(stat.st_size)?;

    let buf = Box::new_zeroed_slice(size as usize);
    let mut buf: Box<[u8]> = unsafe { buf.assume_init() };
    let mut bytes_read = 0;
    while bytes_read < size {
        let b = match rustix::io::read(&fd, &mut buf[bytes_read..]) {
            Ok(0) => bail!("Unexpected end of file"),
            Ok(b) => b,
            Err(Errno::INTR) => continue,
            Err(e) => bail!(e),
        };
        bytes_read += b;
    }

    Ok(buf)
}
pub(crate) fn platform_write_entire_file(file_name: &str, data: &[u8]) -> Result<()> {
    let fd = rustix::fs::open(
        file_name,
        OFlags::WRONLY | OFlags::CREATE,
        Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
    )?;
    rustix::io::write(fd, data)?;
    Ok(())
}

fn main() -> Result<()> {
    let logging_env_filter = EnvFilter::builder()
        .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(logging_env_filter)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    info!("Starting up");

    let wayland_connection = Connection::connect_to_env().context("connecting to compositor")?;
    let (global_list, mut event_queue) =
        wayland_client::globals::registry_queue_init::<AppData>(&wayland_connection)
            .context("getting globals list and queue")?;
    let qh = &event_queue.handle();

    let wl_compositor: WlCompositor = global_list.bind(&event_queue.handle(), 0..=1, ())?;
    let wl_shm: WlShm = global_list.bind(&event_queue.handle(), 0..=1, ())?;
    let xdg_wm_base: XdgWmBase = global_list.bind(&event_queue.handle(), 0..=1, ())?;

    let wl_display = wayland_connection.display();
    let wl_surface = wl_compositor.create_surface(qh, ());
    let xdg_surface = xdg_wm_base.get_xdg_surface(&wl_surface, qh, ());
    let xdg_toplevel = xdg_surface.get_toplevel(qh, ());

    let _wl_output: WlOutput = global_list.bind(&event_queue.handle(), 0..=1, ())?;
    let anon_fd = memfd_create("backbuff", MemfdFlags::empty())?;
    let anon_file = File::from(anon_fd);

    let size = POOL_WIDTH_PIXELS * POOL_HEIGHT_PIXELS * BYTES_PER_PIXEL;
    anon_file.set_len(size as u64)?;
    let backbuffer = unsafe { memmap2::MmapMut::map_mut(&anon_file)? };
    let wl_shm_pool = wl_shm.create_pool(anon_file.as_fd(), size as i32, qh, ());

    let pool_region_a = PoolRegion {
        offset: 0,
        index: RegionIndex::A,
        state: RegionState::Free,
        current_buffer_width: DEFAULT_WINDOW_WIDTH,
        current_buffer_height: DEFAULT_WINDOW_HEIGHT,
        buffer: wl_shm_pool.create_buffer(
            0,
            DEFAULT_WINDOW_WIDTH as i32,
            DEFAULT_WINDOW_HEIGHT as i32,
            POOL_STRIDE_BYTES as i32,
            FORMAT,
            qh,
            RegionIndex::A,
        ),
    };

    let offset = POOL_STRIDE_BYTES * POOL_HEIGHT_PIXELS / 2;
    let pool_region_b = PoolRegion {
        offset: POOL_STRIDE_BYTES * POOL_HEIGHT_PIXELS / 2,
        index: RegionIndex::B,
        state: RegionState::Free,
        current_buffer_width: DEFAULT_WINDOW_WIDTH,
        current_buffer_height: DEFAULT_WINDOW_HEIGHT,
        buffer: wl_shm_pool.create_buffer(
            offset as i32,
            DEFAULT_WINDOW_WIDTH as i32,
            DEFAULT_WINDOW_HEIGHT as i32,
            POOL_STRIDE_BYTES as i32,
            FORMAT,
            qh,
            RegionIndex::B,
        ),
    };

    let wl_seat: WlSeat = global_list.bind(&event_queue.handle(), 0..=1, ())?;

    // TODO(coljnr9): Right now, assume that we have keyboard and pointer capabilities.  Later, add
    // checks
    let _keyboard = wl_seat.get_keyboard(qh, ());
    let _pointer = wl_seat.get_pointer(qh, ());

    let mut app = AppData {
        proxies: Proxies {
            connection: wayland_connection,
            wl_compositor,
            wl_display,
            wl_surface,
            wl_shm,
            wl_shm_pool,
            xdg_wm_base,
            xdg_surface,
            xdg_toplevel,
        },
        memory: Memory {
            file: anon_file,
            backbuffer,
            regions: [pool_region_a, pool_region_b],
        },
        inbox: Inbox {
            configure_width: 0,
            configure_height: 0,
            configure_serial: None,
            frame_done: true,
            close: false,
            reload_requested: false,
        },
        window_width_pixels: DEFAULT_WINDOW_WIDTH,
        window_height_pixels: DEFAULT_WINDOW_HEIGHT,
        controller: GameInput::default(),
        monitor_refresh_hz: 60,
    };

    app.proxies.wl_surface.commit();
    event_queue.roundtrip(&mut app)?;

    // Trying out sound loop
    unsafe {
        env::set_var("PIPEWIRE_LATENCY", "256/48000");
    }

    let pcm = PCM::new("default", Direction::Playback, false)?;
    let hwp = HwParams::any(&pcm)?;
    hwp.set_channels(ALSA_CHANNELS)?;
    hwp.set_rate(ALSA_SAMPLE_RATE, alsa::ValueOr::Nearest)?;
    hwp.set_format(pcm::Format::s16())?;
    hwp.set_access(pcm::Access::MMapInterleaved)?;
    pcm.hw_params(&hwp)?;
    let io = pcm.io_i16()?;

    let (alsa_capacity_frames, _alsa_period_frames) = pcm.get_params()?;

    // Platform <-> game interaction assumes a zeroed, aligned memory block.
    let permanent_ptr = unsafe {
        let layout = Layout::from_size_align(64 * MEGABYTE, 4096)?;
        let permanent_ptr = alloc_zeroed(layout);
        if permanent_ptr.is_null() {
            panic!("Allocation failed");
        }
        permanent_ptr
    };

    let mut transient = vec![0u8; 256 * MEGABYTE];
    let mut game_memory = GameMemory {
        is_initialized: false,
        permanent: permanent_ptr,
        permanent_size: 64 * MEGABYTE,
        transient: transient.as_mut_ptr(),
        transient_size: transient.len(),
    };
    let mut loop_start = Instant::now();
    let mut loop_end: Instant;
    let mut written_window = [0usize; 32];
    let mut queued_window = [0usize; 32];
    let mut idx = 0;
    let mut generation_id = 0;
    let mut game_code = load("target/release/libgame.so", generation_id)?;
    let mut last_mtime = fs::metadata(GAME_LIB_PATH)?.modified()?;
    let mut need_reload = false;
    loop {
        let _loop_enter = debug_span!("loop").entered();
        if app.inbox.close {
            break Ok(());
        }

        // TODO(coljnr9): Decide on behavior when the file is not available. Also, see if there's a
        // better completion signal for the compile being complete
        match fs::metadata(GAME_LIB_PATH) {
            Ok(meta) => match meta.modified() {
                Ok(mtime) => {
                    if mtime != last_mtime {
                        last_mtime = mtime;
                        need_reload = true;
                    }
                }

                Err(e) => error!("Weird filesystem doesn't support mtimes on files: {:?}", e),
            },
            Err(e) => error!(
                "Error accessing file metadata: {:?}\n{:?}",
                e, GAME_LIB_PATH
            ),
        };

        if need_reload
            && (SystemTime::now().duration_since(last_mtime)? > Duration::from_millis(10))
        {
            need_reload = false;
            generation_id += 1;
            game_code = load(GAME_LIB_PATH, generation_id).unwrap_or(game_code);
        }

        // Sound
        let available_frames = pcm.avail_update()?;
        let queued_frames = alsa_capacity_frames as i64 - available_frames;
        let desired_frames = min(
            LATENCY_TARGET_FRAMES as i64 - queued_frames,
            available_frames,
        )
        .max(0) as usize;
        written_window[idx] = desired_frames;
        queued_window[idx] = queued_frames as usize;
        idx += 1;
        if idx == 32 {
            idx = 0;
        }

        // We ask for desired frames, ALSA determines how much it can handle, then gives me a slice
        // len min(desired_frames, alsa_capacity_frames) * 2.
        let written_frames = io.mmap(desired_frames, |mem| {
            let _frame_count = mem.len() / 2;
            let audio_buffer = AudioBuffer {
                samples_buf: mem,
                sample_rate: ALSA_SAMPLE_RATE,
            };

            // Require that the game generates all requested samples, never fewer.
            let mut audio_buffer = audio_buffer.to_raw();
            unsafe {
                (game_code.audio_render)(&mut game_memory, &mut audio_buffer);
            }

            mem.len() / 2
        })? as usize;

        if pcm.state() != pcm::State::Running {
            debug!("Kicking off audio playback");
            pcm.start()?;
        }

        debug!(
            available_frames = available_frames,
            written_frames = written_frames,
            queued_frames = queued_frames,
            pcm_state = ?pcm.state(),
            "audio queue measurements"
        );
        let slot = if let Some(index) = app
            .memory
            .regions
            .iter()
            .position(|r| r.state == RegionState::Free)
        {
            &mut app.memory.regions[index]
        } else {
            event_queue.blocking_dispatch(&mut app)?;
            continue;
        };

        // Pacing by compositor
        if app.inbox.frame_done {
            // Check if a resize is needed. We may have applied the resize request to the _other_
            // buffer but not this one.
            if slot.current_buffer_width != app.window_width_pixels
                || slot.current_buffer_height != app.window_height_pixels
            {
                slot.ensure_size(
                    app.window_width_pixels,
                    app.window_height_pixels,
                    &app.proxies.wl_shm_pool,
                    qh,
                );
            }

            app.proxies.wl_surface.frame(qh, ());

            slot.state = RegionState::Busy;

            let graphics_buffer = GraphicsBuffer {
                pixels: &mut app.memory.backbuffer
                    [slot.offset..slot.offset + app.window_height_pixels * POOL_STRIDE_BYTES],
                width_pixels: app.window_width_pixels,
                height_pixels: app.window_height_pixels,
                pitch_bytes: POOL_STRIDE_BYTES,
                bytes_per_pixel: BYTES_PER_PIXEL,
            };

            let _platform_api = PlatformApi;
            // Get new frame content
            let mut graphics_buffer_raw = graphics_buffer.to_raw();
            unsafe {
                (game_code.update_and_render)(
                    &mut game_memory,
                    &app.controller,
                    &mut graphics_buffer_raw,
                );
            }

            app.controller.clear_half_transition_count();
            app.proxies.wl_surface.attach(Some(&slot.buffer), 0, 0);
            app.proxies.wl_surface.damage(
                0,
                0,
                app.window_width_pixels as i32,
                app.window_height_pixels as i32,
            );

            app.inbox.frame_done = false;
        }

        if app.inbox.configure_serial.is_some() {
            app.proxies
                .xdg_surface
                .ack_configure(app.inbox.configure_serial.unwrap());
            app.inbox.configure_serial = None;
        }
        app.proxies.wl_surface.commit();

        debug_span!("dispatch")
            .in_scope(|| event_queue.blocking_dispatch(&mut app))
            .context("Dispatching")?;

        loop_end = Instant::now();
        let _elapsed = loop_end - loop_start;

        loop_start = loop_end;
    }
}

fn draw_audio_debug(
    graphics_buffer: &mut GraphicsBuffer,
    written_window: [usize; 32],
    queued_window: [usize; 32],
    _latency_target_frames: u32,
) {
    draw_vertical(
        graphics_buffer,
        LATENCY_TARGET_FRAMES as usize,
        0,
        graphics_buffer.height_pixels,
        [255, 255, 255, 255],
    );
    let line_height_px = graphics_buffer.height_pixels / 32;

    for (i, (&written, queued)) in written_window.iter().zip(queued_window).enumerate() {
        draw_vertical(
            graphics_buffer,
            queued,
            line_height_px * i,
            line_height_px,
            [0, 255, 0, 0],
        );
        draw_vertical(
            graphics_buffer,
            queued + written,
            line_height_px * i,
            line_height_px,
            [0, 0, 255, 0],
        );
    }
}

fn draw_vertical(
    graphics_buffer: &mut GraphicsBuffer,
    x: usize,
    top: usize,
    height: usize,
    color: [u8; 4],
) {
    let x_ = x * graphics_buffer.width_pixels / 5760;
    for pixel in graphics_buffer.pixels
        [(POOL_STRIDE_BYTES * top)..(top + height) * POOL_STRIDE_BYTES]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .skip(x_)
        .step_by(POOL_STRIDE_BYTES / 4)
    {
        *pixel = color;
    }
}

unsafe extern "C" fn stub_update_and_render(
    _: *mut GameMemory,
    _: *const GameInput,
    _: *mut GraphicsBufferRaw,
) -> bool {
    false
}

unsafe extern "C" fn stub_audio_render(
    _: *mut GameMemory,
    audio_buffer: *mut AudioBufferRaw,
) -> bool {
    let audio_buffer = unsafe { &mut *audio_buffer };
    let samples_buf = unsafe {
        std::slice::from_raw_parts_mut(audio_buffer.samples_buf, audio_buffer.frame_count * 2)
    };
    for sample in samples_buf {
        *sample = 0;
    }
    false
}

struct GameCode {
    _lib: libloading::Library,
    pub update_and_render: GameUpdateAndRenderFn,
    pub audio_render: GameAudioRenderFn,
}

/// # Safety: Deal w/ this
/// Attempt to load a new game library. Symbol lookup failures are mapped to stubs. Other failures
/// (filesystem, etc) return Err. The caller can choose to keep using the old library.
fn load(path: impl Into<PathBuf>, generation_id: u64) -> Result<GameCode> {
    let p: PathBuf = path.into();
    let p_copy = p.with_file_name(format!("libgame_loaded_{}.so", generation_id));

    std::fs::copy(&p, &p_copy).context(format!("Copying {:?} to {:?}", p, p_copy))?;

    let lib =
        unsafe { Library::new(&p_copy) }.context(format!("Creating library from {:?}", p_copy))?;
    let mut complete_load = true;

    let update_and_render_symbol =
        match unsafe { lib.get::<GameUpdateAndRenderFn>(b"game_update_and_render\0") } {
            Ok(s) => *s,
            Err(_) => {
                error!(
                    "Could not locate game_update_and_render in {:?}.  Using fallback",
                    p_copy
                );
                complete_load = false;
                stub_update_and_render
            }
        };
    let audio_render_symbol = match unsafe { lib.get::<GameAudioRenderFn>(b"game_audio_render\0") }
    {
        Ok(s) => *s,
        Err(_) => {
            error!(
                "Could not locate game_audio_render in {:?}. Using fallback",
                p_copy
            );
            complete_load = false;
            stub_audio_render
        }
    };

    if complete_load && generation_id > 0 {
        let p_old = p.with_file_name(format!("libgame_loaded_{}.so", generation_id - 1));
        info!("Successful game lib update, deleting previous generation");
        std::fs::remove_file(p_old).context("Deleting previous libgame")?;
    }
    // TODO(coljnr9): In the event of an error, would like to choose: keep previous working one or use a stubbed
    // one?
    Ok(GameCode {
        _lib: lib,
        update_and_render: update_and_render_symbol,
        audio_render: audio_render_symbol,
    })
}
