use quinn::VarInt;

pub struct Code {
    pub code_u32: u32,
    pub code: VarInt,
    pub reason: &'static [u8],
}

pub const WRONG_AUTH_KEY: Code = Code {
    code_u32: 1,
    code: VarInt::from_u32(1),
    reason: b"wrong auth key",
};

pub const SERVER_DISCONNECTED: Code = Code {
    code_u32: 2,
    code: VarInt::from_u32(2),
    reason: b"server disconnected",
};
