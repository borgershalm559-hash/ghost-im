//! Repositories — one per table. Each takes `&Database` and exposes a focused API.

pub mod contacts;
pub mod mls_groups;

pub use contacts::{Contact, ContactsRepo, Verification};
pub use mls_groups::{MlsGroupRow, MlsGroupsRepo};
