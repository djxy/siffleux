use base64::{
    Engine, alphabet,
    engine::{self, general_purpose},
};
use tokio::signal::unix::{SignalKind, signal};

pub const BASE64_ENGINE: engine::GeneralPurpose =
    engine::GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::PAD);

pub fn generate_secure_random_key<const L: usize>() -> String {
    let mut bytes = [0u8; L];

    getrandom::fill(&mut bytes).unwrap();

    BASE64_ENGINE.encode(bytes)
}

pub async fn wait_for_shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    let sigterm = async {
        signal(SignalKind::terminate())
            .expect("Failed to listen SIGTERM signal.")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = sigterm => {},
    }
}
