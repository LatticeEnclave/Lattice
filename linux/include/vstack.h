#include <linux/types.h>

struct vstack {
  size_t regs[8];
  size_t size;
  uintptr_t sp;
};

#define vs_bp(vs) (uintptr_t)((void *)vs + sizeof(struct vstack))
#define vs_sp(vs) (uintptr_t)vs->sp
#define vs_is_full(vs) vstack_bp(vs) == vs->sp
#define vs_is_empty(vs) vs->sp == vstack_bp(vs) + vs->size

struct vstack *vs_create(void *start, size_t size);

void *vs_store(struct vstack *vs, void *value, size_t len);
