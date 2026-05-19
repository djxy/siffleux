use quinn::VarInt;

pub struct Code {
    pub code: VarInt,
    pub reason: &'static [u8],
}

pub const CLOSED: Code = Code {
    code: VarInt::from_u32(0),
    reason: b"connection closed successfully.",
};

pub const WRONG_AUTH_KEY: Code = Code {
    code: VarInt::from_u32(1),
    reason: b"wrong auth key.",
};