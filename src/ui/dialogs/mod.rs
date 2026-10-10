//! Modal dialogs.

mod confirm_delete;
mod drive_dialog;
mod error_dialog;
mod scan_dialog;

pub use confirm_delete::confirm_delete;
pub use drive_dialog::{DriveDialog, DriveDialogOutcome};
pub use error_dialog::error_dialog;
pub use scan_dialog::scan_dialog;
