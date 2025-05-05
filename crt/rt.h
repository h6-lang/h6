#ifndef _H6_RT_H
#define _H6_RT_H

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

typedef struct h6_heap_arr h6_heap_arr;

typedef struct h6_op h6_op;

void h6_op_print(FILE* out, h6_op *o);

void h6_heap_arr_destr(h6_heap_arr*);
h6_heap_arr* h6_heap_arr_mk();
size_t h6_heap_arr_len(h6_heap_arr*);

void h6_heap_arr_push_num(h6_heap_arr* arr, int32_t num);

/** note that you still have to h6_heap_arr_destr(other) afterwards */
void h6_heap_arr_push_box_arr(h6_heap_arr* arr, h6_heap_arr* other);

int32_t h6_heap_arr_pop_num(h6_heap_arr* arr);
h6_heap_arr* h6_heap_arr_pop_arr(h6_heap_arr* arr);

int32_t h6_heap_arr_get_num(h6_heap_arr* arr, size_t idx);
h6_heap_arr* h6_heap_arr_get_arr(h6_heap_arr* arr, size_t idx);

h6_op* h6_heap_arr_get_op(h6_heap_arr* arr, size_t idx);

typedef struct h6_rt_t h6_rt_t;
typedef void (*h6_rt_syscallback_t)(h6_rt_t* rt, uint32_t id, void* userptr);
struct h6_rt_t {
    h6_heap_arr* stack;
    char* bytecode;

    h6_rt_syscallback_t syscall;
    void* syscall_userptr;

/*private:*/
    size_t ind;
    h6_heap_arr* building_arr;

/*private:*/
    char* dso_by;
    uint32_t* resolved_dso_abs_off;
    size_t resolved_dso_len;
};

h6_rt_t h6_mk_rt(char* bytecode, h6_rt_syscallback_t opt_syscallback, void* opt_syscallback_userptr);
/** the dso bytecode object has to be already self-linked, and can NOT contain dso references itself */
void h6_set_dso(h6_rt_t* rt, char* /** MOVED */ dso_bytecode);
void h6_run_bytecode(h6_rt_t* rt, char* bytecode);

#endif
