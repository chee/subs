pub mod none;
pub mod socket;
pub mod watch;

pub enum Manager {
    Watch,
    Socket,
    None,
}
