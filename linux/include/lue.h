#include "../include/ecall.h"

#ifdef __riscv
int ecall_finish_ctl(void *head, long res) {
  register long arg0 asm("a0") = (long)head;
  register long res asm("a1") = res;
  register long ext_id asm("a6") = SBI_EXT_HTEE_ENCLAVE;
  register long ctl_id asm("a7") = SBI_SM_ENCLAVE_FINISH_CTL;
  asm volatile("ecall"
               :
               : "r"(arg0), "r"(res), "r"(ext_id), "r"(ctl_id)
               : "memory");
}
#else
#define ecall_finish_ctl(head) ;
#endif
