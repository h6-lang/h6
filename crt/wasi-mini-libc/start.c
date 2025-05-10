#include <stdint.h>
#include <stddef.h>
#include "lib.h"

__attribute__((weak))
void __wasm_call_ctors(void) {}
__attribute__((weak))
void __wasm_call_dtors(void) {}


WASI_FUNC(void, proc_exit, int);
WASI_FUNC(uint16_t, args_sizes_get, size_t* argc, size_t* argb);
WASI_FUNC(uint16_t, args_get, uint8_t** argv, uint8_t* buf);

__attribute__((export_name("_start")))
void _start(void) {
    // The linker synthesizes this to call constructors.
    __wasm_call_ctors();

    entry();

    // Call atexit functions, destructors, stdio cleanup, etc.
    __wasm_call_dtors();
}

__attribute__((noreturn))
void exit(int c) {
    proc_exit(c);
    __builtin_unreachable();
}

void args(int* argc_out, char*** argv_out) {
    size_t argc;
    size_t nb;
    args_sizes_get(&argc, &nb);
    *argc_out = argc;

    char* buf = malloc(nb);
    char** ptrs = malloc((argc + 1) * sizeof(char*));

    args_get((uint8_t**)ptrs, (uint8_t*) buf);
    ptrs[argc] = 0;

    *argv_out = ptrs;
}

static char const* errno_lut[] = {
    "success",
    "2big",
    "acces",
    "addrinuse",
    "addrnotavail",
    "afnosupport",
    "again",
    "already",
    "badf",
    "badmsg",
    "busy",
    "canceled",
    "child",
    "connaborted",
    "connrefused",
    "connreset",
    "deadlk",
    "destaddrreq",
    "dom",
    "dquot",
    "exist",
    "fault",
    "fbig",
    "hostunreach",
    "idrm",
    "ilseq",
    "inprogress",
    "intr",
    "inval",
    "io",
    "isconn",
    "isdir",
    "loop",
    "mfile",
    "mlink",
    "msgsize",
    "multihop",
    "nametoolong",
    "netdown",
    "netreset",
    "netunreach",
    "nfile",
    "nobufs",
    "nodev",
    "noent",
    "noexec",
    "nolck",
    "nolink",
    "nomem",
    "nomsg",
    "noprotoopt",
    "nospc",
    "nosys",
    "notconn",
    "notdir",
    "notempty",
    "notrecoverable",
    "notsock",
    "notsup",
    "notty",
    "nxio",
    "overflow",
    "ownerdead",
    "perm",
    "pipe",
    "proto",
    "protonosupport",
    "prototype",
    "range",
    "rofs",
    "spipe",
    "srch",
    "stale",
    "timedout",
    "txtbsy",
    "xdev",
    "notcapable",
};

char const* errstr(uint16_t errno) {
    if (errno > (sizeof(errno_lut) / sizeof(*errno_lut))) {
        return "???";
    }
    return errno_lut[errno];
}
