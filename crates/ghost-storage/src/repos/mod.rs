//! Repositories — one per table. Each takes `&Database` and exposes a focused API.

pub mod contacts;
pub mod folders;
pub mod inbox_dedup;
pub mod messages;
pub mod mls_groups;
pub mod my_keypackages;
pub mod outbox;
pub mod settings;

pub use contacts::{Contact, ContactsRepo, Verification};
pub use folders::{Folder, FoldersRepo};
pub use inbox_dedup::InboxDedupRepo;
pub use messages::{Direction, MessageRow, MessageStatus, MessagesRepo};
pub use mls_groups::{MlsGroupRow, MlsGroupsRepo};
pub use my_keypackages::{MyKeyPackageRow, MyKeyPackagesRepo};
pub use outbox::{OutboxRepo, OutboxRow};
pub use settings::SettingsRepo;
