#include <stddef.h>
#include <stdint.h>

#define PAGESIZE (64*1024)

#define WASI_FUNC(ty, name, ...) \
    __attribute__((import_module("wasi_snapshot_preview1"), import_name(#name))) ty name(__VA_ARGS__);

__attribute__((noreturn))
void _assert_fail(char const* cond, char const* loc);

__attribute__((noreturn))
void _check_errno_fail(uint16_t errno);

#define assert__p(x) #x
#define assert__p2(x) assert__p(x)
#define assert(cond) if (!(cond)) { _assert_fail(#cond, __FILE__ ":" assert__p2(__LINE__)); }

#ifdef NDEBUG
# define debug_assert(cond) if (!(cond)) { __builtin_unreachable(); }
# define check_errno(errno) if (errno) { exit(errno); }
#else
# define debug_assert(...) assert(__VA_ARGS__)
# define check_errno(errno) if (errno) { _check_errno_fail(errno); }
#endif

#define static_assert(expr) __attribute__((unused)) static char __static_assert__##__LINE__[((size_t)(expr))-1];

__attribute__((noreturn))
void exit(int);

void entry(void);
void args(int* argc_out, char*** argv_out);

void* malloc(size_t num);
void free(void* ptr);
void* realloc(void* ptr, size_t newnum);

char const* errstr(uint16_t errno);
