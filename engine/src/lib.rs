pub mod agentic;
pub mod ai;
pub mod lsp;
pub mod safety;
pub mod system;
pub mod terminal;
pub mod workspace;

// Backward-compatibility re-exports for the flat root modules
pub use safety::audit;
pub use safety::history;
pub use safety::permissions;
pub use safety::recovery;
pub use safety::search_policy;
pub use system::app_data;
pub use system::logs;
pub use system::recent_projects;
pub use system::settings;
pub use terminal::devserver_detect;
pub use terminal::error_detect;
pub use terminal::ring_buffer;
pub use workspace::fs;
pub use workspace::fs_watch;
pub use workspace::git;
pub use workspace::search;
pub use workspace::tree;
