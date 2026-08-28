pub mod irc;
pub mod steer;
pub mod async_job;
pub mod daemon;
pub mod ids;
pub mod message;
pub mod agent;
pub mod event;
pub mod finding;
pub mod tool;
pub mod config;
pub mod error;
pub mod token;
pub mod memory;
pub mod skill;
pub mod profile;
pub mod session;
pub mod export;
pub mod notify;
pub mod contact;
pub mod crm;
pub mod capability;
pub mod protected;
pub mod uri;

pub use irc::*;
pub use steer::*;
pub use async_job::*;
pub use daemon::*;
pub use ids::*;
pub use message::*;
pub use agent::*;
pub use event::*;
pub use finding::*;
pub use tool::*;
pub use config::*;
pub use error::*;
pub use token::*;
pub use memory::*;
pub use skill::*;
pub use profile::*;
pub use session::*;
pub use export::*;
pub use notify::*;
pub use contact::*;
pub use crm::*;
pub use capability::*;
pub use protected::*;
pub use uri::*;

/// Shared outbound HTTP client with bounded timeouts, so a slow or hanging
/// endpoint can never stall an agent indefinitely. Use everywhere instead of
/// `reqwest::Client::new()`.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}
