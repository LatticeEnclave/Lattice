pub use super::misa::XLEN;

read_csr!(0x320);
write_csr!(0x320);
set!(0x320);
clear!(0x320);

set_clear_csr!(
    /// Cycle Counter Disable/Enable
    , set_cy, clear_cy, 1 << 0);
set_clear_csr!(
    /// Instret Counter Disable/Enable
    , set_ir, clear_ir, 1 << 2);
