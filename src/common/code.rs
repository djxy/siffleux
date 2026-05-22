use quinn::VarInt;

pub struct Code {
    pub code: VarInt,
    pub reason: &'static [u8],
}
