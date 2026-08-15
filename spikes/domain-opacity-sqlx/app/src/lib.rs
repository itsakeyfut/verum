//! Plays the user's application crate: it defines the Domain, the Repository
//! implementation, and ordinary handler code, all in one crate. That is the
//! common shape, and it is the shape under which `pub(crate)` is widest.
pub mod domain;
pub mod handler;
pub mod repo;
