#include "lib.h"
#include "string.h"
#include "io.h"

typedef struct {
    uint8_t* buf;
    size_t len;
} __attribute__((packed)) iovec;

typedef struct {
    struct {
        uint32_t pr_name_len;
    } __attribute__((packed)) dir;
    uint32_t _resv;
} __attribute__((packed)) prestat;

WASI_FUNC(uint16_t, fd_write, int fd, iovec* vecs, size_t len, size_t* numwritten);
WASI_FUNC(uint16_t, fd_read, int fd, iovec* vecs, size_t len, size_t* numread);
WASI_FUNC(uint16_t, path_open, int fd, int follow_redirects, char const* path, size_t pathlen, uint16_t oflags, uint64_t rights_base, uint64_t rights_inheriting, int fdlags, int* fd_out);
WASI_FUNC(uint16_t, fd_close, int fd);
WASI_FUNC(uint16_t, fd_prestat_get, int fd, prestat* prestat);
WASI_FUNC(uint16_t, fd_prestat_dir_name, int fd, char* path, size_t pathbuflen);

void write(int fd, char const* buf, size_t n) {
    iovec vec = (iovec) {
        .buf = (uint8_t*) buf,
        .len = n,
    };

    size_t nw;
    uint16_t err = fd_write(fd, &vec, 1, &nw);
    check_errno(err);
}

void writes(int fd, char const* str) {
    write(fd, str, strlen(str));
}

/** returns num actually written */
size_t read(int fd, char* buf, size_t n) {
    iovec vec = (iovec) {
        .buf = (uint8_t*) buf,
        .len = n,
    };

    size_t nr;
    uint16_t err = fd_read(fd, &vec, 1, &nr);
    check_errno(err);
    return nr;
}

#define OFLAG_CREAT       (0b0001)
#define OFLAG_REQUIRE_DIR (0b0010)
#define OFLAG_REQUIRE_NEW (0b0100)
#define OFLAG_TRUNC       (0b1000)

static int find_dir_fd() {
    static int dir_fd = 0;
    if (dir_fd != 0) return dir_fd;

    for (int fd = 3; fd < 32; fd++) {
        prestat prestat;
        uint16_t err = fd_prestat_get(fd, &prestat);
        if (!err) {
            char name[200];
            err = fd_prestat_dir_name(fd, name, prestat.dir.pr_name_len);
            write(STDOUT, name, prestat.dir.pr_name_len);
            name[prestat.dir.pr_name_len] = '\0';
            if (!strcmp(name, ".")) {
                dir_fd = fd;
                return fd;
            }
        }
    }

    dir_fd = 3;
    return dir_fd;
}

int open(char const* path, int fdflags, int rights) {
    int oflags = 0;
    if (fdflags & F_DIR)
        oflags |= OFLAG_REQUIRE_DIR;
    if (fdflags & F_TRUNC)
        oflags |= OFLAG_TRUNC;
    if (fdflags & F_CREATE)
        oflags |= OFLAG_CREAT;

    int dfd = find_dir_fd();

    int fd;
    uint16_t err = path_open(dfd, 1, path, strlen(path), oflags, rights, rights, fdflags & F__WASI_MASK, &fd);
    check_errno(err);
    return fd;
}

void close(int fd) {
    (void) fd_close(fd);
}

__attribute__((noreturn))
void _assert_fail(char const* cond, char const* loc) {
#ifdef NDEBUG
    writes(STDOUT, "assert fail ");
    writes(STDOUT, cond);
    exit(1);
#else
    writes(STDOUT, "\nAssertion failed at ");
    writes(STDOUT, loc);
    writes(STDOUT, ": ");
    writes(STDOUT, cond);
    writes(STDOUT, "\n");
    exit(1);
#endif
}

__attribute__((noreturn))
void _check_errno_fail(uint16_t errno) {
    writes(STDOUT, "\nWASI I/O Function Failed: ");
    writes(STDOUT, errstr(errno));
    writes(STDOUT, "\n");
    exit(1);
}

char* readToEnd(int fd, size_t* len_out) {
    size_t len = 0;

    size_t cap = 250;
    char* buf = malloc(cap);

    size_t read_iter;
    while ((read_iter = read(fd, buf + len, cap - len)))
    {
        len += read_iter;
        if (len == cap) {
            cap += 250;
            buf = realloc(buf, cap);
        }
    }

    if (len_out) *len_out = len;

    if (cap == len) {
        cap += 1;
        buf = realloc(buf, cap);
    }

    buf[len] = '\0';
    return buf;
}

