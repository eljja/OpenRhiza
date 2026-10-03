#ifndef OR_STDLIB_H
#define OR_STDLIB_H
#include <stddef.h>
void *malloc(size_t size);
void *calloc(size_t count, size_t size);
void *realloc(void *ptr, size_t size);
void free(void *ptr);
int atoi(const char *s);
double atof(const char *s);
int abs(int n);
void exit(int code);
void abort(void);
void qsort(void *base, size_t nmemb, size_t size, int (*compar)(const void *, const void *));
int system(const char *command);
#endif
