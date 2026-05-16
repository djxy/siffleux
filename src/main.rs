use crate::error::Error;

pub mod client;
pub mod error;
pub mod message;
pub mod server;
pub mod types;

#[tokio::main]
async fn main() -> Result<(), Error> {
    Ok(())
}
