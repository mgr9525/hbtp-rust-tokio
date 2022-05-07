
mod msg;
pub mod entity;
pub mod tcps;
pub mod udps;

pub use msg::{Message,Messages,Messageu,Messageus};


// pub const MaxOther: u64 = 1024 * 1024 * 20; //20M
pub const MAX_HEADS: u64 = 1024 * 1024 * 100; //100M
pub const MAX_BODYS: u64 = 1024 * 1024 * 1024; //1G

