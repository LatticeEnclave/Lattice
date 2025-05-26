#include "../include/ecall.h"

#ifdef __riscv
#define ecall_eenter(rc)   do { \
                        register long eenter_id asm("a6") = SBI_SM_EENTER; \
                        register long ext_id asm("a7") = SBI_EXT_TEE_ENCLAVE; \
                        register int ret asm("a0"); \
                        asm volatile("unimp" : "=r"(ret) :"r"(ext_id), "r"(eenter_id) : "memory"); \
                        rc = ret; \
                        } while (0)

#define ecall_eexit(rc)   do { \
                        register long eexit_id asm("a6") = SBI_SM_EEXIT; \
                        register long ext_id asm("a7") = SBI_EXT_TEE_ENCLAVE; \
                        register int ret asm("a0"); \
                        asm volatile("unimp" : "=r"(ret) :"r"(ext_id), "r"(eexit_id) : "memory", "a1", "a2", "a3", "a4", "a5", "t0", "t1", "t2", \
                        "t3", "t4", "t5", "t6", "tp", "s1", "s2", "s3", "s4", \
                        "s5", "s6", "s7", "s8", "s9", "s10", "s11"); \
                        rc = ret; \
                        } while (0)
#else
#define ecall_eenter(rc)  rc = -1;
#define ecall_eexit(rc)   rc = -1;
#endif

void ecall_request_ctl(void *head);

int elock(long vaddr, long size);

int efree(long vaddr, long size);

int esend(long vaddr, long size);