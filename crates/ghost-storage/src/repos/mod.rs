//! Repositories — one per table. Each takes `&Database` and exposes a focused API.

pub mod contacts;
pub mod messages;
pub mod mls_groups;
pub mod my_keypackages;

pub use contacts::{Contact, ContactsRepo, Verification};
pub use messages::{Direction, MessageRow, MessageStatus, MessagesRepo};
pub use mls_groups::{MlsGroupRow, MlsGroupsRepo};
pub use my_keypackages::{MyKeyPackageRow, MyKeyPackagesRepo};
