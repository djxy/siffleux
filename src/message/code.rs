use quinn::VarInt;

pub struct Code {
    pub code: VarInt,
    pub reason: &'static [u8],
}

pub const WRONG_AUTH_KEY: Code = Code {
    code: VarInt::from_u32(1),
    reason: b"wrong auth key",
};