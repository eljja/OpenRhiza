#include "w_file.h"
#include "z_zone.h"
#include <string.h>

__attribute__((import_module("env"), import_name("os_file_size_root_8_3")))
unsigned int os_file_size_root_8_3(const unsigned char *name);
__attribute__((import_module("env"), import_name("os_file_read_root_8_3")))
unsigned int os_file_read_root_8_3(const unsigned char *name, unsigned int offset, unsigned char *ptr, unsigned int max_len);

typedef struct {
    wad_file_t wad;
    unsigned char name[11];
} openrhiza_wad_file_t;

extern wad_file_class_t stdc_wad_file;

static void fat_name_for_path(const char *path, unsigned char out[11]) {
    const char *base = path;
    for (const char *p = path; *p; ++p) if (*p == '/' || *p == '\\') base = p + 1;
    for (int i = 0; i < 11; ++i) out[i] = ' ';
    int i = 0;
    while (*base && *base != '.' && i < 8) {
        char c = *base++;
        out[i++] = c >= 'a' && c <= 'z' ? c - 32 : c;
    }
    if (*base == '.') ++base;
    i = 8;
    while (*base && i < 11) {
        char c = *base++;
        out[i++] = c >= 'a' && c <= 'z' ? c - 32 : c;
    }
}

static wad_file_t *OR_OpenFile(char *path) {
    unsigned char name[11];
    fat_name_for_path(path, name);
    unsigned int len = os_file_size_root_8_3(name);
    if (!len) return 0;
    openrhiza_wad_file_t *result = Z_Malloc(sizeof(openrhiza_wad_file_t), PU_STATIC, 0);
    result->wad.file_class = &stdc_wad_file;
    result->wad.mapped = 0;
    result->wad.length = len;
    memcpy(result->name, name, 11);
    return &result->wad;
}

static void OR_CloseFile(wad_file_t *wad) {
    Z_Free(wad);
}

static size_t OR_Read(wad_file_t *wad, unsigned int offset, void *buffer, size_t buffer_len) {
    openrhiza_wad_file_t *f = (openrhiza_wad_file_t *)wad;
    unsigned char *out = buffer;
    size_t done = 0;
    while (done < buffer_len) {
        unsigned int chunk = (unsigned int)((buffer_len - done) > 4096 ? 4096 : (buffer_len - done));
        unsigned int got = 0;
        for (int attempt = 0; attempt < 4 && !got; ++attempt) {
            got = os_file_read_root_8_3(f->name, offset + (unsigned int)done, out + done, chunk);
        }
        if (!got) break;
        done += got;
    }
    return done;
}

wad_file_class_t stdc_wad_file = { OR_OpenFile, OR_CloseFile, OR_Read };
