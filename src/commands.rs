use anyhow::Result;
use twilight_model::application::interaction::application_command::CommandDataOption;

use crate::arguments::CommandOption;

pub trait Command: Send + Sync + 'static + Sized {
    /// Gets a list of options for this command
    fn options() -> Vec<CommandOption>;
    /// Converts a Vec of `CommandDataOption` into this command
    fn from_command_data(data: Vec<CommandDataOption>) -> Result<Self>;

    /// The command description as rendered in the discord client
    fn description() -> &'static str;
    /// The command's name
    fn name() -> &'static str;
}
