use xmas_elf::dynamic::*;
use bitflags::bitflags;

bitflags! {
    #[derive(Default)]
    pub struct DynamicFlags1: u64 {
        const NOW = FLAG_1_NOW;
        const GLOBAL = FLAG_1_GLOBAL;
        const GROUP = FLAG_1_GROUP;
        const NODELETE = FLAG_1_NODELETE;
        const LOADFLTR = FLAG_1_LOADFLTR;
        const INITFIRST = FLAG_1_INITFIRST;
        const NOOPEN = FLAG_1_NOOPEN;
        const ORIGIN = FLAG_1_ORIGIN;
        const DIRECT = FLAG_1_DIRECT;
        const TRANS = FLAG_1_TRANS;
        const INTERPOSE = FLAG_1_INTERPOSE;
        const NODEFLIB = FLAG_1_NODEFLIB;
        const NODUMP = FLAG_1_NODUMP;
        const CONFALT = FLAG_1_CONFALT;
        const ENDFILTEE = FLAG_1_ENDFILTEE;
        const DISPRELDNE = FLAG_1_DISPRELDNE;
        const DISPRELPND = FLAG_1_DISPRELPND;
        const NODIRECT = FLAG_1_NODIRECT;
        const IGNMULDEF = FLAG_1_IGNMULDEF;
        const NOKSYMS = FLAG_1_NOKSYMS;
        const NOHDR = FLAG_1_NOHDR;
        const EDITED = FLAG_1_EDITED;
        const NORELOC = FLAG_1_NORELOC;
        const SYMINTPOSE = FLAG_1_SYMINTPOSE;
        const GLOBAUDIT = FLAG_1_GLOBAUDIT;
        const SINGLETON = FLAG_1_SINGLETON;
        const STUB = FLAG_1_STUB;
        const PIE = FLAG_1_PIE;
    }
}

pub struct DynamicInfo {
    pub flags1: DynamicFlags1,
    pub rela: u64,
    pub rela_size: u64,
}