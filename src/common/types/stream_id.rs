use std::fmt::{Display, Formatter};

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct StreamId(u64);

impl StreamId {
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        StreamId(u64::from_be_bytes(bytes))
    }

    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn to_bytes(&self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
}

impl Display for StreamId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(self.0.fmt(f)?)
    }
}
