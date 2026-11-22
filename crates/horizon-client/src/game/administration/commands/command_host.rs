//! `Game` as the host of the commands that moved to `horizon_gameplay::commands`. Each command
//! family has its trait there and its impl in a file of `command_host/`; every method
//! calls the `Game` code it names and adds no logic.
pub(super) mod data;
#[path = "command_host/entity.rs"]
mod entity;
#[path = "command_host/fillbiome.rs"]
mod fillbiome;
#[path = "command_host/forceload.rs"]
mod forceload;
#[path = "command_host/gamerule.rs"]
mod gamerule;
#[path = "command_host/help.rs"]
mod help;
mod misc;
#[path = "command_host/profiler.rs"]
mod profiler;
pub(super) mod profiles;
#[path = "command_host/publish.rs"]
mod publish;
#[path = "command_host/recipe.rs"]
mod recipe;
#[path = "command_host/rotate.rs"]
mod rotate;
#[path = "command_host/sound.rs"]
mod sound;
#[path = "command_host/swing.rs"]
mod swing;
#[path = "command_host/tag.rs"]
pub(super) mod tag;
#[path = "command_host/time.rs"]
mod time;
#[path = "command_host/version.rs"]
mod version;
#[path = "command_host/weather.rs"]
mod weather;
#[path = "command_host/worldborder.rs"]
mod worldborder;

use super::Game;
use horizon_client::{command::CommandError, text::Text};
use horizon_gameplay::commands::host::CommandHost;

impl CommandHost for Game<'_> {
    fn error(&self, key: &str, args: &[&dyn std::fmt::Display]) -> CommandError {
        super::err(key, args)
    }
    fn feedback(&mut self, message: Text) {
        Game::feedback(self, message);
    }
    fn feedback_unbroadcast(&mut self, message: Text) {
        Game::feedback_unbroadcast(self, message);
    }
}
