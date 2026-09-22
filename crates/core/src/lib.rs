pub mod agent;
pub mod artifact;
pub mod async_job;
pub mod capability;
pub mod config;
pub mod contact;
pub mod crm;
pub mod daemon;
pub mod error;
pub mod eval;
pub mod event;
pub mod export;
pub mod finding;
pub mod ids;
pub mod irc;
pub mod memory;
pub mod message;
pub mod notify;
pub mod profile;
pub mod protected;
pub mod session;
pub mod skill;
pub mod steer;
pub mod token;
pub mod tool;
pub mod uri;
pub use agent::*;
pub use artifact::*;
pub use async_job::*;
pub use capability::*;
pub use config::*;
pub use contact::*;
pub use crm::*;
pub use daemon::*;
pub use error::*;
pub use eval::*;
pub use event::*;
pub use export::*;
pub use finding::*;
pub use ids::*;
pub use irc::*;
pub use memory::*;
pub use message::*;
pub use notify::*;
pub use profile::*;
pub use protected::*;
pub use session::*;
pub use skill::*;
pub use steer::*;
pub use token::*;
pub use tool::*;
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
