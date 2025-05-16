use core::fmt::Display;

#[derive(Debug)]
pub enum Error {
    NodeNotFound(&'static str),
    RegNotFound(&'static str),
}

impl Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::NodeNotFound(node) => write!(f, "Node not found: {}", node),
            Error::RegNotFound(node) => write!(f, "Reg not found: {}", node),
        }
    }
}
