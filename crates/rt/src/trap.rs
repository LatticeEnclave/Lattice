use core::ops::DerefMut;

use crate::{log, println};
use riscv::register::{
    satp::{self, Mode},
    scause::{self, Exception, Interrupt},
    sepc, sie, sscratch, sstatus, stval, stvec,
};
use sbi::{
    ecall::{runtime_sbi_call_from_usize, RuntimeSbiCall},
    TrapRegs,
};
use vm::{page_table::PTEFlags, vm::VirtPageNum, Translate, VirtAddr};

use crate::{
    consts::{DRIVER_STACK_SIZE, PAGE_SIZE},
    context::TrapRegsSMode,
    kernel::{self, LinuxDriverKernel},
    pt::RtPtWriter,
    scratch::Scratch,
    syscall::{
        linux_syscall, sbi_copy_from_kernel, sbi_exit_enclave, sbi_recv_channel, sbi_stop_enclave,
    },
    trap_restore_a0_t0_smode, trap_restore_general_regs_except_a0_t0_smode,
    trap_restore_sepc_sstatus, trap_save_and_setup_sp_t0_smode,
    trap_save_general_regs_except_sp_t0_smode, trap_save_sepc_sstatus, trap_switch_satp,
};

#[no_mangle]
#[inline(never)]
#[link_section = ".tp"]
pub extern "C" fn handle_the_lue_trap() -> ! {
    unsafe {
        _enter();
        _call_lue_handle();
        _exit();
    }
}

#[no_mangle]
#[inline(always)]
#[link_section = ".tp"]
pub unsafe fn _enter() {
    // save t0, sp, change sp to the hart stack
    trap_save_and_setup_sp_t0_smode!();
    trap_save_sepc_sstatus!();
    trap_save_general_regs_except_sp_t0_smode!();
    core::arch::asm!("add    a0, sp, zero");
    trap_switch_satp!();
}

#[no_mangle]
#[inline(always)]
#[link_section = ".tp"]
unsafe extern "C" fn _call_lue_handle() {
    core::arch::asm!("call   {}", sym lue_handle,);
}

#[no_mangle]
fn lue_handle(regs: &mut TrapRegsSMode) {
    let cause = scause::read();
    let tval = stval::read();
    if cause.is_exception() {
        let exception = scause::Exception::from(cause.code());
        match exception {
            Exception::UserEnvCall => {
                let n = runtime_sbi_call_from_usize(regs.a7);
                let arg0 = regs.a0;
                let arg1 = regs.a1;
                let arg2 = regs.a2;
                let arg3 = regs.a3;
                let arg4 = regs.a4;
                let arg5 = regs.a5;
                regs.sepc += 4;

                // enable_supervisor_interrupt();
                // disable_any_interrupt();

                match n {
                    Some(rt_call) => match rt_call {
                        RuntimeSbiCall::RuntimeSyscallOcall => todo!(),
                        RuntimeSbiCall::RuntimeSyscallSharedcopy => todo!(),
                        RuntimeSbiCall::RuntimeSyscallAttestEnclave => todo!(),
                        RuntimeSbiCall::RuntimeSyscallGetSealingKey => todo!(),
                        RuntimeSbiCall::RuntimeSyscallExit => {
                            sbi_exit_enclave(arg0);
                        }
                        _ => {
                            panic!("Unsupported trap",);
                        }
                    },
                    None => {
                        let syscall_id = regs.a7;
                        regs.a0 = linux_syscall(syscall_id, [arg0, arg1, arg2, arg3, arg4, arg5])
                            as usize;
                    }
                }
                // disable_supervisor_interrupt();
            }
            Exception::LoadPageFault => {
                log::error!(
                    "get page fault. sepc: {:#x}, stval: {:#x}, scause: {:#x}",
                    regs.sepc,
                    stval::read(),
                    scause::read().bits()
                );
                // handle_load_page_fault(regs);
                panic!()
            }
            Exception::StorePageFault => {
                log::error!(
                    "get page fault. sepc: {:#x}, stval: {:#x}, scause: {:#x}",
                    regs.sepc,
                    stval::read(),
                    scause::read().bits()
                );
                // handle_store_page_fault(regs);
                panic!()
            }
            Exception::InstructionPageFault => {
                log::error!(
                    "get page fault. sepc: {:#x}, stval: {:#x}, scause: {:#x}",
                    regs.sepc,
                    stval::read(),
                    scause::read().bits()
                );
                // let user_satp = Scratch::from_ssratch()

                panic!()
            }
            _ => {
                todo!()
            }
        }
    } else {
        let interrupt = scause::Interrupt::from(cause.code());
        match interrupt {
            Interrupt::SupervisorTimer => {
                // log::debug!("time interrupt.");
                // log::debug!("sip: {:#x}", riscv::register::sip::read().bits());
                sbi_stop_enclave(1);
            }
            Interrupt::SupervisorExternal => {
                // log::debug!("sei.");
                // log::debug!("sip: {:#x}", riscv::register::sip::read().bits());
                sbi_stop_enclave(1);
            }
            Interrupt::SupervisorSoft => {
                // println!("ssoft.");
                // log::debug!("sip: {:#x}", riscv::register::sip::read().bits());
                sbi_stop_enclave(1);
            }
            _ => {
                log::error!("unsupported trap: {:#x}", cause.bits());
                log::error!("sepc: {:#x}", regs.sepc);
                log::error!("stval: {:#x}", tval);
                log::error!("sstatus: {:#x}", regs.sstatus);
                log::error!("scause: {:#x}", cause.bits());
                panic!("Unsupported trap!",);
            }
        }
    }
}

#[no_mangle]
#[inline(always)]
#[link_section = ".tp"]
unsafe fn _exit() -> ! {
    // switch back to process satp
    trap_switch_satp!();
    core::arch::asm!(
        "sd     a0, -{}(sp)",
        "mv     a0, sp",
        const core::mem::size_of::<usize>(),
    );
    trap_restore_general_regs_except_a0_t0_smode!();
    trap_restore_sepc_sstatus!();
    trap_restore_a0_t0_smode!();
    core::arch::asm!("sret", options(noreturn))
}

pub fn lue_stvec_init() {
    unsafe {
        stvec::write(handle_the_lue_trap as usize, stvec::TrapMode::Direct);
    }
}

pub fn lde_stvec_init() {
    unsafe {
        stvec::write(handle_the_lde_trap as usize, stvec::TrapMode::Direct);
    }
}

pub fn disable_any_interrupt() {
    unsafe {
        sie::clear_sext();
        sie::clear_ssoft();
    }
}

pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

pub fn disable_timer_interrupt() {
    unsafe {
        sie::clear_stimer();
    }
}

pub fn enable_supervisor_interrupt() {
    unsafe {
        sstatus::set_sie();
    }
}

pub fn disable_supervisor_interrupt() {
    unsafe {
        sstatus::clear_sie();
    }
}

fn handle_load_page_fault(regs: &mut TrapRegsSMode) {
    let user_satp = unsafe { Scratch::from_ssratch().prev_satp };
}

fn handle_store_page_fault(regs: &mut TrapRegsSMode) {
    let scratch = unsafe { Scratch::from_ssratch() };
    let kernel = scratch.get_lu_kernel();

    let pte = kernel
        .task
        .vmm
        .lock()
        .get_pte(VirtPageNum::from_vaddr(stval::read()))
        .unwrap();

    // if pte.is_w() && pte.is
    let mut flag = pte.get_flags();
    assert!(flag.contains(PTEFlags::V));
    if !flag.contains(PTEFlags::W) {
        log::error!("{:#x} not allowed to write", stval::read());
        panic!()
    }
    if !flag.contains(PTEFlags::D) {
        flag = flag.dirty();
    }
    if !flag.contains(PTEFlags::A) {
        flag = flag.accessed();
    }

    // kernel.task.vmm.lock()

    // let task = kernel.task.vmm.lock().get_pte(vpn)

    // satp::read().ppn();
}

pub fn trap_to_process(
    a0: usize,
    a1: usize,
    entry: usize,
    sp: usize,
    satp: usize,
    sstatus: usize,
) -> ! {
    unsafe {
        debug_assert_ne!(satp, 0);
        // set sscratch pre_satp as task satp
        core::arch::asm!(
            "csrrw  tp, sscratch, tp",
            "sd     {}, {}(tp)",
            "csrrw  tp, sscratch, tp",
            in(reg) satp,
            const core::mem::offset_of!(Scratch, prev_satp),
        );

        // prepare task init context
        let mut regs = TrapRegsSMode::default();
        regs.a0 = a0;
        regs.a1 = a1;
        regs.sepc = entry;
        regs.sp = sp;
        regs.sstatus = sstatus;

        let regs_ptr = &regs as *const TrapRegsSMode as usize;

        // jump to _exit
        core::arch::asm!(
            "mv     sp, {}", // force sp point to regs
            "fence.i",
            "j      {}",
            in(reg) regs_ptr,
            sym _exit,
            options(noreturn)
        )
    }
}

/// it will only trap when page fault and access fault happen
/// the sscratch can be saved while the driver running
#[no_mangle]
#[inline(never)]
#[link_section = ".tp"]
pub extern "C" fn handle_the_lde_trap() -> ! {
    unsafe {
        _enter();
        _call_lde_handle();
        _exit();
    }
}

#[no_mangle]
#[inline(always)]
#[link_section = ".tp"]
unsafe extern "C" fn _call_lde_handle() {
    core::arch::asm!("call   {}", sym lde_handle,);
}

/// handle the page fault and access fault
#[no_mangle]
fn lde_handle(regs: &mut TrapRegsSMode) {
    let cause = scause::read();
    log::debug!("lde handle: {:#x}", cause.bits());
    if cause.is_exception() {
        let exception = scause::Exception::from(cause.code());
        match exception {
            // enter lde
            Exception::UserEnvCall => {
                let kernel = unsafe { LinuxDriverKernel::from_sscratch() };

                // clear the previous mapped stack pages
                let mut mapped_pages = kernel.stack_pages.lock();
                let mut vmm_lock = kernel.vmm.lock();

                for mapped_addr in mapped_pages.clone() {
                    vmm_lock.dealloc_vma(mapped_addr, PAGE_SIZE);
                }

                // clear the mapped_pages vec
                mapped_pages.deref_mut().clear();

                // new stack region
                let stk_vaddr = regs.sp;
                let stk_vpn = VirtPageNum::from_vaddr(stk_vaddr);

                let start_vpn = stk_vpn.sub(DRIVER_STACK_SIZE);
                let end_vpn = if stk_vaddr % PAGE_SIZE == 0 {
                    stk_vpn
                } else {
                    stk_vpn.add(1)
                };

                for vpn in start_vpn..=end_vpn {
                    // mapped stack pages set update
                    mapped_pages.deref_mut().push(vpn.0 * PAGE_SIZE);

                    // map physical page for stack
                    vmm_lock.alloc_new_page(vpn, PTEFlags::rwx()).unwrap();
                }

                riscv::asm::sfence_vma_all();

                // copy stack data for driver
                sbi_copy_from_kernel(stk_vaddr, stk_vaddr, PAGE_SIZE);

                // set the spp to supervisor mode
                regs.sstatus |= 0x1 << 8;
            }
            Exception::LoadPageFault | Exception::StorePageFault => {
                let tval = stval::read();
                let page_vaddr = (tval / PAGE_SIZE) * PAGE_SIZE;
                let kernel = unsafe { LinuxDriverKernel::from_sscratch() };
                kernel
                    .vmm
                    .lock()
                    .alloc_new_page(VirtPageNum::from_vaddr(page_vaddr), PTEFlags::rwx());
                // kernel.mapped_pages.lock().deref_mut().push(page_vaddr);

                sbi_copy_from_kernel(page_vaddr, page_vaddr, PAGE_SIZE);

                // no need to modify sepc, execute the old instruction again
            }
            _ => todo!(),
        }
    } else {
        panic!("Unsupported trap!",);
    }
}
