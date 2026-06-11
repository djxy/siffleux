use quinn::VarInt;

// ###################
// Connection codes [1000-1999]
// ###################
pub const CONNECTION_EOF: VarInt = VarInt::from_u32(0);
pub const COMMAND_STREAM_CLOSED: VarInt = VarInt::from_u32(1);

// ###################
// Stream codes [2000-2999]
// ###################
pub const STREAM_EOF: VarInt = VarInt::from_u32(2000);

pub const DATA_STREAM_ERROR: VarInt = VarInt::from_u32(2001);

// ###################
// Common codes [3000-3999]
// ###################
pub const INVALID_VALUE: VarInt = VarInt::from_u32(3000);
pub const UNEXPECTED_FRAME_RECEIVED: VarInt = VarInt::from_u32(3001);
pub const FRAME_NOT_RECEIVED_ON_TIME: VarInt = VarInt::from_u32(3002);

pub const UNKNOWN_ERROR: VarInt = VarInt::from_u32(3999);
pub const UNKNOWN_ERROR_SERVER_REASON: &[u8] = b"Unknown server side issue.";
pub const UNKNOWN_ERROR_CLIENT_REASON: &[u8] = b"Unknown client side issue.";
