#ifndef OR_UNISTD_H
#define OR_UNISTD_H
int close(int fd);
int read(int fd, void *buf, unsigned int count);
int write(int fd, const void *buf, unsigned int count);
int lseek(int fd, int offset, int whence);
#endif
