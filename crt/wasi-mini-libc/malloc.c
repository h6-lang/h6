#include "lib.h"
#include "string.h"

//extern uint32_t __heap_base;
//extern uint32_t __heap_end;

void *unsafe_sbrk(intptr_t /* NUMBER OF PAGES; NOT NEGATIVE */ increment) {
    // sbrk(0) returns the current memory size.
    if (increment == 0) {
        // The wasm spec doesn't guarantee that memory.grow of 0 always succeeds.
        return (void *)(__builtin_wasm_memory_size(0) * PAGESIZE);
    }

    uintptr_t old = __builtin_wasm_memory_grow(0, (uintptr_t)increment);

    return (void *)(old * PAGESIZE);
}

static size_t align(size_t increment) {
    return (increment + 3) & ~3;
}

void* still_unsafe_sbrk(size_t increment) {
    debug_assert(increment % 4 == 0);
    debug_assert(increment != 0);

    static uint32_t begin = 0;
    static uint32_t page_rem = 0;

    if (page_rem < (uint32_t)increment) {
        begin = (uint32_t)(intptr_t) unsafe_sbrk((increment + PAGESIZE - 1) / PAGESIZE);
        page_rem = PAGESIZE;
    }

    uint32_t pt = begin;
    page_rem -= increment;
    begin += increment;
    return (void*)(intptr_t) pt;
}

typedef struct {
    unsigned used : 1;
    uint32_t len  : 31;
} __attribute__((packed)) AllocNode;

static_assert(sizeof(AllocNode) % 4 == 0);

static AllocNode* last_alloc_node = 0;
static AllocNode* first_free_node = 0;

static char* nodeBytes(AllocNode* ndp) {
    return &((char*)ndp)[sizeof(AllocNode)];
}

static AllocNode* nextNode(AllocNode* ndp) {
    AllocNode* next = (AllocNode*)&nodeBytes(ndp)[ndp->len];
    if (next > last_alloc_node)
        return NULL;
    return next;
}

static AllocNode* nodeOfAlloc(void* ptr) {
    debug_assert((size_t)ptr >= sizeof(AllocNode));
    return (AllocNode*) &((char*)ptr)[-sizeof(AllocNode)];
}

void free(void* ptr) {
    AllocNode* nd = nodeOfAlloc(ptr);
    nd->used = 0;

    // merge with next node if free
    AllocNode* nextnd = nextNode(nd);
    if (nextnd && !nextnd->used) {
        nd->len += sizeof(AllocNode) + nextnd->len;
    }
    // TODO: also merge with prev node

    if (nd < first_free_node || first_free_node == 0) {
        first_free_node = nd;
    }
}

void* malloc(size_t num) {
#ifndef NDEBUG
    static void* next_expected_sbrk = 0;
    if (next_expected_sbrk == 0) {
        next_expected_sbrk = unsafe_sbrk(0);
    }
#endif

    num = align(num);

    // TODO: uncomment
    for (AllocNode* nd = first_free_node; nd; nd = nextNode(nd)) {
        if (!nd->used && nd->len >= num) {
            // TODO should split node if we wasting at least 15% but meh
            nd->used = 1;
            return nodeBytes(nd);
        }
    }
    AllocNode* p = still_unsafe_sbrk(sizeof(AllocNode) + num);
#ifndef NDEBUG
    debug_assert(next_expected_sbrk == p);
    next_expected_sbrk += sizeof(AllocNode) + num;
#endif

    debug_assert(p);
    p->used = 1;
    p->len = num;
    last_alloc_node = p;

    debug_assert((size_t)nodeBytes(p) % 4 == 0);
    debug_assert(nodeOfAlloc(nodeBytes(p)) == p);
    return nodeBytes(p);
}

void* realloc(void* ptr, size_t newnum) {
    if (!ptr)
        return malloc(newnum);

    AllocNode* oldNd = nodeOfAlloc(ptr);
    debug_assert(oldNd);
    debug_assert(oldNd->used);
    void* o = malloc(newnum);
    size_t cpynum = newnum;
    if (oldNd->len < cpynum)
        cpynum = oldNd->len;
    memcpy(o, ptr, cpynum);
    free(ptr);
    return o;
}
