use crate::common::error::Error;

pub mod client;
pub mod common;
pub mod server;

#[tokio::main]
async fn main() -> Result<(), Error> {
    Ok(())
}
