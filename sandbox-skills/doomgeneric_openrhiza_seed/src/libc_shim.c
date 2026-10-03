#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

int errno;
struct OR_FILE { int kind; int pos; unsigned int size; unsigned char name[11]; };
static struct OR_FILE or_stdout = {1, 0}, or_stderr = {2, 0}, or_stdin = {0, 0};
FILE *stdout = &or_stdout;
FILE *stderr = &or_stderr;
FILE *stdin = &or_stdin;

__attribute__((import_module("env"), import_name("os_log")))
void os_log(const unsigned char *ptr, unsigned int len);
__attribute__((import_module("env"), import_name("os_file_size_root_8_3")))
unsigned int os_file_size_root_8_3(const unsigned char *name);
__attribute__((import_module("env"), import_name("os_file_read_root_8_3")))
unsigned int os_file_read_root_8_3(const unsigned char *name, unsigned int offset, unsigned char *ptr, unsigned int max_len);

static unsigned char heap[12 * 1024 * 1024] __attribute__((aligned(16)));
static size_t heap_used;

void *malloc(size_t size) {
    size_t total = ((size + 16 + 15) & ~(size_t)15);
    if (heap_used + total > sizeof(heap)) return 0;
    size_t *header = (size_t *)(heap + heap_used);
    *header = size;
    heap_used += total;
    return (void *)((unsigned char *)header + 16);
}

void free(void *ptr) { (void)ptr; }

void *calloc(size_t count, size_t size) {
    size_t total = count * size;
    unsigned char *p = malloc(total);
    if (!p) return 0;
    for (size_t i = 0; i < total; ++i) p[i] = 0;
    return p;
}

void *realloc(void *ptr, size_t size) {
    void *p = malloc(size);
    if (p && ptr) {
        size_t old_size = *(size_t *)((unsigned char *)ptr - 16);
        size_t copy = old_size < size ? old_size : size;
        for (size_t i = 0; i < copy; ++i) ((unsigned char *)p)[i] = ((unsigned char *)ptr)[i];
    }
    return p;
}

void *memcpy(void *dst, const void *src, size_t n) {
    unsigned char *d = dst; const unsigned char *s = src;
    for (size_t i = 0; i < n; ++i) d[i] = s[i];
    return dst;
}

void *memmove(void *dst, const void *src, size_t n) {
    unsigned char *d = dst; const unsigned char *s = src;
    if (d < s) for (size_t i = 0; i < n; ++i) d[i] = s[i];
    else while (n--) d[n] = s[n];
    return dst;
}

void *memset(void *dst, int c, size_t n) {
    unsigned char *d = dst;
    for (size_t i = 0; i < n; ++i) d[i] = (unsigned char)c;
    return dst;
}

int memcmp(const void *a, const void *b, size_t n) {
    const unsigned char *x = a, *y = b;
    for (size_t i = 0; i < n; ++i) if (x[i] != y[i]) return x[i] - y[i];
    return 0;
}

size_t strlen(const char *s) { size_t n = 0; while (s && s[n]) ++n; return n; }
int strcmp(const char *a, const char *b) { while (*a && *a == *b) { ++a; ++b; } return (unsigned char)*a - (unsigned char)*b; }
int strncmp(const char *a, const char *b, size_t n) { for (size_t i=0;i<n;i++){ if(a[i]!=b[i]||!a[i]||!b[i]) return (unsigned char)a[i]-(unsigned char)b[i]; } return 0; }
char *strcpy(char *d, const char *s) { char *r=d; while ((*d++=*s++)); return r; }
char *strncpy(char *d, const char *s, size_t n) { size_t i=0; for(;i<n&&s[i];i++) d[i]=s[i]; for(;i<n;i++) d[i]=0; return d; }
char *strcat(char *d, const char *s) { strcpy(d + strlen(d), s); return d; }
char *strchr(const char *s, int c) { while (*s) { if (*s == (char)c) return (char*)s; ++s; } return c == 0 ? (char*)s : 0; }
char *strrchr(const char *s, int c) { const char *r=0; do { if (*s == (char)c) r=s; } while (*s++); return (char*)r; }
char *strstr(const char *h, const char *n) { size_t nl=strlen(n); if(!nl) return (char*)h; for(;*h;h++) if(!strncmp(h,n,nl)) return (char*)h; return 0; }
char *strdup(const char *s) { size_t n = strlen(s)+1; char *p = malloc(n); if (p) memcpy(p,s,n); return p; }

int tolower(int c) { return c >= 'A' && c <= 'Z' ? c + 32 : c; }
int toupper(int c) { return c >= 'a' && c <= 'z' ? c - 32 : c; }
int isspace(int c) { return c==' '||c=='\t'||c=='\n'||c=='\r'||c=='\f'||c=='\v'; }
int isdigit(int c) { return c>='0'&&c<='9'; }
int isalpha(int c) { return (c>='A'&&c<='Z')||(c>='a'&&c<='z'); }
int isalnum(int c) { return isalpha(c)||isdigit(c); }
int strcasecmp(const char *a, const char *b) { while (*a && tolower(*a)==tolower(*b)) {++a;++b;} return tolower(*a)-tolower(*b); }
int strncasecmp(const char *a, const char *b, size_t n) { for(size_t i=0;i<n;i++){int d=tolower(a[i])-tolower(b[i]); if(d||!a[i]||!b[i]) return d;} return 0; }

int atoi(const char *s) { int sign=1,v=0; while(isspace(*s))++s; if(*s=='-'){sign=-1;++s;} while(isdigit(*s)){v=v*10+*s-'0';++s;} return sign*v; }
double atof(const char *s) { return (double)atoi(s); }
int abs(int n) { return n < 0 ? -n : n; }
double fabs(double x) { return x < 0 ? -x : x; }
double sqrt(double x) { double r=x>1?x:1; for(int i=0;i<16;i++) r=(r+x/r)/2; return r; }

static void out_char(char **out, size_t *left, char c) {
    if (*out && *left > 1) { **out = c; (*out)++; (*left)--; }
}

static void out_str(char **out, size_t *left, const char *s) { while (s && *s) out_char(out,left,*s++); }

static void out_num(char **out, size_t *left, long long v, int base, int sign) {
    char buf[32]; int i=0; unsigned long long u;
    if (sign && v < 0) { out_char(out,left,'-'); u=(unsigned long long)(-v); } else u=(unsigned long long)v;
    do { int d = u % base; buf[i++] = d < 10 ? '0'+d : 'a'+d-10; u /= base; } while (u && i < 31);
    while (i--) out_char(out,left,buf[i]);
}

static void out_num_fmt(char **out, size_t *left, long long v, int base, int sign, int width, int precision, int zero_pad) {
    char buf[32]; int i=0; unsigned long long u; int neg = sign && v < 0;
    if (neg) u = (unsigned long long)(-v); else u = (unsigned long long)v;
    do { int d = u % base; buf[i++] = d < 10 ? '0'+d : 'a'+d-10; u /= base; } while (u && i < 31);
    int digits = i;
    int zeros = precision > digits ? precision - digits : 0;
    int total = digits + zeros + (neg ? 1 : 0);
    char pad = zero_pad && precision < 0 ? '0' : ' ';
    while (width-- > total) out_char(out, left, pad);
    if (neg) out_char(out, left, '-');
    while (zeros-- > 0) out_char(out, left, '0');
    while (i--) out_char(out,left,buf[i]);
}

int vsnprintf(char *out, size_t n, const char *fmt, va_list ap) {
    char *p = out; size_t left = n ? n : 0; int count = 0;
    for (; *fmt; ++fmt) {
        if (*fmt != '%') { out_char(&p,&left,*fmt); count++; continue; }
        ++fmt;
        int zero_pad = 0, width = 0, precision = -1;
        if (*fmt == '0') { zero_pad = 1; ++fmt; }
        while (*fmt=='-'||*fmt==' ') ++fmt;
        while (*fmt>='0'&&*fmt<='9') { width = width * 10 + (*fmt - '0'); ++fmt; }
        if (*fmt == '.') {
            precision = 0;
            ++fmt;
            while (*fmt>='0'&&*fmt<='9') { precision = precision * 10 + (*fmt - '0'); ++fmt; }
        }
        if (*fmt=='l') ++fmt;
        switch (*fmt) {
            case 's': { char *s=va_arg(ap,char*); out_str(&p,&left,s?s:"(null)"); break; }
            case 'c': out_char(&p,&left,(char)va_arg(ap,int)); break;
            case 'd': case 'i': out_num_fmt(&p,&left,va_arg(ap,int),10,1,width,precision,zero_pad); break;
            case 'u': out_num_fmt(&p,&left,va_arg(ap,unsigned int),10,0,width,precision,zero_pad); break;
            case 'x': case 'X': out_num_fmt(&p,&left,va_arg(ap,unsigned int),16,0,width,precision,zero_pad); break;
            case 'p': out_num_fmt(&p,&left,(uintptr_t)va_arg(ap,void*),16,0,width,precision,zero_pad); break;
            case '%': out_char(&p,&left,'%'); break;
            default: out_char(&p,&left,*fmt); break;
        }
    }
    if (out && n) *p = 0;
    return count;
}

int snprintf(char *out, size_t n, const char *fmt, ...) { va_list ap; va_start(ap,fmt); int r=vsnprintf(out,n,fmt,ap); va_end(ap); return r; }
int sprintf(char *out, const char *fmt, ...) { va_list ap; va_start(ap,fmt); int r=vsnprintf(out,(size_t)-1,fmt,ap); va_end(ap); return r; }
int vprintf(const char *fmt, va_list ap) { char b[256]; int r=vsnprintf(b,sizeof(b),fmt,ap); os_log((const unsigned char*)b, strlen(b)); return r; }
int vfprintf(FILE *stream, const char *fmt, va_list ap) { (void)stream; return vprintf(fmt, ap); }
int printf(const char *fmt, ...) { va_list ap; va_start(ap,fmt); int r=vprintf(fmt,ap); va_end(ap); return r; }
int fprintf(FILE *stream, const char *fmt, ...) { (void)stream; va_list ap; va_start(ap,fmt); int r=vprintf(fmt,ap); va_end(ap); return r; }
int puts(const char *s) { os_log((const unsigned char*)s, strlen(s)); os_log((const unsigned char*)"\n",1); return 0; }
int putchar(int c) { unsigned char b=(unsigned char)c; os_log(&b,1); return c; }

int sscanf(const char *s, const char *fmt, ...) { (void)fmt; va_list ap; va_start(ap,fmt); int *out=va_arg(ap,int*); if(out) *out=atoi(s); va_end(ap); return 1; }

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

FILE *fopen(const char *path, const char *mode) {
    if (!mode || mode[0] != 'r') return 0;
    struct OR_FILE *f = malloc(sizeof(struct OR_FILE));
    if (!f) return 0;
    f->kind = 3;
    f->pos = 0;
    fat_name_for_path(path, f->name);
    f->size = os_file_size_root_8_3(f->name);
    if (!f->size) return 0;
    return f;
}

int fclose(FILE *f) { (void)f; return 0; }
size_t fread(void *ptr, size_t size, size_t nmemb, FILE *f) {
    if (!f || f->kind != 3 || !size) return 0;
    unsigned int want = (unsigned int)(size * nmemb);
    if (f->pos >= (int)f->size) return 0;
    if ((unsigned int)f->pos + want > f->size) want = f->size - (unsigned int)f->pos;
    unsigned int got = os_file_read_root_8_3(f->name, (unsigned int)f->pos, ptr, want);
    f->pos += (int)got;
    return got / size;
}
size_t fwrite(const void *ptr, size_t size, size_t nmemb, FILE *f) { (void)ptr;(void)f; return size*nmemb; }
int fseek(FILE *f, long offset, int whence) {
    if (!f || f->kind != 3) return -1;
    long pos = whence == SEEK_SET ? offset : (whence == SEEK_CUR ? f->pos + offset : (long)f->size + offset);
    if (pos < 0) pos = 0;
    if (pos > (long)f->size) pos = f->size;
    f->pos = (int)pos;
    return 0;
}
long ftell(FILE *f) { return f && f->kind == 3 ? f->pos : 0; }
int fflush(FILE *f) { (void)f; return 0; }
int remove(const char *path) { (void)path; return -1; }
int rename(const char *oldpath, const char *newpath) { (void)oldpath;(void)newpath; return -1; }
int mkdir(const char *path, int mode) { (void)path; (void)mode; return 0; }
int close(int fd) { (void)fd; return 0; }
int read(int fd, void *buf, unsigned int count) { (void)fd;(void)buf;(void)count; return 0; }
int write(int fd, const void *buf, unsigned int count) { (void)fd;(void)buf; return (int)count; }
int lseek(int fd, int offset, int whence) { (void)fd;(void)whence; return offset; }
long time(long *out) { if(out) *out=0; return 0; }
void qsort(void *base, size_t nmemb, size_t size, int (*compar)(const void *, const void *)) { (void)base;(void)nmemb;(void)size;(void)compar; }
int system(const char *command) { (void)command; return -1; }
void exit(int code) { (void)code; for(;;){} }
void abort(void) { for(;;){} }
