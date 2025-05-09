#include <assert.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "rt.h"

enum op_kind {
    Terminate = 0,
    Const = 2,
    TypeId = 3,
    Push = 8,

    Add = 9,
    Sub = 10,
    Mul = 11,
    Dup = 12,
    Swap = 14,
    Pop = 15,
    Exec = 16,
    Select = 17,
    Lt = 18,
    Gt = 19,
    Eq = 20,
    Not = 21,
    RoL = 22,
    RoR = 24,
    Reach = 25,

    ArrBegin = 26,
    ArrEnd = 27,
    ArrCat = 29,
    ArrFirst = 30,
    ArrLen = 31,
    ArrSkip1 = 32,
    Pack = 33,
    Mod = 34,
    Div = 36,

    System = 41,
    Materialize = 42,
    OpsOf = 43,
    ConstAt = 44,

    ConstDso = 45,

    U8ArrAt = 46,
    I16ArrAt = 47,

    CustomPushArr = 100,
};

static
int op_has_arg(enum op_kind op) {
    switch (op) {
        case Const:
        case Push:
        case Reach:
        case System:
        case ConstDso:
        case U8ArrAt:
        case I16ArrAt:
            return 1;

        default:
            return 0;
    }
}

typedef struct h6_heap_arr heap_arr;

struct h6_op {
  enum op_kind kind : 8;
  union {
    int32_t  num;
    uint32_t uint;
  } __attribute__((packed)) arg;
  union {
      heap_arr* push_arr;
  } custom;
};

typedef h6_op op;

struct h6_heap_arr {
  op* items;
  size_t items_len;
  int rc;
};

void h6_op_print(FILE* out, h6_op *o) {
    switch (o->kind)
    {
    case Push:
        fprintf(out, "%i", o->arg.num);
        break;

    case CustomPushArr: {
        fputc('{', out);
        fputc(' ', out);
        for (size_t i = 0; i < o->custom.push_arr->items_len; i ++) {
            op* item = &o->custom.push_arr->items[i];
            h6_op_print(out, item);
            fputc(' ', out);
        }
        fputc('}', out);
    } break;

    default: {
        fprintf(out, "<op %i>", o->kind);
    } break;
    }
}

static
void op_destr(op o);

void h6_heap_arr_destr(h6_heap_arr* arr) {
    if (!(arr->rc --)) {
        for (size_t i = 0; i < arr->items_len; i ++) {
            op_destr(arr->items[i]);
        }
        free(arr->items);
        free(arr);
    }
}

h6_heap_arr* h6_heap_arr_mk() {
    heap_arr* a = malloc(sizeof(heap_arr));
    a->rc = 1;
    a->items_len = 0;
    a->items = NULL;
    return a;
}

#define h6_heap_arr_mk_opt_u8 h6_heap_arr_mk

size_t h6_heap_arr_len(h6_heap_arr* arr) {
    return arr->items_len;
}

static
void heap_arr_append(heap_arr* arr, heap_arr* append) {
    arr->items = realloc(arr->items, sizeof(op) * (arr->items_len + append->items_len));
    memcpy(&arr->items[arr->items_len], append->items, sizeof(op) * append->items_len);
    arr->items_len += append->items_len;
}

static
void heap_arr_push(heap_arr* arr, op o) {
    heap_arr other;
    other.items_len = 1;
    other.items = &o;
    heap_arr_append(arr, &other);
}

static
op heap_arr_pop(heap_arr* arr) {
    assert(arr->items_len);
    return arr->items[-- arr->items_len];
}

static
op heap_arr_last(heap_arr* arr) {
    assert(arr->items_len);
    return arr->items[arr->items_len - 1];
}

static
op heap_arr_popfront(heap_arr* arr) {
    assert(arr->items_len);
    op v = arr->items[0];
    -- arr->items_len;
    memcpy(arr->items, arr->items + 1, arr->items_len * sizeof(op));
    return v;
}

static
heap_arr* read_const(char* opp) {
    heap_arr* out = h6_heap_arr_mk();

    for (;;) {
        op found;

        enum op_kind kind = opp[0];
        opp ++;
        found.kind = kind;
        if ( op_has_arg(kind) ) {
            memcpy(&found.arg, opp, 4);
            opp += 4;
        }

        if (kind == Terminate)
            break;

        heap_arr_push(out, found);
    }

    return out;
}

static op op_dup(op o);

static
heap_arr* heap_arr_cow(heap_arr* arr) {
    if (arr->rc > 1) {
        heap_arr* new = h6_heap_arr_mk();
        for (size_t i = 0; i < arr->items_len; i ++) {
            op val = arr->items[i];
            val = op_dup(val);
            heap_arr_push(new, val);
        }
        return new;
    }
    return arr;
}

__attribute__((always_inline))
static
op mk_push(int32_t v) {
    op o;
    o.kind = Push;
    o.arg.num = v;
    return o;
}

void h6_heap_arr_push_num(h6_heap_arr* arr, int32_t num) {
    heap_arr_push(arr, mk_push(num));
}

void h6_heap_arr_push_box_arr(h6_heap_arr* arr, h6_heap_arr* other) {
    ++ other->rc;
    op o; o.kind = CustomPushArr; o.custom.push_arr = other;
    heap_arr_push(arr, o);
}

static void op_destr(op o) {
    if (o.kind == CustomPushArr) {
        h6_heap_arr_destr(o.custom.push_arr);
    }
}

static op op_dup(op o) {
    if (o.kind == CustomPushArr) {
        ++ o.custom.push_arr->rc;
    }
    return o;
}

static int32_t as_int(op o) {
    assert(o.kind == Push);
    return o.arg.num;
}

static void run_arr(h6_rt_t* rt, heap_arr* ops);

static void run_op(h6_rt_t* rt, op o) {
    if (o.kind == ArrBegin) {
        if (rt->ind == 0) {
            rt->building_arr = h6_heap_arr_mk();
        } else {
            heap_arr_push(rt->building_arr, o);
        }
        rt->ind ++;
        return;
    }
    else if (o.kind == ArrEnd) {
        rt->ind --;
        if (rt->ind == 0) {
            heap_arr_push(rt->stack, (op) {
                .kind = CustomPushArr,
                .custom.push_arr = rt->building_arr,
            });
            rt->building_arr = NULL;
        } else {
            heap_arr_push(rt->building_arr, o);
        }
        return;
    }

    if (rt->ind > 0) {
        heap_arr_push(rt->building_arr, o);
    }
    else {
        switch (o.kind)
        {
        case ArrBegin:
        case ArrEnd:
        case Terminate:
            assert(0 && "unreachable");
            break;

        case Const: {
            heap_arr* arr = read_const(&rt->bytecode[16 + o.arg.uint]);
            run_arr(rt, arr);
            h6_heap_arr_destr(arr);
        } break;

        case U8ArrAt:
        case I16ArrAt: {
            char* bptr = &rt->bytecode[16 + o.arg.uint];
            uint16_t len = *(uint16_t*)bptr;
            char* arrp = bptr + 2;
            heap_arr* out = h6_heap_arr_mk_opt_u8();

            switch (o.kind) {
                case U8ArrAt: {
                    for (uint16_t i = 0; i < len; i ++) {
                        int32_t v = ((uint8_t*)arrp)[i];
                        h6_heap_arr_push_num(out, v);
                    }
                } break;

                case I16ArrAt: {
                    for (uint16_t i = 0; i < len; i ++) {
                        int32_t v = ((uint16_t*)arrp)[i];
                        h6_heap_arr_push_num(out, v);
                    }
                } break;

                default: break;
            }

            h6_heap_arr_push_box_arr(rt->stack, out);
            h6_heap_arr_destr(out);
        } break;

        case ConstDso: {
            assert(o.arg.uint < rt->resolved_dso_len);
            char* ptr = &rt->dso_by[rt->resolved_dso_abs_off[o.arg.uint]];
            heap_arr* arr = read_const(ptr);
            run_arr(rt, arr);
            h6_heap_arr_destr(arr);
        } break;

        case Push:
        case CustomPushArr:
        {
            heap_arr_push(rt->stack, o);
        } break;

        case Add: case Sub: case Mul: case Div: case Mod:
        case Lt: case Gt: case Eq:
        {
            int32_t b = as_int(heap_arr_pop(rt->stack));
            int32_t a = as_int(heap_arr_pop(rt->stack));

            int32_t res;
            switch (o.kind) {
                case Add: res = a + b; break;
                case Sub: res = a - b; break;
                case Mul: res = a * b; break;
                case Div: res = a / b; break;
                case Mod: res = ((int)a) % (int)b; break;
                case Lt: res = a < b; break;
                case Gt: res = a > b; break;
                case Eq: res = a == b; break;
                default: break;
            }

            heap_arr_push(rt->stack, mk_push(res));
        } break;

        case Not: {
            assert(rt->stack->items_len);
            assert(rt->stack->items[0].kind == Push);
            rt->stack->items[0].arg.num = !rt->stack->items[0].arg.num;
        } break;

        case Dup: {
            op v = heap_arr_last(rt->stack);
            heap_arr_push(rt->stack, op_dup(v));
        } break;

        case Swap: {
            op b = heap_arr_pop(rt->stack);
            op a = heap_arr_pop(rt->stack);
            heap_arr_push(rt->stack, b);
            heap_arr_push(rt->stack, a);
        } break;

        case Pop: {
            op v = heap_arr_pop(rt->stack);
            op_destr(v);
        } break;

        case Exec: {
            op a = heap_arr_pop(rt->stack);
            assert(a.kind == CustomPushArr);
            h6_heap_arr_destr(a.custom.push_arr);
            run_arr(rt, a.custom.push_arr);
        } break;

        case Select: {
            int cond = as_int(heap_arr_pop(rt->stack));
            op a = heap_arr_pop(rt->stack);
            op b = heap_arr_pop(rt->stack);
            op v = cond ? a : b;
            op notv = cond ? b : a;
            op_destr(notv);
            heap_arr_push(rt->stack, v);
        } break;

        case RoL: {
            op t0 = heap_arr_pop(rt->stack);
            op t1 = heap_arr_pop(rt->stack);
            op t2 = heap_arr_pop(rt->stack);
            heap_arr_push(rt->stack, t1);
            heap_arr_push(rt->stack, t0);
            heap_arr_push(rt->stack, t2);
        } break;

        case RoR: {
            op t0 = heap_arr_pop(rt->stack);
            op t1 = heap_arr_pop(rt->stack);
            op t2 = heap_arr_pop(rt->stack);
            heap_arr_push(rt->stack, t0);
            heap_arr_push(rt->stack, t2);
            heap_arr_push(rt->stack, t1);
        } break;

        case Reach: {
            assert(o.arg.uint < rt->stack->items_len);
            op v = rt->stack->items[rt->stack->items_len - o.arg.uint - 1];
            v = op_dup(v);
            heap_arr_push(rt->stack, v);
        } break;

        case ArrCat: {
            op b = heap_arr_pop(rt->stack);
            assert(b.kind == CustomPushArr);
            op a = heap_arr_pop(rt->stack);
            assert(a.kind == CustomPushArr);
            a.custom.push_arr = heap_arr_cow(a.custom.push_arr);
            heap_arr_append(a.custom.push_arr, b.custom.push_arr);
            h6_heap_arr_destr(b.custom.push_arr);
            heap_arr_push(rt->stack, a);
        } break;

        case ArrFirst: {
            op a = heap_arr_pop(rt->stack);
            assert(a.kind == CustomPushArr);
            op first = heap_arr_popfront(a.custom.push_arr);
            h6_heap_arr_destr(a.custom.push_arr);
            heap_arr_push(rt->stack, first);
        } break;

        case ArrLen: {
            op a = heap_arr_pop(rt->stack);
            heap_arr_push(rt->stack, mk_push(a.custom.push_arr->items_len));
            h6_heap_arr_destr(a.custom.push_arr);
        } break;

        case ArrSkip1: {
            op a = heap_arr_pop(rt->stack);
            assert(a.kind == CustomPushArr);
            a.custom.push_arr = heap_arr_cow(a.custom.push_arr);
            op first = heap_arr_popfront(a.custom.push_arr);
            op_destr(first);
            heap_arr_push(rt->stack, a);
        } break;

        case Pack: {
            op v = heap_arr_pop(rt->stack);
            heap_arr* r = h6_heap_arr_mk();
            heap_arr_push(r, v);
            op ro; ro.kind = CustomPushArr; ro.custom.push_arr = r;
            heap_arr_push(rt->stack, ro);
        } break;

        case System: {
            assert(rt->syscall);
            rt->syscall(rt, o.arg.uint, rt->syscall_userptr);
        } break;

        case TypeId: {
            op v = heap_arr_pop(rt->stack);
            int id = v.kind == Push ? 0 : 1;
            op_destr(v);
            heap_arr_push(rt->stack, mk_push(id));
        } break;

        case Materialize: {
            op v = heap_arr_pop(rt->stack);
            assert(v.kind == CustomPushArr);

            heap_arr* old_stack = rt->stack;
            rt->stack = h6_heap_arr_mk();
            run_arr(rt, v.custom.push_arr);
            op_destr(v);
            op new; new.kind = CustomPushArr; new.custom.push_arr = rt->stack;
            rt->stack = old_stack;

            heap_arr_push(rt->stack, new);
        } break;

        case OpsOf: {
            op v = heap_arr_pop(rt->stack);
            assert(v.kind == CustomPushArr);
            h6_heap_arr* arr = v.custom.push_arr;

            h6_heap_arr* by_out = h6_heap_arr_mk_opt_u8();
            for (size_t i = 0; i < arr->items_len; i ++) {
                op o = arr->items[i];
                h6_heap_arr_push_num(by_out, o.kind);
                if (op_has_arg(o.kind)) {
                    char* bytes = ((char*)&o.arg);
                    for (size_t i = 0; i < 4; i ++) {
                        h6_heap_arr_push_num(by_out, bytes[i]);
                    }
                }
            }

            h6_heap_arr_destr(arr);
            h6_heap_arr_push_box_arr(rt->stack, by_out);
            h6_heap_arr_destr(by_out);
        } break;

        // TODO: this is important-ish too
        case ConstAt:
          assert(0);
          break;
        }
    }
}

static void run_arr(h6_rt_t* rt, heap_arr* ops) {
    for (size_t i = 0; i < ops->items_len; i ++) {
        run_op(rt, ops->items[i]);
    }
}

void h6_run_bytecode(h6_rt_t* rt, char* bytecode) {
    size_t gtab_nent = *(uint16_t*) &bytecode[6];
    size_t gtab_off = *(uint32_t*) &bytecode[8];
    size_t main_off = gtab_off + gtab_nent * 8 + 16;

    heap_arr* main_ops = read_const(&bytecode[main_off]);
    run_arr(rt, main_ops);
}

h6_rt_t h6_mk_rt(char* bytecode, h6_rt_syscallback_t opt_syscallback, void* opt_syscallback_userptr) {
    h6_rt_t rt = {0};
    heap_arr* stack = h6_heap_arr_mk();
    rt.stack = stack;
    rt.bytecode = bytecode;
    rt.syscall = opt_syscallback;
    rt.syscall_userptr = opt_syscallback_userptr;
    rt.dso_by = NULL;
    rt.resolved_dso_len = 0;
    rt.resolved_dso_abs_off = NULL;
    return rt;
}

int32_t h6_heap_arr_pop_num(h6_heap_arr* arr) {
    assert(arr->rc);
    op o = heap_arr_pop(arr);
    return as_int(o);
}

h6_heap_arr* h6_heap_arr_pop_arr(h6_heap_arr* arr) {
    assert(arr->rc);
    op o = heap_arr_pop(arr);
    assert(o.kind == CustomPushArr);
    return o.custom.push_arr;
}

int32_t h6_heap_arr_get_num(h6_heap_arr* arr, size_t idx) {
    assert(idx < arr->items_len);
    assert(arr->rc);
    op o = arr->items[idx];
    return as_int(o);
}

h6_heap_arr* h6_heap_arr_get_arr(h6_heap_arr* arr, size_t idx) {
    assert(idx < arr->items_len);
    assert(arr->rc);
    op o = arr->items[idx];
    o = op_dup(o);
    assert(o.kind == CustomPushArr);
    return o.custom.push_arr;
}

h6_op* h6_heap_arr_get_op(h6_heap_arr* arr, size_t idx) {
    assert(idx < arr->items_len);
    assert(arr->rc);
    return &arr->items[idx];
}

void h6_set_dso(h6_rt_t* rt, char* /** MOVED */ dso_bytecode) {
    assert(!rt->dso_by);
    rt->dso_by = dso_bytecode;

    size_t ex_header_off = *(uint32_t*) &rt->bytecode[12];
    if (!ex_header_off)
        return;

    char* ex_header = &rt->bytecode[ex_header_off];

    size_t ex_header_len = *(uint16_t*) &ex_header[0];
    size_t num_dso_ent = *(uint32_t*) &ex_header[2];

    uint32_t* dso_tab = (uint32_t*) &ex_header[ex_header_len];

    rt->resolved_dso_len = num_dso_ent;
    rt->resolved_dso_abs_off = malloc(sizeof(uint32_t) * num_dso_ent);

    struct global_kv {
        uint32_t name;
        uint32_t value;
    } __attribute__((packed));

    uint16_t dso_globals_nent = *(uint16_t*) &dso_bytecode[6];
    struct global_kv* globals = (struct global_kv*) &dso_bytecode[16 + *(uint32_t*) &dso_bytecode[8]];

    for (size_t i = 0; i < num_dso_ent; i ++) {
        char* name = &rt->bytecode[16 + dso_tab[i]];

        int found = 0;
        for (uint16_t g = 0; g < dso_globals_nent; g ++) {
            char* gname = &dso_bytecode[16 + globals[g].name];
            if (!strcmp(name, gname)) {
                rt->resolved_dso_abs_off[i] = 16 + globals[g].value;
                found = 1;
                break;
            }
        }
        if (!found) {
            fprintf(stderr, "dso not found: %s\n", name);
            exit(1);
        }
    }
}
