#include "lib.h"

#define STDIN  (0)
#define STDOUT (1)
#define STDERR (2)

/** returns num actually written */
size_t read(int fd, char* buf, size_t n);

void write(int fd, char const* buf, size_t n);
void writes(int fd, char const* str);

#define F_APPEND   (1 << 0)
#define F_DSYNC    (1 << 1)
#define F_NONBLOCK (1 << 2)
#define F_RSYNC    (1 << 3)
#define F_SYNC     (1 << 4)
#define F_CREATE   (1 << 5) // first non wasi
#define F__WASI_MASK (F_CREATE - 1)
#define F_DIR      (1 << 6)
#define F_TRUNC    (1 << 7)

#define RIGHT_DSYNC (1 << 0)
#define RIGHT_READ  (1 << 1)
#define RIGHT_SEEK  (1 << 2)
#define RIGHT_STAT_SET_FLAGS (1 << 3)
#define RIGHT_SYNC  (1 << 4)
#define RIGHT_TELL  (1 << 5)
#define RIGHT_WRITE (1 << 6)

int open(char const* path, int fdflags, int rights);

void close(int fd);

/** result needs to be free()-ed */
char* readToEnd(int fd, size_t* len_out);
