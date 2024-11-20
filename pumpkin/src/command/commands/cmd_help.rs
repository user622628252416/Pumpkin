use async_trait::async_trait;
use pumpkin_core::text::TextComponent;

use crate::command::args::arg_command::CommandTreeArgumentConsumer;
use crate::command::args::ConsumedArgs;
use crate::command::dispatcher::CommandError;
use crate::command::tree::{Command, CommandTree};
use crate::command::tree_builder::argument;
use crate::command::{CommandExecutor, CommandSender};
use crate::server::Server;
use crate::command::args::FindArg;

const NAMES: [&str; 3] = ["help", "h", "?"];

const DESCRIPTION: &str = "Print a help message.";

const ARG_COMMAND: &str = "command";

struct CommandHelpExecutor;

#[async_trait]
impl CommandExecutor for CommandHelpExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender<'a>,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {

        match CommandTreeArgumentConsumer::find_optional_arg(args, ARG_COMMAND)? {
            Some(tree) => {
                sender.send_message(
                        TextComponent::text_string(format!(
                        "{} - {} Usage: {}",
                        tree.names.join("/"),
                        tree.description,
                        tree
                    ))
                ).await;
            },
            None => {
                let mut keys: Vec<&str> = server.command_dispatcher.commands.keys().copied().collect();
                keys.sort_unstable();
        
                for key in keys {
                    let Command::Tree(tree) = &server.command_dispatcher.commands[key] else {
                        continue;
                    };
        
                    sender
                        .send_message(TextComponent::text(&format!(
                            "{} - {} Usage: {}",
                            tree.names.join("/"),
                            tree.description,
                            tree
                        )))
                        .await;
                }
            }
        };

        Ok(())
    }
}

pub fn init_command_tree<'a>() -> CommandTree<'a> {
    CommandTree::new(NAMES, DESCRIPTION)
        .with_child(
            argument(ARG_COMMAND, &CommandTreeArgumentConsumer).execute(&CommandHelpExecutor),
        )
        .execute(&CommandHelpExecutor)
}
