//! The AI agent's own tools on Amethystora: an MCP server over stdio, started by the agent as
//! `amethystora-agent-tools mcp` and living only while it does, and the same answers as a command. Level 1
//! knows: it searches a local index of the manual and the image's own reference, and reads the machine's
//! state and journal. Level 2 acts, and asks first every time. plans/done/PLAN-agentic-os.md says why.

pub mod act;
pub mod checks;
pub mod files;
pub mod handoff;
pub mod history;
pub mod index;
pub mod logs;
pub mod mcp;
pub mod never;
pub mod policy;
pub mod root;
pub mod routines;
pub mod run;
pub mod server;
pub mod state;
pub mod text;

pub const IMAGE_INFO: &str = "/usr/share/amethystora/image-info.json";
pub const DOCTOR: &str = "/usr/libexec/amethystora-doctor";
