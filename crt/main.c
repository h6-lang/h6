#include "rt.h"

#ifdef CUSTOM_WASI_LIB
# include "wasi-mini-libc/io.h"
# include "wasi-mini-libc/lib.h"
# include "wasi-mini-libc/string.h"
#else
# include <assert.h>
# include <stdio.h>
# include <stdlib.h>
# include <string.h>
#endif

void syscallback(h6_rt_t* rt, uint32_t id, void* userptr)
{
    switch (id)
    {
    case 0: {
        int32_t byte = h6_heap_arr_pop_num(rt->stack);
        int32_t stream = h6_heap_arr_pop_num(rt->stack);
        assert(stream == 1);

#ifdef CUSTOM_WASI_LIB
        char str[] = {byte};
        write(STDOUT, str, 1);
#else
        fputc(byte, stdout);
#endif
    } break;

    case 1: {
        int32_t stream = h6_heap_arr_pop_num(rt->stack);
        assert(stream == 1);
        int32_t byte;
#ifdef CUSTOM_WASI_LIB
        char str[1];
        read(STDIN, str, 1);
        byte = str[0];
#else
        byte = fgetc(stdin);
#endif
        h6_heap_arr_push_num(rt->stack, byte);
    } break;

    default:
        assert(0 && "unknwon syscall");
        break;
    }
}

char* read_bytecode(char const* path)
{
#ifdef CUSTOM_WASI_LIB
    int fd = open(path, 0, RIGHT_READ);
    char* bytecode = readToEnd(fd, NULL);
    close(fd);
#else
    FILE* fp = fopen(path, "rb");
    assert(fp);
    fseek(fp, 0, SEEK_END);
    size_t len = ftell(fp);
    rewind(fp);
    char* bytecode = malloc(len);
    assert(bytecode);
    fread(bytecode, 1, len, fp);
    fclose(fp);
#endif
    return bytecode;
}

#ifdef CUSTOM_WASI_LIB
void entry() {
    int argc;
    char** argv;
    args(&argc, &argv);
#define RETURN(n) if (n == 0) { return; } else { exit(n); }
#else
int main(int argc, char** argv) {
#define RETURN(n) return n
#endif

    char const* arg_inp_file = NULL;
    char const* dso_file = NULL;

    ++ argv;
    for (; *argv; ++argv)
    {
        if (!strcmp(*argv, "--dso")) {
            ++argv;
            dso_file = *argv;
        }
        else if (!strcmp(*argv, "--help")) {
            char const* helpstr =
                     "h6crt [input h6b file]"
                "\n" " options:"
                "\n" "   --dso [path] \tload dso bytecode"
                "\n" "   --help"
                "\n";
#ifdef CUSTOM_WASI_LIB
            writes(STDOUT, helpstr);
#else
            printf("%s", helpstr);
#endif
            RETURN(0);
        }
        else {
            arg_inp_file = *argv;
        }
    }

    assert(arg_inp_file);
    char* bytecode = read_bytecode(arg_inp_file);

    h6_rt_t rt = h6_mk_rt(bytecode, syscallback, NULL);

    if (dso_file) {
        char* by = read_bytecode(dso_file);
        h6_set_dso(&rt, by);
    }

    h6_run_bytecode(&rt, bytecode);

    if ( h6_heap_arr_len(rt.stack) > 0 )
    {
#ifdef CUSTOM_WASI_LIB
        writes(STDOUT, "BOT\n");
        for (size_t i = 0; i < h6_heap_arr_len(rt.stack); i ++) {
            h6_op* v = h6_heap_arr_get_op(rt.stack, i);

            writes(STDOUT, "  ");
            h6_op_print(STDOUT, v);
            writes(STDOUT, "\n");
        }
        writes(STDOUT, "TOP\n");
#else
        printf("BOT\n");
        for (size_t i = 0; i < h6_heap_arr_len(rt.stack); i ++) {
            h6_op* v = h6_heap_arr_get_op(rt.stack, i);

            putc(' ', stdout);
            putc(' ', stdout);
            h6_op_print(stdout, v);
            putc('\n', stdout);
        }
        printf("TOP\n");
#endif
    }
}
