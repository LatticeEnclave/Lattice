#include "vstack.h"

static void *memcpy(void *dest, const void *src, size_t n) {
    if (dest == NULL || src == NULL) return NULL; 

    char *d = (char *)dest;
    const char *s = (const char *)src;

    while (n--) {
        *d++ = *s++;
    }

    return dest;
}

struct vstack *vs_create(void *start, size_t len) {
  size_t size;
  struct vstack *vs;

  if (!start) {
    return NULL;
  }

  size = len - sizeof(struct vstack);
  vs = (struct vstack*)start;
  vs->size = size;
  vs->sp = (uintptr_t)(start + len);

  return vs;
}

void *vs_store(struct vstack *vs, void *value, size_t len) {
  unsigned long new;

  if (vs->size < len) {
    return NULL;
  }

  new = vs->sp - len;
  if (!memcpy((void *)new, value, len)) {
    return NULL;
  };
  vs->sp = new;

  return (void *)new;
}

