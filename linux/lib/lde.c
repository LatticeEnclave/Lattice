#include "../include/lde.h"

#ifdef __riscv
void ecall_request_ctl(void *head) {
  register long arg0 asm("a0") = (long)head;
  register long ctl_id asm("a6") = SBI_SM_ENCLAVE_CTL;
  register long ext_id asm("a7") = SBI_EXT_HTEE_ENCLAVE;
  asm volatile("unimp" : : "r"(arg0), "r"(ext_id), "r"(ctl_id) : "memory");
}

int elock(long vaddr, long size) {
  register long arg0 asm("a0") = vaddr;
  register long arg1 asm("a1") = size;
  register long lock_id asm("a6") = SBI_SM_ELOCK;
  register long ext_id asm("a7") = SBI_EXT_HTEE_ENCLAVE;
  register long rc asm("a0");
  asm volatile("unimp" : "=r"(rc) : "r"(arg0), "r"(arg1), "r"(ext_id), "r"(lock_id) : "memory");
  return rc;
}

int efree(long vaddr, long size) {
  register long arg0 asm("a0") = vaddr;
  register long arg1 asm("a1") = size;
  register long free_id asm("a6") = SBI_SM_EFREE;
  register long ext_id asm("a7") = SBI_EXT_HTEE_ENCLAVE;

  register long rc asm("a0");
  asm volatile("unimp" : "=r"(rc) : "r"(arg0), "r"(arg1), "r"(ext_id), "r"(free_id) : "memory");
  return rc;
}

#else
// #define ecall_eenter() register long rc asm("a0"); asm volatile("" : "=r"(rc) : :);
// inline __attribute__((always_inline)) int ecall_eenter() {
//   return -1;
// }

// inline __attribute__((always_inline)) int ecall_eexit() {
//   return -1;
// }

void ecall_request_ctl(void *head) {}
#endif
