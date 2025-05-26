#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelStatus {
    Free,
    Using,
}

pub struct Channel<T> {
    pub lue: Option<T>,
    pub lde: Option<T>,
    pub arg0: u64,
    pub arg1: u64,
    pub status: ChannelStatus,
}

impl<T> Channel<T> {
    pub const EMPTY: Self = Self {
        lue: None,
        lde: None,
        arg0: 0,
        arg1: 0,
        status: ChannelStatus::Free,
    };

    pub const fn new() -> Self {
        Self::EMPTY
    }
}
