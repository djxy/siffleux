use quinn::VarInt;

pub struct Code {
    pub code: VarInt,
    pub reason: &'static [u8],
}

pub const CLOSED: Code = Code {
    code: VarInt::from_u32(0),
    reason: b"done",
};

pub const AUTH_KEY_REJECTED: Code = Code {
    code: VarInt::from_u32(1),
    reason: b"auth key rejected",
};

pub const INGRESS_ID_REJECTED: Code = Code {
    code: VarInt::from_u32(2),
    reason: b"ingress id rejected",
};

pub const FIRST_FRAME_RECEIVED_NOT_AUTH: Code = Code {
    code: VarInt::from_u32(3),
    reason: b"first frame received not auth",
};

pub const AUTH_FRAME_NOT_RECEIVED: Code = Code {
    code: VarInt::from_u32(4),
    reason: b"auth frame not received",
};

pub const SERVER_SIDE_ISSUE: Code = Code {
    code: VarInt::from_u32(5),
    reason: b"server side issue",
};

pub const COMMAND_STREAM_CLOSED: Code = Code {
    code: VarInt::from_u32(6),
    reason: b"command stream closed",
};

pub const TCP_OR_QUIC_STREAM_FAILED: Code = Code {
    code: VarInt::from_u32(7),
    reason: b"tcp or quic stream failed",
};
