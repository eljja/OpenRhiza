#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_gui_blit_rgba_frame(ptr: *const u8, width: u32, height: u32) -> u32;
    fn os_file_size_root_8_3(name_ptr: *const u8) -> u32;
    fn os_file_read_root_8_3(
        name_ptr: *const u8,
        offset: u32,
        ptr: *mut u8,
        max_len: u32,
    ) -> u32;
    fn os_fetch_hid_packet_for_kind(kind: u32, ptr: *mut u8, max_len: u32) -> u32;
    fn os_gui_select_session(session: u32);
    fn os_gui_focus_object(target: u32);
    fn os_gui_set_object_label(handle: u32, ptr: *const u8, len: u32);
    fn os_gui_set_object_style(handle: u32, style: u32);
    fn os_gui_set_object_bounds(handle: u32, x: u32, y: u32, width: u32, height: u32);
    fn os_gui_set_object_interaction(handle: u32, interaction: u32);
}

const TITLE: &[u8] = b"DOOM";
const OWNER: &[u8] = b"skill_doom_port_v1";
const FRAME_W: usize = 640;
const FRAME_H: usize = 480;
const MAP_W: usize = 16;
const MAP_H: usize = 16;
const FIXED: i32 = 256;
const WAD_DIR_CHUNK: usize = 4096;
const MAX_WAD_VERTICES: usize = 2048;
const MAX_WAD_LINES: usize = 2048;
const INIT: &[u8] =
    b"[Skill][doom] initialized: engine state, renderer, input, and WAD bridge stay sandbox-owned.\n";
const READY: &[u8] =
    b"[Skill][doom] runtime started: sandbox Doom game loop active; core only owns bounded frame/input ABI.\n";
const TICK: &[u8] =
    b"[Skill][doom] tick: sandbox frame generated and blitted through bounded Doom ABI.\n";
const WAD_READY: &[u8] =
    b"[Skill][doom] IWAD/PWAD header accepted through bounded root-file ABI; WAD parsing stays sandbox-owned.\n";
const WAD_MAP_READY: &[u8] =
    b"[Skill][doom] WAD map geometry loaded: first-person renderer uses supplied MAP01/E1M1 linedefs from sandbox memory.\n";
const WAD_MISSING: &[u8] =
    b"[Skill][doom] no DOOM1.WAD/DOOM.WAD/FREEDOOM.WAD found; running bundled sandbox episode.\n";
const FOOTER: &[u8] =
    b"Boundary: Doom gameplay/WAD/input/save policy is sandbox skill code; core exposes generic frame and input handles.";
const STATUS: &[u8] =
    b"RUNNING  |  arrows/WASD move  |  space/ctrl fire  |  poll_net keeps the loop alive";

const MAP: &[u8; MAP_W * MAP_H] = b"################\
#..............#\
#..##.....#....#\
#..#......#..#.#\
#..#..##.....#.#\
#.....##..##...#\
#.###.....##.#.#\
#...#........#.#\
#...#..####....#\
#......#.......#\
#.##...#..##...#\
#......#..#....#\
#..###....#..#.#\
#.........#..#.#\
#..............#\
################";

#[derive(Clone, Copy)]
struct GameState {
    running: u32,
    tick: u32,
    player_x: i32,
    player_y: i32,
    angle: u16,
    health: u8,
    ammo: u8,
    kills: u8,
    shot_flash: u8,
    autopilot: u32,
    poll_divider: u32,
}

#[derive(Clone, Copy)]
struct WadLump {
    filepos: u32,
    size: u32,
    name: [u8; 8],
}

#[derive(Clone, Copy)]
struct WadVertex {
    x: i16,
    y: i16,
}

#[derive(Clone, Copy)]
struct WadLine {
    v1: u16,
    v2: u16,
    flags: u16,
}

static mut GAME: GameState = GameState {
    running: 0,
    tick: 0,
    player_x: 2 * FIXED + FIXED / 2,
    player_y: 2 * FIXED + FIXED / 2,
    angle: 0,
    health: 100,
    ammo: 50,
    kills: 0,
    shot_flash: 0,
    autopilot: 0,
    poll_divider: 0,
};
static mut FRAMEBUFFER: [u8; FRAME_W * FRAME_H * 4] = [0; FRAME_W * FRAME_H * 4];
static mut HID_PACKET: [u8; 12] = [0; 12];
static mut WAD_HEADER: [u8; 12] = [0; 12];
static mut WAD_NAME: [u8; 11] = [0; 11];
static mut WAD_READ_BUF: [u8; WAD_DIR_CHUNK] = [0; WAD_DIR_CHUNK];
static mut WAD_VERTICES: [WadVertex; MAX_WAD_VERTICES] =
    [WadVertex { x: 0, y: 0 }; MAX_WAD_VERTICES];
static mut WAD_LINES: [WadLine; MAX_WAD_LINES] =
    [WadLine { v1: 0, v2: 0, flags: 0 }; MAX_WAD_LINES];
static mut WAD_VERTEX_COUNT: usize = 0;
static mut WAD_LINE_COUNT: usize = 0;
static mut WAD_MIN_X: i32 = 0;
static mut WAD_MAX_X: i32 = 0;
static mut WAD_MIN_Y: i32 = 0;
static mut WAD_MAX_Y: i32 = 0;
static mut WAD_MODE: u32 = 0;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn log(bytes: &[u8]) {
    unsafe { os_log(bytes.as_ptr(), bytes.len() as u32) };
}

fn set_label(handle: u32, bytes: &[u8]) {
    unsafe { os_gui_set_object_label(handle, bytes.as_ptr(), bytes.len() as u32) };
}

fn log_hex(label: &[u8], value: u32) {
    let mut buf = [0u8; 96];
    let mut pos = 0usize;
    for &byte in label {
        if pos < buf.len() {
            buf[pos] = byte;
            pos += 1;
        }
    }
    for &byte in b"0x" {
        if pos < buf.len() {
            buf[pos] = byte;
            pos += 1;
        }
    }
    let hex = b"0123456789abcdef";
    for shift in (0..8).rev() {
        if pos < buf.len() {
            buf[pos] = hex[((value >> (shift * 4)) & 0x0f) as usize];
            pos += 1;
        }
    }
    if pos < buf.len() {
        buf[pos] = b'\n';
        pos += 1;
    }
    log(&buf[..pos]);
}

fn read_le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_le_i16(bytes: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn wad_name_matches(name: &[u8; 8], expected: &[u8]) -> bool {
    let mut i = 0usize;
    while i < expected.len() {
        if name[i] != expected[i] {
            return false;
        }
        i += 1;
    }
    while i < 8 {
        if name[i] != 0 && name[i] != b' ' {
            return false;
        }
        i += 1;
    }
    true
}

fn parse_lump_entry(bytes: &[u8], offset: usize) -> WadLump {
    let mut name = [0u8; 8];
    name.copy_from_slice(&bytes[offset + 8..offset + 16]);
    WadLump {
        filepos: read_le_u32(bytes, offset),
        size: read_le_u32(bytes, offset + 4),
        name,
    }
}

fn read_wad_bytes(offset: u32, len: u32) -> u32 {
    if len == 0 || len > WAD_DIR_CHUNK as u32 {
        return 0;
    }
    unsafe {
        os_file_read_root_8_3(
            core::ptr::addr_of!(WAD_NAME) as *const u8,
            offset,
            core::ptr::addr_of_mut!(WAD_READ_BUF) as *mut u8,
            len,
        )
    }
}

fn read_lump_entry(directory_offset: u32, index: u32, lump_count: u32) -> Option<WadLump> {
    if index >= lump_count {
        return None;
    }
    let offset = directory_offset.checked_add(index.checked_mul(16)?)?;
    if read_wad_bytes(offset, 16) != 16 {
        return None;
    }
    let buf = unsafe { &*core::ptr::addr_of!(WAD_READ_BUF) };
    Some(parse_lump_entry(buf, 0))
}

fn find_map_lump_index(directory_offset: u32, lump_count: u32) -> Option<u32> {
    let mut index = 0u32;
    while index < lump_count {
        let entries_left = lump_count - index;
        let entries_to_read = (entries_left as usize).min(WAD_DIR_CHUNK / 16);
        let bytes_to_read = (entries_to_read * 16) as u32;
        let offset = directory_offset.checked_add(index.checked_mul(16)?)?;
        if read_wad_bytes(offset, bytes_to_read) != bytes_to_read {
            return None;
        }
        let buf = unsafe { &*core::ptr::addr_of!(WAD_READ_BUF) };
        let mut local = 0usize;
        while local < entries_to_read {
            let lump = parse_lump_entry(buf, local * 16);
            if wad_name_matches(&lump.name, b"MAP01") || wad_name_matches(&lump.name, b"E1M1") {
                return Some(index + local as u32);
            }
            local += 1;
        }
        index += entries_to_read as u32;
    }
    None
}

fn load_wad_vertices(lump: WadLump) -> u32 {
    let total = (lump.size as usize / 4).min(MAX_WAD_VERTICES);
    if total == 0 {
        return 0;
    }
    unsafe {
        WAD_VERTEX_COUNT = 0;
    }

    let entries_per_read = WAD_DIR_CHUNK / 4;
    let mut loaded = 0usize;
    while loaded < total {
        let entries = (total - loaded).min(entries_per_read);
        let offset = match lump.filepos.checked_add((loaded * 4) as u32) {
            Some(value) => value,
            None => break,
        };
        let read_len = (entries * 4) as u32;
        if read_wad_bytes(offset, read_len) != read_len {
            break;
        }
        let buf = unsafe { &*core::ptr::addr_of!(WAD_READ_BUF) };
        let mut local = 0usize;
        while local < entries {
            let x = read_le_i16(buf, local * 4);
            let y = read_le_i16(buf, local * 4 + 2);
            let index = loaded + local;
            unsafe {
                WAD_VERTICES[index] = WadVertex { x, y };
                if index == 0 {
                    WAD_MIN_X = x as i32;
                    WAD_MAX_X = x as i32;
                    WAD_MIN_Y = y as i32;
                    WAD_MAX_Y = y as i32;
                } else {
                    WAD_MIN_X = WAD_MIN_X.min(x as i32);
                    WAD_MAX_X = WAD_MAX_X.max(x as i32);
                    WAD_MIN_Y = WAD_MIN_Y.min(y as i32);
                    WAD_MAX_Y = WAD_MAX_Y.max(y as i32);
                }
            }
            local += 1;
        }
        loaded += entries;
    }
    unsafe {
        WAD_VERTEX_COUNT = loaded;
    }
    loaded as u32
}

fn load_wad_lines(lump: WadLump) -> u32 {
    let total = (lump.size as usize / 14).min(MAX_WAD_LINES);
    if total == 0 {
        return 0;
    }
    unsafe {
        WAD_LINE_COUNT = 0;
    }

    let entries_per_read = WAD_DIR_CHUNK / 14;
    let mut loaded = 0usize;
    while loaded < total {
        let entries = (total - loaded).min(entries_per_read);
        let offset = match lump.filepos.checked_add((loaded * 14) as u32) {
            Some(value) => value,
            None => break,
        };
        let read_len = (entries * 14) as u32;
        if read_wad_bytes(offset, read_len) != read_len {
            break;
        }
        let buf = unsafe { &*core::ptr::addr_of!(WAD_READ_BUF) };
        let mut local = 0usize;
        while local < entries {
            let index = loaded + local;
            unsafe {
                WAD_LINES[index] = WadLine {
                    v1: read_le_u16(buf, local * 14),
                    v2: read_le_u16(buf, local * 14 + 2),
                    flags: read_le_u16(buf, local * 14 + 4),
                };
            };
            local += 1;
        }
        loaded += entries;
    }
    unsafe {
        WAD_LINE_COUNT = loaded;
    }
    loaded as u32
}

fn load_wad_player_start(lump: WadLump) {
    let total = lump.size as usize / 10;
    let entries_per_read = WAD_DIR_CHUNK / 10;
    let mut loaded = 0usize;
    while loaded < total {
        let entries = (total - loaded).min(entries_per_read);
        let offset = match lump.filepos.checked_add((loaded * 10) as u32) {
            Some(value) => value,
            None => return,
        };
        let read_len = (entries * 10) as u32;
        if read_wad_bytes(offset, read_len) != read_len {
            return;
        }
        let buf = unsafe { &*core::ptr::addr_of!(WAD_READ_BUF) };
        let mut local = 0usize;
        while local < entries {
            let thing_type = read_le_i16(buf, local * 10 + 6);
            if thing_type == 1 {
                let x = read_le_i16(buf, local * 10) as i32;
                let y = read_le_i16(buf, local * 10 + 2) as i32;
                unsafe {
                    GAME.player_x = x * 4;
                    GAME.player_y = y * 4;
                }
                return;
            }
            local += 1;
        }
        loaded += entries;
    }
}

fn load_wad_map(directory_offset: u32, lump_count: u32) -> u32 {
    let Some(map_index) = find_map_lump_index(directory_offset, lump_count) else {
        return 0;
    };
    let Some(things) = read_lump_entry(directory_offset, map_index + 1, lump_count) else {
        return 0;
    };
    let Some(linedefs) = read_lump_entry(directory_offset, map_index + 2, lump_count) else {
        return 0;
    };
    let Some(vertexes) = read_lump_entry(directory_offset, map_index + 4, lump_count) else {
        return 0;
    };
    if !wad_name_matches(&linedefs.name, b"LINEDEFS")
        || !wad_name_matches(&vertexes.name, b"VERTEXES")
    {
        return 0;
    }

    let vertices = load_wad_vertices(vertexes);
    let lines = load_wad_lines(linedefs);
    load_wad_player_start(things);
    if vertices == 0 || lines == 0 {
        return 0;
    }

    unsafe {
        WAD_MODE = 2;
    }
    log(WAD_MAP_READY);
    log_hex(b"[Skill][doom] map vertices=", vertices);
    log_hex(b"[Skill][doom] map linedefs=", lines);
    1
}

fn probe_wad() -> u32 {
    let names: [&[u8; 11]; 3] = [b"DOOM1   WAD", b"DOOM    WAD", b"FREEDOOMWAD"];
    for name in names {
        let size = unsafe { os_file_size_root_8_3(name.as_ptr()) };
        if size < 12 {
            continue;
        }
        let read = unsafe {
            os_file_read_root_8_3(
                name.as_ptr(),
                0,
                core::ptr::addr_of_mut!(WAD_HEADER) as *mut u8,
                12,
            )
        };
        if read != 12 {
            continue;
        }
        let header = unsafe { WAD_HEADER };
        let magic_ok = &header[0..4] == b"IWAD" || &header[0..4] == b"PWAD";
        if !magic_ok {
            continue;
        }
        let lump_count = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        let directory_offset = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
        unsafe {
            let mut i = 0usize;
            while i < 11 {
                WAD_NAME[i] = name[i];
                i += 1;
            }
            WAD_MODE = 1;
        }
        log(WAD_READY);
        log_hex(b"[Skill][doom] WAD size=", size);
        log_hex(b"[Skill][doom] WAD lumps=", lump_count);
        log_hex(b"[Skill][doom] WAD directory=", directory_offset);
        let map_loaded = load_wad_map(directory_offset, lump_count);
        return if map_loaded != 0 { 2 } else { 1 };
    }
    unsafe {
        WAD_NAME = [0; 11];
        WAD_MODE = 0;
        WAD_VERTEX_COUNT = 0;
        WAD_LINE_COUNT = 0;
    }
    log(WAD_MISSING);
    0
}

fn draw_gui_chrome() {
    unsafe {
        os_gui_select_session(1);
        os_gui_focus_object(20);

        os_gui_set_object_bounds(20, 284, 92, 1612, 760);
        os_gui_set_object_style(20, 5);
        os_gui_set_object_interaction(20, 3);
        set_label(20, b"DOOM sandbox runtime is drawing into the framebuffer handle.");

        os_gui_set_object_bounds(30, 304, 862, 1592, 132);
        os_gui_set_object_style(30, 8);
        os_gui_set_object_interaction(30, 2);
        if WAD_MODE == 0 {
            set_label(30, STATUS);
        } else {
            set_label(30, b"RUNNING  |  WAD header accepted  |  sandbox owns WAD parsing  |  core owns no Doom policy");
        }

        os_gui_set_object_bounds(31, 316, 908, 1568, 78);
        os_gui_set_object_style(31, 11);
        set_label(31, TITLE);

        os_gui_set_object_bounds(40, 304, 1010, 1592, 44);
        os_gui_set_object_style(40, 9);
        set_label(40, FOOTER);

        os_gui_set_object_bounds(41, 316, 1034, 1568, 16);
        os_gui_set_object_style(41, 11);
        set_label(41, OWNER);
    }
}

fn put_pixel(x: usize, y: usize, r: u8, g: u8, b: u8) {
    if x >= FRAME_W || y >= FRAME_H {
        return;
    }
    let offset = (y * FRAME_W + x) * 4;
    unsafe {
        FRAMEBUFFER[offset] = r;
        FRAMEBUFFER[offset + 1] = g;
        FRAMEBUFFER[offset + 2] = b;
        FRAMEBUFFER[offset + 3] = 255;
    }
}

fn fill_rect(x0: usize, y0: usize, w: usize, h: usize, r: u8, g: u8, b: u8) {
    let xmax = (x0 + w).min(FRAME_W);
    let ymax = (y0 + h).min(FRAME_H);
    let mut y = y0;
    while y < ymax {
        let mut x = x0;
        while x < xmax {
            put_pixel(x, y, r, g, b);
            x += 1;
        }
        y += 1;
    }
}

fn shade(value: i32) -> u8 {
    if value < 0 {
        0
    } else if value > 255 {
        255
    } else {
        value as u8
    }
}

fn sin1024(angle: i32) -> i32 {
    let x = angle & 1023;
    if x < 256 {
        x * 4
    } else if x < 512 {
        (512 - x) * 4
    } else if x < 768 {
        -((x - 512) * 4)
    } else {
        -((1024 - x) * 4)
    }
}

fn cos1024(angle: i32) -> i32 {
    sin1024(angle + 256)
}

fn map_cell(x: i32, y: i32) -> u8 {
    if x < 0 || y < 0 || x >= MAP_W as i32 || y >= MAP_H as i32 {
        return b'#';
    }
    MAP[y as usize * MAP_W + x as usize]
}

fn solid_at(world_x: i32, world_y: i32) -> bool {
    if unsafe { WAD_MODE } == 2 {
        return wad_blocks_point(world_x / 4, world_y / 4, 18);
    }
    map_cell(world_x / FIXED, world_y / FIXED) == b'#'
}

fn wad_blocks_point(px: i32, py: i32, radius: i32) -> bool {
    unsafe {
        let mut i = 0usize;
        while i < WAD_LINE_COUNT {
            let line = WAD_LINES[i];
            if (line.flags & 1) != 0 {
                let v1 = line.v1 as usize;
                let v2 = line.v2 as usize;
                if v1 < WAD_VERTEX_COUNT && v2 < WAD_VERTEX_COUNT {
                    let a = WAD_VERTICES[v1];
                    let b = WAD_VERTICES[v2];
                    if point_segment_distance_sq(
                        px,
                        py,
                        a.x as i32,
                        a.y as i32,
                        b.x as i32,
                        b.y as i32,
                    ) <= radius * radius
                    {
                        return true;
                    }
                }
            }
            i += 1;
        }
    }
    false
}

fn point_segment_distance_sq(px: i32, py: i32, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let ab_len_sq = abx * abx + aby * aby;
    if ab_len_sq == 0 {
        let dx = px - ax;
        let dy = py - ay;
        return dx * dx + dy * dy;
    }
    let t = ((apx * abx + apy * aby) * 1024 / ab_len_sq).clamp(0, 1024);
    let cx = ax + abx * t / 1024;
    let cy = ay + aby * t / 1024;
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy
}

fn try_move(game: &mut GameState, dx: i32, dy: i32) {
    let nx = game.player_x + dx;
    let ny = game.player_y + dy;
    if !solid_at(nx, game.player_y) {
        game.player_x = nx;
    }
    if !solid_at(game.player_x, ny) {
        game.player_y = ny;
    }
}

fn key_down(report: &[u8; 12], key: u8) -> bool {
    let mut i = 6;
    while i < 12 {
        if report[i] == key {
            return true;
        }
        i += 1;
    }
    false
}

fn poll_keyboard(game: &mut GameState) -> bool {
    let len = unsafe {
        os_fetch_hid_packet_for_kind(
            1,
            core::ptr::addr_of_mut!(HID_PACKET) as *mut u8,
            12,
        )
    };
    if len < 12 {
        return false;
    }

    let packet = unsafe { HID_PACKET };
    let mut active = false;
    if key_down(&packet, 0x50) || key_down(&packet, 0x04) {
        game.angle = game.angle.wrapping_sub(28);
        active = true;
    }
    if key_down(&packet, 0x4f) || key_down(&packet, 0x07) {
        game.angle = game.angle.wrapping_add(28);
        active = true;
    }

    let forward = key_down(&packet, 0x52) || key_down(&packet, 0x1a);
    let backward = key_down(&packet, 0x51) || key_down(&packet, 0x16);
    let strafe_left = key_down(&packet, 0x14);
    let strafe_right = key_down(&packet, 0x08);
    let speed = 18;
    let ca = cos1024(game.angle as i32);
    let sa = sin1024(game.angle as i32);
    if forward {
        try_move(game, ca * speed / 1024, sa * speed / 1024);
        active = true;
    }
    if backward {
        try_move(game, -ca * speed / 1024, -sa * speed / 1024);
        active = true;
    }
    if strafe_left {
        try_move(game, sa * speed / 1024, -ca * speed / 1024);
        active = true;
    }
    if strafe_right {
        try_move(game, -sa * speed / 1024, ca * speed / 1024);
        active = true;
    }

    let modifiers = packet[4];
    if key_down(&packet, 0x2c) || (modifiers & 0x11) != 0 {
        if game.ammo > 0 {
            game.ammo -= 1;
            game.shot_flash = 5;
            if game.kills < 99 {
                game.kills += 1;
            }
        }
        active = true;
    }

    active
}

fn step_game(game: &mut GameState, forced: bool) {
    game.tick = game.tick.wrapping_add(1);
    if game.shot_flash > 0 {
        game.shot_flash -= 1;
    }

    let input_active = poll_keyboard(game);
    if !input_active {
        game.autopilot = game.autopilot.wrapping_add(1);
        if forced || (game.autopilot & 3) == 0 {
            game.angle = game.angle.wrapping_add(5);
        }
        let ca = cos1024(game.angle as i32);
        let sa = sin1024(game.angle as i32);
        let old_x = game.player_x;
        let old_y = game.player_y;
        try_move(game, ca * 8 / 1024, sa * 8 / 1024);
        if old_x == game.player_x && old_y == game.player_y {
            game.angle = game.angle.wrapping_add(180);
        }
    } else {
        game.autopilot = 0;
    }
}

fn render_world(game: &GameState) {
    if unsafe { WAD_MODE } == 2 {
        render_wad_first_person_frame(game);
        return;
    }

    let horizon = FRAME_H / 2;
    let mut y = 0;
    while y < horizon {
        let sky = 36 + y as i32;
        fill_rect(0, y, FRAME_W, 1, shade(sky / 2), shade(sky / 3), shade(sky + 64));
        y += 1;
    }
    while y < FRAME_H {
        let floor = 74 - ((y - horizon) as i32 / 2);
        fill_rect(0, y, FRAME_W, 1, shade(floor), shade(floor - 10), shade(floor - 18));
        y += 1;
    }

    let fov = 230i32;
    let mut x = 0usize;
    while x < FRAME_W {
        let ray_angle = game.angle as i32 + (x as i32 * fov / FRAME_W as i32) - fov / 2;
        let dx = cos1024(ray_angle);
        let dy = sin1024(ray_angle);
        let mut dist = 32i32;
        let mut hit = false;
        let mut hit_x = 0i32;
        let mut hit_y = 0i32;
        while dist < 2300 {
            let wx = game.player_x + dx * dist / 1024;
            let wy = game.player_y + dy * dist / 1024;
            if solid_at(wx, wy) {
                hit = true;
                hit_x = wx / FIXED;
                hit_y = wy / FIXED;
                break;
            }
            dist += 18;
        }

        if hit {
            let corrected = (dist * (900 + cos1024(ray_angle - game.angle as i32).abs() / 8)) / 1024;
            let wall_h = (FRAME_H as i32 * 190 / corrected.max(1)).clamp(4, FRAME_H as i32);
            let top = (horizon as i32 - wall_h / 2).max(0) as usize;
            let bottom = (horizon as i32 + wall_h / 2).min(FRAME_H as i32 - 1) as usize;
            let mortar = ((hit_x + hit_y + (x as i32 / 4)) & 3) == 0
                || ((top as i32 + bottom as i32) & 15) < 2;
            let base = 210 - corrected / 11;
            let (r, g, b) = if mortar {
                (shade(base / 2), shade(base / 2 - 10), shade(base / 2 - 18))
            } else if (hit_x ^ hit_y) & 1 == 0 {
                (shade(base + 36), shade(base / 3), shade(base / 4))
            } else {
                (shade(base + 10), shade(base / 2), shade(base / 3))
            };
            fill_rect(x, top, 1, bottom.saturating_sub(top).max(1), r, g, b);
        }
        x += 1;
    }
}

fn ray_segment_distance(
    px: i32,
    py: i32,
    ray_dx: i32,
    ray_dy: i32,
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
) -> Option<i32> {
    let sx = bx - ax;
    let sy = by - ay;
    let rel_x = ax - px;
    let rel_y = ay - py;
    let den = ray_dx as i64 * sy as i64 - ray_dy as i64 * sx as i64;
    if den == 0 {
        return None;
    }
    let t_num = rel_x as i64 * sy as i64 - rel_y as i64 * sx as i64;
    let u_num = rel_x as i64 * ray_dy as i64 - rel_y as i64 * ray_dx as i64;

    if den > 0 {
        if t_num < 0 || u_num < 0 || u_num > den {
            return None;
        }
    } else if t_num > 0 || u_num > 0 || u_num < den {
        return None;
    }

    let dist = (t_num * 1024 / den) as i32;
    if dist <= 1 {
        None
    } else {
        Some(dist)
    }
}

fn cast_wad_ray(px: i32, py: i32, angle: i32) -> Option<(i32, u16, usize)> {
    let ray_dx = cos1024(angle);
    let ray_dy = sin1024(angle);
    let mut best_dist = i32::MAX;
    let mut best_flags = 0u16;
    let mut best_index = 0usize;

    unsafe {
        let mut i = 0usize;
        while i < WAD_LINE_COUNT {
            let line = WAD_LINES[i];
            let v1 = line.v1 as usize;
            let v2 = line.v2 as usize;
            if v1 < WAD_VERTEX_COUNT && v2 < WAD_VERTEX_COUNT {
                let a = WAD_VERTICES[v1];
                let b = WAD_VERTICES[v2];
                if let Some(dist) = ray_segment_distance(
                    px,
                    py,
                    ray_dx,
                    ray_dy,
                    a.x as i32,
                    a.y as i32,
                    b.x as i32,
                    b.y as i32,
                ) {
                    if dist < best_dist {
                        best_dist = dist;
                        best_flags = line.flags;
                        best_index = i;
                    }
                }
            }
            i += 1;
        }
    }

    if best_dist == i32::MAX {
        None
    } else {
        Some((best_dist, best_flags, best_index))
    }
}

fn render_wad_first_person_frame(game: &GameState) {
    let horizon = FRAME_H / 2;
    let mut y = 0usize;
    while y < horizon {
        let sky = 28 + y as i32;
        fill_rect(0, y, FRAME_W, 1, shade(sky / 2), shade(sky / 2), shade(sky + 50));
        y += 1;
    }
    while y < FRAME_H {
        let floor = 80 - ((y - horizon) as i32 / 2);
        fill_rect(0, y, FRAME_W, 1, shade(floor), shade(floor - 14), shade(floor - 20));
        y += 1;
    }

    let px = game.player_x / 4;
    let py = game.player_y / 4;
    let fov = 260i32;
    let mut x = 0usize;
    while x < FRAME_W {
        let delta = (x as i32 * fov / FRAME_W as i32) - fov / 2;
        let ray_angle = game.angle as i32 + delta;
        if let Some((dist, flags, index)) = cast_wad_ray(px, py, ray_angle) {
            let corrected = (dist * cos1024(delta).abs().max(128)) / 1024;
            let wall_h = (FRAME_H as i32 * 150 / corrected.max(1)).clamp(5, FRAME_H as i32);
            let top = (horizon as i32 - wall_h / 2).max(0) as usize;
            let bottom = (horizon as i32 + wall_h / 2).min(FRAME_H as i32 - 1) as usize;
            let fade = 230 - corrected / 5;
            let pattern = ((index as i32 + x as i32 / 3 + top as i32) & 7) < 2;
            let (r, g, b) = if (flags & 1) != 0 {
                (shade(fade + 36), shade(fade / 4), shade(fade / 5))
            } else if (flags & 4) != 0 {
                (shade(fade / 3), shade(fade / 2), shade(fade + 18))
            } else {
                (shade(fade + 4), shade(fade / 2), shade(fade / 3))
            };
            let (r, g, b) = if pattern {
                (r / 2, g / 2, b / 2)
            } else {
                (r, g, b)
            };
            fill_rect(x, top, 1, bottom.saturating_sub(top).max(1), r, g, b);
        }
        x += 1;
    }

    render_wad_minimap(game);
}

fn draw_line(mut x0: i32, mut y0: i32, x1: i32, y1: i32, r: u8, g: u8, b: u8) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= 0 && y0 >= 0 {
            put_pixel(x0 as usize, y0 as usize, r, g, b);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = err * 2;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn render_wad_minimap(game: &GameState) {
    let map_x = 16usize;
    let map_y = 16usize;
    let map_w = 160usize;
    let map_h = 104usize;
    fill_rect(map_x, map_y, map_w, map_h, 5, 6, 8);
    fill_rect(map_x + 1, map_y + 1, map_w - 2, map_h - 2, 12, 14, 16);

    unsafe {
        let mut i = 0usize;
        while i < WAD_LINE_COUNT {
            let line = WAD_LINES[i];
            let v1 = line.v1 as usize;
            let v2 = line.v2 as usize;
            if v1 < WAD_VERTEX_COUNT && v2 < WAD_VERTEX_COUNT {
                let a = WAD_VERTICES[v1];
                let b = WAD_VERTICES[v2];
                let color = if (line.flags & 1) != 0 {
                    (210, 40, 32)
                } else if (line.flags & 4) != 0 {
                    (76, 126, 170)
                } else {
                    (170, 92, 54)
                };
                let ax = map_x as i32 + wad_to_minimap_x(a.x as i32, map_w);
                let ay = map_y as i32 + wad_to_minimap_y(a.y as i32, map_h);
                let bx = map_x as i32 + wad_to_minimap_x(b.x as i32, map_w);
                let by = map_y as i32 + wad_to_minimap_y(b.y as i32, map_h);
                draw_line(ax, ay, bx, by, color.0, color.1, color.2);
            }
            i += 1;
        }
    }

    let px = map_x as i32 + wad_to_minimap_x(game.player_x / 4, map_w);
    let py = map_y as i32 + wad_to_minimap_y(game.player_y / 4, map_h);
    fill_rect(px.saturating_sub(4) as usize, py.saturating_sub(4) as usize, 9, 9, 244, 218, 68);
    let dx = cos1024(game.angle as i32) * 18 / 1024;
    let dy = -sin1024(game.angle as i32) * 18 / 1024;
    draw_line(px, py, px + dx, py + dy, 255, 248, 164);
}

fn wad_to_minimap_x(x: i32, width: usize) -> i32 {
    unsafe {
        let span = (WAD_MAX_X - WAD_MIN_X).max(1);
        ((x - WAD_MIN_X) * (width as i32 - 4) / span + 2).clamp(0, width as i32 - 1)
    }
}

fn wad_to_minimap_y(y: i32, height: usize) -> i32 {
    unsafe {
        let span = (WAD_MAX_Y - WAD_MIN_Y).max(1);
        ((WAD_MAX_Y - y) * (height as i32 - 4) / span + 2).clamp(0, height as i32 - 1)
    }
}

fn render_sprite(game: &GameState) {
    let phase = ((game.tick / 12) & 3) as usize;
    let cx = FRAME_W / 2 + phase.saturating_sub(1) * 8;
    let cy = FRAME_H / 2 + 48;
    let size = 36usize;
    let r = if game.shot_flash > 0 { 255 } else { 128 };
    fill_rect(cx - size / 2, cy - size, size, size * 2, r, 24, 18);
    fill_rect(cx - size / 4, cy - size / 2, size / 2, size / 2, 250, 210, 120);
}

fn render_weapon(game: &GameState) {
    let sway = (((game.tick >> 2) & 3) as usize) * 4;
    let gun_x = FRAME_W / 2 - 36 + sway;
    let gun_y = FRAME_H - 126;
    fill_rect(gun_x, gun_y, 72, 72, 60, 58, 56);
    fill_rect(gun_x + 20, gun_y - 40, 32, 56, 96, 94, 90);
    fill_rect(gun_x + 26, gun_y - 54, 20, 18, 62, 62, 64);
    if game.shot_flash > 0 {
        fill_rect(gun_x + 10, gun_y - 78, 52, 38, 255, 220, 80);
        fill_rect(gun_x + 22, gun_y - 96, 28, 24, 255, 146, 40);
    }
}

fn render_hud(game: &GameState) {
    let hud_y = FRAME_H - 56;
    fill_rect(0, hud_y, FRAME_W, 56, 36, 32, 30);
    fill_rect(0, hud_y, FRAME_W, 3, 122, 32, 26);
    fill_rect(16, hud_y + 16, game.health as usize * 2, 16, 176, 22, 20);
    fill_rect(448, hud_y + 16, game.ammo as usize, 16, 196, 148, 42);
    fill_rect(FRAME_W / 2 - 22, hud_y + 10, 44, 38, 120, 88, 58);
    let eye = if game.health > 40 { 235 } else { 235 - game.health };
    fill_rect(FRAME_W / 2 - 10, hud_y + 24, 6, 6, eye, 20, 20);
    fill_rect(FRAME_W / 2 + 4, hud_y + 24, 6, 6, eye, 20, 20);
    let kills_w = (game.kills as usize * 3).min(108);
    fill_rect(510, hud_y + 42, kills_w, 6, 230, 230, 210);
}

fn render_title_overlay() {
    fill_rect(136, 32, 368, 68, 18, 10, 8);
    fill_rect(152, 44, 56, 44, 176, 24, 18);
    fill_rect(224, 44, 56, 44, 176, 24, 18);
    fill_rect(296, 44, 56, 44, 176, 24, 18);
    fill_rect(368, 44, 104, 44, 176, 24, 18);
    fill_rect(384, 56, 16, 28, 30, 10, 10);
    fill_rect(432, 56, 16, 28, 30, 10, 10);
}

fn render_frame(game: &GameState) -> u32 {
    render_world(game);
    if unsafe { WAD_MODE } != 2 {
        render_sprite(game);
    }
    render_weapon(game);
    render_hud(game);
    if game.tick < 20 && unsafe { WAD_MODE } != 2 {
        render_title_overlay();
    }
    let ptr = core::ptr::addr_of!(FRAMEBUFFER) as *const u8;
    unsafe { os_gui_blit_rgba_frame(ptr, FRAME_W as u32, FRAME_H as u32) }
}

fn reset_game() {
    unsafe {
        GAME = GameState {
            running: 1,
            tick: 0,
            player_x: 2 * FIXED + FIXED / 2,
            player_y: 2 * FIXED + FIXED / 2,
            angle: 0,
            health: 100,
            ammo: 50,
            kills: 0,
            shot_flash: 0,
            autopilot: 0,
            poll_divider: 0,
        };
    }
}

fn step_and_render(forced: bool) -> u32 {
    unsafe {
        let game = &mut *core::ptr::addr_of_mut!(GAME);
        if game.running == 0 {
            game.running = 1;
        }
        step_game(game, forced);
        render_frame(game)
    }
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log(INIT);
    reset_game();
    let _ = probe_wad();
    draw_gui_chrome();
    unsafe {
        let game = &*core::ptr::addr_of!(GAME);
        let _ = render_frame(game);
    }
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log(READY);
    reset_game();
    let _ = probe_wad();
    draw_gui_chrome();
    let _ = step_and_render(true);
    1
}

#[no_mangle]
pub extern "C" fn tick_skill() -> i32 {
    log(TICK);
    step_and_render(true) as i32
}

#[no_mangle]
pub extern "C" fn poll_net() {
    unsafe {
        let game = &mut *core::ptr::addr_of_mut!(GAME);
        if game.running == 0 {
            return;
        }
        game.poll_divider = game.poll_divider.wrapping_add(1);
        if (game.poll_divider & 7) != 0 {
            return;
        }
    }
    let _ = step_and_render(false);
}
