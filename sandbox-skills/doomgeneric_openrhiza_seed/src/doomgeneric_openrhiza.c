#include "doomgeneric.h"
#include "doomkeys.h"
#include "d_loop.h"
#include "m_menu.h"
#include "r_main.h"
#include "r_state.h"
#include <stdint.h>
#include <string.h>

__attribute__((import_module("env"), import_name("os_log")))
void os_log(const unsigned char *ptr, unsigned int len);
__attribute__((import_module("env"), import_name("os_gui_blit_rgba_frame")))
unsigned int os_gui_blit_rgba_frame(const unsigned char *ptr, unsigned int width, unsigned int height);
__attribute__((import_module("env"), import_name("os_frame_resource_create")))
unsigned int os_frame_resource_create(unsigned int width, unsigned int height, unsigned int format, unsigned int target_hz);
__attribute__((import_module("env"), import_name("os_frame_resource_metric")))
unsigned int os_frame_resource_metric(unsigned int handle, unsigned int metric);
__attribute__((import_module("env"), import_name("os_frame_resource_submit_rgba")))
unsigned int os_frame_resource_submit_rgba(unsigned int handle, const unsigned char *ptr, unsigned int width, unsigned int height, unsigned int pitch_bytes);
__attribute__((import_module("env"), import_name("os_frame_resource_submit_rgba_seq")))
unsigned int os_frame_resource_submit_rgba_seq(unsigned int handle, const unsigned char *ptr, unsigned int width, unsigned int height, unsigned int pitch_bytes, unsigned int frame_sequence);
__attribute__((import_module("env"), import_name("os_frame_resource_submit_indexed8_seq")))
unsigned int os_frame_resource_submit_indexed8_seq(unsigned int handle, const unsigned char *pixels, unsigned int width, unsigned int height, unsigned int pitch_bytes, const unsigned int *palette, unsigned int palette_entries, unsigned int frame_sequence);
__attribute__((import_module("env"), import_name("os_now_ms")))
unsigned int os_now_ms(void);
__attribute__((import_module("env"), import_name("os_gui_release_frame")))
void os_gui_release_frame(void);
__attribute__((import_module("env"), import_name("os_gui_frame_close_requested")))
unsigned int os_gui_frame_close_requested(void);
__attribute__((import_module("env"), import_name("os_fetch_hid_packet_for_kind")))
unsigned int os_fetch_hid_packet_for_kind(unsigned int kind, unsigned char *ptr, unsigned int max_len);

static int created;
static int running;
static unsigned short key_queue[32];
static unsigned int key_w, key_r;
static unsigned char last_keys[6];
static unsigned int fps_window_ms;
static unsigned int fps_frame_count;
static unsigned int fps_value;
static unsigned int last_game_tick_ms;
static unsigned int last_present_ms;
static unsigned int frame_handle;
static unsigned int render_sequence;
static const unsigned char *last_indexed_pixels;
static const unsigned int *last_indexed_palette;
static unsigned int last_quality_check_ms;
static unsigned int quality_recover_since_ms;
static int adaptive_screenblocks;
static int indexed_present_mode;
static unsigned char fullscreen_indexed_buffer[DOOMGENERIC_RESX * DOOMGENERIC_RESY];
static pixel_t fullscreen_rgba_buffer[DOOMGENERIC_RESX * DOOMGENERIC_RESY];
static unsigned short fullscreen_x_map[DOOMGENERIC_RESX];
static unsigned short fullscreen_hud_x_map[DOOMGENERIC_RESX];
static unsigned short fullscreen_y_map[DOOMGENERIC_RESY];
static int fullscreen_map_x;
static int fullscreen_map_y;
static int fullscreen_map_w;
static int fullscreen_map_h;
static int fullscreen_map_play_h;
static int fullscreen_map_hud_h;

#define DOOM_GAME_TICK_MS 1u
#define DOOM_PRESENT_MIN_MS 1u
#define FRAME_METRIC_FPS 1u
#define FRAME_METRIC_FRESH_FPS 2u
#define FRAME_METRIC_FLOOR_VIOLATIONS 5u
#define DOOM_QUALITY_CHECK_MS 500u
#define DOOM_QUALITY_FLOOR_GUARD_FPS 90u
#define DOOM_QUALITY_RECOVER_FPS 130u
#define DOOM_QUALITY_RECOVER_MS 60000u
#define DOOM_QUALITY_MIN_SCREENBLOCKS 6
#define DOOM_QUALITY_MAX_SCREENBLOCKS 6

static void log_s(const char *s) { os_log((const unsigned char *)s, (unsigned int)strlen(s)); }

static unsigned char hid_to_doom(unsigned char key) {
    switch (key) {
        case 0x50: case 0x04: return KEY_LEFTARROW;
        case 0x4f: case 0x07: return KEY_RIGHTARROW;
        case 0x52: case 0x1a: return KEY_UPARROW;
        case 0x51: case 0x16: return KEY_DOWNARROW;
        case 0x2c: return KEY_FIRE;
        case 0x1d: return KEY_USE;
        case 0x29: return KEY_ESCAPE;
        case 0x28: return KEY_ENTER;
        default: return key;
    }
}

static int key_in_last(unsigned char key) {
    for (int i = 0; i < 6; ++i) if (last_keys[i] == key) return 1;
    return 0;
}

static void push_key(int pressed, unsigned char key) {
    if (pressed && key == 0x45) {
        running = 0;
        os_gui_release_frame();
        return;
    }
    unsigned char doom_key = hid_to_doom(key);
    if (!doom_key) return;
    key_queue[key_w++ & 31] = (unsigned short)((pressed << 8) | doom_key);
}

static void poll_hid(void) {
    unsigned char packet[12];
    unsigned int len = os_fetch_hid_packet_for_kind(1, packet, sizeof(packet));
    if (len < 12) return;
    unsigned char now[6];
    for (int i = 0; i < 6; ++i) now[i] = packet[6 + i];
    for (int i = 0; i < 6; ++i) if (now[i] && !key_in_last(now[i])) push_key(1, now[i]);
    for (int i = 0; i < 6; ++i) {
        unsigned char old = last_keys[i];
        int still = 0;
        for (int j = 0; j < 6; ++j) if (now[j] == old) still = 1;
        if (old && !still) push_key(0, old);
    }
    memcpy(last_keys, now, sizeof(last_keys));
}

static const unsigned char font_3x5[14][5] = {
    {7, 5, 5, 5, 7},
    {2, 6, 2, 2, 7},
    {7, 1, 7, 4, 7},
    {7, 1, 7, 1, 7},
    {5, 5, 7, 1, 1},
    {7, 4, 7, 1, 7},
    {7, 4, 7, 5, 7},
    {7, 1, 1, 1, 1},
    {7, 5, 7, 5, 7},
    {7, 5, 7, 1, 7},
    {0, 0, 0, 0, 0},
    {7, 4, 6, 4, 4},
    {6, 5, 6, 4, 4},
    {7, 4, 7, 1, 7},
};

static void draw_glyph_3x5(int x, int y, int glyph, pixel_t color) {
    for (int row = 0; row < 5; ++row) {
        unsigned char bits = font_3x5[glyph][row];
        for (int col = 0; col < 3; ++col) {
            if (bits & (1 << (2 - col))) {
                int px = x + col;
                int py = y + row;
                if (px >= 0 && py >= 0 && px < DOOMGENERIC_RESX && py < DOOMGENERIC_RESY) {
                    DG_ScreenBuffer[py * DOOMGENERIC_RESX + px] = color;
                }
            }
        }
    }
}

static void draw_overlay_text(const char *text, int x, int y) {
    const pixel_t fg = (pixel_t)0x00ffffff;
    const pixel_t shadow = (pixel_t)0x00000000;
    int cursor = x;
    for (const char *p = text; *p; ++p) {
        int glyph = 10;
        if (*p >= '0' && *p <= '9') glyph = *p - '0';
        else if (*p == 'F') glyph = 11;
        else if (*p == 'P') glyph = 12;
        else if (*p == 'S') glyph = 13;
        if (*p != ' ') {
            draw_glyph_3x5(cursor + 1, y + 1, glyph, shadow);
            draw_glyph_3x5(cursor, y, glyph, fg);
        }
        cursor += 4;
    }
}

static void draw_fps_overlay(void) {
    unsigned int now = os_now_ms();
    if (!fps_window_ms) fps_window_ms = now;
    fps_frame_count++;
    if (now - fps_window_ms >= 1000) {
        fps_value = fps_frame_count;
        fps_frame_count = 0;
        fps_window_ms = now;
    }

    char text[9];
    unsigned int fps = fps_value;
    if (fps > 999) fps = 999;
    text[0] = 'F';
    text[1] = 'P';
    text[2] = 'S';
    text[3] = ' ';
    text[4] = (char)('0' + (fps / 100) % 10);
    text[5] = (char)('0' + (fps / 10) % 10);
    text[6] = (char)('0' + fps % 10);
    text[7] = 0;
    for (int y = 2; y < 12; ++y) {
        for (int x = 2; x < 36; ++x) {
            DG_ScreenBuffer[y * DOOMGENERIC_RESX + x] = (pixel_t)0x00000000;
        }
    }
    draw_overlay_text(text, 4, 4);
}

static void draw_indexed_glyph_3x5(unsigned char *pixels, int x, int y, int glyph, unsigned char color) {
    for (int row = 0; row < 5; ++row) {
        unsigned char bits = font_3x5[glyph][row];
        for (int col = 0; col < 3; ++col) {
            if (bits & (1 << (2 - col))) {
                int px = x + col;
                int py = y + row;
                if (px >= 0 && py >= 0 && px < DOOMGENERIC_RESX && py < DOOMGENERIC_RESY) {
                    pixels[py * DOOMGENERIC_RESX + px] = color;
                }
            }
        }
    }
}

static unsigned char brightest_palette_index(const unsigned int *palette) {
    unsigned int best = 1;
    unsigned int best_score = 0;
    for (unsigned int i = 1; i < 256; ++i) {
        unsigned int value = palette[i];
        unsigned int score = (value & 0xff) + ((value >> 8) & 0xff) + ((value >> 16) & 0xff);
        if (score > best_score) {
            best_score = score;
            best = i;
        }
    }
    return (unsigned char)best;
}

static void draw_indexed_fps_overlay(unsigned char *pixels, const unsigned int *palette) {
    unsigned int now = os_now_ms();
    if (!fps_window_ms) fps_window_ms = now;
    fps_frame_count++;
    if (now - fps_window_ms >= 1000) {
        fps_value = fps_frame_count;
        fps_frame_count = 0;
        fps_window_ms = now;
    }

    unsigned int fps = fps_value;
    if (fps > 999) fps = 999;
    char text[9];
    text[0] = 'F';
    text[1] = 'P';
    text[2] = 'S';
    text[3] = ' ';
    text[4] = (char)('0' + (fps / 100) % 10);
    text[5] = (char)('0' + (fps / 10) % 10);
    text[6] = (char)('0' + fps % 10);
    text[7] = 0;

    unsigned char fg = brightest_palette_index(palette);
    for (int y = 2; y < 12; ++y) {
        for (int x = 2; x < 36; ++x) {
            pixels[y * DOOMGENERIC_RESX + x] = 0;
        }
    }

    int cursor = 4;
    for (const char *p = text; *p; ++p) {
        int glyph = 10;
        if (*p >= '0' && *p <= '9') glyph = *p - '0';
        else if (*p == 'F') glyph = 11;
        else if (*p == 'P') glyph = 12;
        else if (*p == 'S') glyph = 13;
        if (*p != ' ') {
            draw_indexed_glyph_3x5(pixels, cursor + 1, 5, glyph, 0);
            draw_indexed_glyph_3x5(pixels, cursor, 4, glyph, fg);
        }
        cursor += 4;
    }
}

static int current_viewport_rect(int *x, int *y, int *w, int *h) {
    int vx = viewwindowx;
    int vy = viewwindowy;
    int vw = scaledviewwidth;
    int vh = viewheight;
    if (vx < 0) vx = 0;
    if (vy < 0) vy = 0;
    if (vw <= 0 || vh <= 0) return 0;
    if (vx >= DOOMGENERIC_RESX || vy >= DOOMGENERIC_RESY) return 0;
    if (vx + vw > DOOMGENERIC_RESX) vw = DOOMGENERIC_RESX - vx;
    if (vy + vh > DOOMGENERIC_RESY) vh = DOOMGENERIC_RESY - vy;
    if (vw <= 0 || vh <= 0) return 0;
    *x = vx;
    *y = vy;
    *w = vw;
    *h = vh;
    return 1;
}

static void update_fullscreen_maps(int vx, int vy, int vw, int vh, int play_dst_h, int hud_dst_h) {
    const int hud_src_w = 320;
    const int hud_src_h = 32;
    const int hud_src_y = DOOMGENERIC_RESY - hud_src_h;
    if (fullscreen_map_x == vx
        && fullscreen_map_y == vy
        && fullscreen_map_w == vw
        && fullscreen_map_h == vh
        && fullscreen_map_play_h == play_dst_h
        && fullscreen_map_hud_h == hud_dst_h) {
        return;
    }
    fullscreen_map_x = vx;
    fullscreen_map_y = vy;
    fullscreen_map_w = vw;
    fullscreen_map_h = vh;
    fullscreen_map_play_h = play_dst_h;
    fullscreen_map_hud_h = hud_dst_h;
    for (int x = 0; x < DOOMGENERIC_RESX; ++x) {
        fullscreen_x_map[x] = (unsigned short)(vx + (x * vw) / DOOMGENERIC_RESX);
        fullscreen_hud_x_map[x] = (unsigned short)((x * hud_src_w) / DOOMGENERIC_RESX);
    }
    for (int y = 0; y < DOOMGENERIC_RESY; ++y) {
        if (y < play_dst_h) {
            fullscreen_y_map[y] = (unsigned short)(vy + (y * vh) / play_dst_h);
        } else {
            fullscreen_y_map[y] = (unsigned short)(hud_src_y + ((y - play_dst_h) * hud_src_h) / hud_dst_h);
        }
    }
}

static const unsigned char *fullscreen_indexed_frame(const unsigned char *pixels) {
    int vx, vy, vw, vh;
    if (!pixels || !current_viewport_rect(&vx, &vy, &vw, &vh)) return pixels;

    const int hud_src_h = 32;
    const int hud_dst_h = (DOOMGENERIC_RESY * hud_src_h) / 200;
    const int play_dst_h = DOOMGENERIC_RESY - hud_dst_h;
    update_fullscreen_maps(vx, vy, vw, vh, play_dst_h, hud_dst_h);

    for (int y = 0; y < DOOMGENERIC_RESY; ++y) {
        unsigned char *dst_row = fullscreen_indexed_buffer + y * DOOMGENERIC_RESX;
        int sy = fullscreen_y_map[y];
        const unsigned char *src_row = pixels + sy * DOOMGENERIC_RESX;
        if (y < play_dst_h) {
            for (int x = 0; x < DOOMGENERIC_RESX; ++x) {
                dst_row[x] = src_row[fullscreen_x_map[x]];
            }
        } else {
            for (int x = 0; x < DOOMGENERIC_RESX; ++x) {
                dst_row[x] = src_row[fullscreen_hud_x_map[x]];
            }
        }
    }
    return fullscreen_indexed_buffer;
}

static const pixel_t *fullscreen_rgba_frame(const pixel_t *pixels) {
    int vx, vy, vw, vh;
    if (!pixels || !current_viewport_rect(&vx, &vy, &vw, &vh)) return pixels;

    const int hud_src_h = 32;
    const int hud_dst_h = (DOOMGENERIC_RESY * hud_src_h) / 200;
    const int play_dst_h = DOOMGENERIC_RESY - hud_dst_h;
    update_fullscreen_maps(vx, vy, vw, vh, play_dst_h, hud_dst_h);

    for (int y = 0; y < DOOMGENERIC_RESY; ++y) {
        pixel_t *dst_row = fullscreen_rgba_buffer + y * DOOMGENERIC_RESX;
        int sy = fullscreen_y_map[y];
        const pixel_t *src_row = pixels + sy * DOOMGENERIC_RESX;
        if (y < play_dst_h) {
            for (int x = 0; x < DOOMGENERIC_RESX; ++x) {
                dst_row[x] = src_row[fullscreen_x_map[x]];
            }
        } else {
            for (int x = 0; x < DOOMGENERIC_RESX; ++x) {
                dst_row[x] = src_row[fullscreen_hud_x_map[x]];
            }
        }
    }
    return fullscreen_rgba_buffer;
}

void DG_Init(void) {
    frame_handle = os_frame_resource_create(DOOMGENERIC_RESX, DOOMGENERIC_RESY, 2, 60);
    log_s("[Skill][doomgeneric] original Doom renderer platform initialized.\n");
}

static void present_frame(void) {
    if (indexed_present_mode && last_indexed_pixels && last_indexed_palette && frame_handle) {
        os_frame_resource_submit_indexed8_seq(
            frame_handle,
            last_indexed_pixels,
            DOOMGENERIC_RESX,
            DOOMGENERIC_RESY,
            DOOMGENERIC_RESX,
            last_indexed_palette,
            256,
            render_sequence
        );
        return;
    }
    if (!DG_ScreenBuffer) return;
    const pixel_t *submit_buffer = fullscreen_rgba_frame(DG_ScreenBuffer);
    if (frame_handle) {
        os_frame_resource_submit_rgba_seq(
            frame_handle,
            (const unsigned char *)submit_buffer,
            DOOMGENERIC_RESX,
            DOOMGENERIC_RESY,
            DOOMGENERIC_RESX * 4,
            render_sequence
        );
    } else {
        os_gui_blit_rgba_frame((const unsigned char *)DG_ScreenBuffer, DOOMGENERIC_RESX, DOOMGENERIC_RESY);
    }
}

void DG_DrawFrame(void) {
    render_sequence++;
    if (!render_sequence) render_sequence = 1;
    present_frame();
    last_present_ms = os_now_ms();
}

void DG_DrawIndexedFrame(const unsigned char *pixels, const unsigned int *palette) {
    if (!pixels || !palette) return;
    indexed_present_mode = 1;
    last_indexed_pixels = fullscreen_indexed_frame(pixels);
    last_indexed_palette = palette;
    render_sequence++;
    if (!render_sequence) render_sequence = 1;
    last_present_ms = os_now_ms();
    present_frame();
}

void DG_SleepMs(uint32_t ms) { (void)ms; }
uint32_t DG_GetTicksMs(void) { return os_now_ms(); }
int DG_GetKey(int *pressed, unsigned char *key) {
    poll_hid();
    if (key_r == key_w) return 0;
    unsigned short data = key_queue[key_r++ & 31];
    *pressed = data >> 8;
    *key = data & 0xff;
    return 1;
}
void DG_SetWindowTitle(const char *title) { (void)title; }

static char arg0[] = "doomgeneric";
static char arg1[] = "-iwad";
static char arg2[] = "DOOM1.WAD";
static char arg3[] = "-skill";
static char arg4[] = "1";
static char arg5[] = "-width";
static char arg6[] = "640";
static char arg7[] = "-height";
static char arg8[] = "400";
static char arg9[] = "-nomusic";
static char arg10[] = "-nosound";
static char arg11[] = "-mb";
static char arg12[] = "4";
static char arg13[] = "-warp";
static char arg14[] = "1";
static char arg15[] = "1";
static char *argv[] = {
    arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7,
    arg8, arg9, arg10, arg11, arg12, arg13, arg14, arg15
};

static void apply_adaptive_quality(unsigned int now) {
    if (!frame_handle) return;
    if (last_quality_check_ms && now - last_quality_check_ms < DOOM_QUALITY_CHECK_MS) return;
    last_quality_check_ms = now;

    unsigned int fps = os_frame_resource_metric(frame_handle, FRAME_METRIC_FPS);
    unsigned int fresh_fps = os_frame_resource_metric(frame_handle, FRAME_METRIC_FRESH_FPS);
    unsigned int violations = os_frame_resource_metric(frame_handle, FRAME_METRIC_FLOOR_VIOLATIONS);
    if (!fps || !fresh_fps) return;

    if (violations || fps < DOOM_QUALITY_FLOOR_GUARD_FPS || fresh_fps < DOOM_QUALITY_FLOOR_GUARD_FPS) {
        quality_recover_since_ms = 0;
        if (adaptive_screenblocks > DOOM_QUALITY_MIN_SCREENBLOCKS) {
            adaptive_screenblocks--;
            screenblocks = adaptive_screenblocks;
            detailLevel = 1;
            R_SetViewSize(screenblocks, detailLevel);
            log_s("[Skill][doomgeneric] adaptive quality lowered to protect 60fps floor.\n");
        }
        return;
    }

    if (adaptive_screenblocks < DOOM_QUALITY_MAX_SCREENBLOCKS
        && fps >= DOOM_QUALITY_RECOVER_FPS
        && fresh_fps >= DOOM_QUALITY_RECOVER_FPS) {
        if (!quality_recover_since_ms) {
            quality_recover_since_ms = now;
            return;
        }
        if (now - quality_recover_since_ms >= DOOM_QUALITY_RECOVER_MS) {
            adaptive_screenblocks++;
            screenblocks = adaptive_screenblocks;
            detailLevel = 1;
            R_SetViewSize(screenblocks, detailLevel);
            quality_recover_since_ms = now;
            log_s("[Skill][doomgeneric] adaptive quality restored after sustained headroom.\n");
        }
    } else {
        quality_recover_since_ms = 0;
    }
}

static void tick_60hz(void) {
    if (os_gui_frame_close_requested()) {
        running = 0;
        return;
    }
    if (!running) return;
    unsigned int now = os_now_ms();
    apply_adaptive_quality(now);

    if (!last_game_tick_ms || now - last_game_tick_ms >= DOOM_GAME_TICK_MS) {
        doomgeneric_Tick();
        last_game_tick_ms = os_now_ms();
        if (frame_handle && last_indexed_pixels && last_indexed_palette) {
            present_frame();
            last_present_ms = os_now_ms();
        }
        return;
    }

    if (DG_ScreenBuffer && (!last_present_ms || now - last_present_ms >= DOOM_PRESENT_MIN_MS)) {
        last_present_ms = now;
        present_frame();
    }
}

__attribute__((export_name("init_driver")))
void init_driver(void) {
    if (!created) {
        created = 1;
        running = 1;
        doomgeneric_Create(16, argv);
        singletics = 1;
        adaptive_screenblocks = DOOM_QUALITY_MAX_SCREENBLOCKS;
        screenblocks = adaptive_screenblocks;
        detailLevel = 1;
        R_SetViewSize(screenblocks, detailLevel);
    }
}

__attribute__((export_name("run_skill")))
int run_skill(void) {
    init_driver();
    running = 1;
    last_game_tick_ms = 0;
    last_present_ms = 0;
    tick_60hz();
    return 1;
}

__attribute__((export_name("tick_skill")))
int tick_skill(void) {
    init_driver();
    tick_60hz();
    return 1;
}

__attribute__((export_name("frame_pump_target_hz")))
int frame_pump_target_hz(void) {
    return 1000;
}

__attribute__((export_name("frame_pump")))
int frame_pump(void) {
    init_driver();
    tick_60hz();
    return running ? 1 : 0;
}

__attribute__((export_name("poll_net")))
void poll_net(void) {
}
