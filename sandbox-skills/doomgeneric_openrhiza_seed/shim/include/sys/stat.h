#ifndef OR_SYS_STAT_H
#define OR_SYS_STAT_H
struct stat { unsigned int st_size; };
int mkdir(const char *path, int mode);
#endif
