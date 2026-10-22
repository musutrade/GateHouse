//! Lifecycle design support. This module grants no execution authority.
//! Wire validation, protected host verification and durable storage are required
//! before this state model can be connected to an engine.
pub mod state;

mod canonical;
pub mod wire;

pub mod journal;

pub mod installation;
