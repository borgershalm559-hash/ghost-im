//! Repositories — one per table. Each takes `&Database` and exposes a focused API.

pub mod contacts;

pub use contacts::{Contact, ContactsRepo, Verification};
