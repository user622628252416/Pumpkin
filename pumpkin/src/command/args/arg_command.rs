use async_trait::async_trait;
use pumpkin_macros::find_arg;
use pumpkin_protocol::client::play::{
    CommandSuggestion, ProtoCmdArgParser, ProtoCmdArgSuggestionType, StringProtoArgBehavior,
};

use crate::{
    command::{
        args::SplitSingleWhitespaceIncludingEmptyParts,
        dispatcher::CommandError,
        tree::{CommandTree, RawArgs},
        CommandSender,
    },
    server::Server,
};

use super::{Arg, ArgumentConsumer, DefaultNameArgConsumer, GetClientSideArgParser};

#[find_arg(&'a CommandTree<'a>, Arg::CommandTree(data) => data)]
pub(crate) struct CommandTreeArgumentConsumer;

impl GetClientSideArgParser for CommandTreeArgumentConsumer {
    fn get_client_side_parser(&self) -> ProtoCmdArgParser {
        ProtoCmdArgParser::String(StringProtoArgBehavior::SingleWord)
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<ProtoCmdArgSuggestionType> {
        Some(ProtoCmdArgSuggestionType::AskServer)
    }
}

#[async_trait]
impl ArgumentConsumer for CommandTreeArgumentConsumer {
    async fn consume<'a>(
        &self,
        _sender: &CommandSender<'a>,
        server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> Option<Arg<'a>> {
        let s = args.pop()?;

        let dispatcher = &server.command_dispatcher;
        return match dispatcher.get_tree(s) {
            Ok(tree) => Some(Arg::CommandTree(tree)),
            Err(_) => None,
        };
    }

    async fn suggest<'a>(
        &self,
        _sender: &CommandSender<'a>,
        server: &'a Server,
        input: &'a str,
    ) -> Result<Option<Vec<CommandSuggestion<'a>>>, CommandError> {
        let Some(input) = input.split_single_whitespace_including_empty_parts().last() else {
            return Ok(None);
        };

        let suggestions = server
            .command_dispatcher
            .commands
            .keys()
            .filter(|suggestion| suggestion.starts_with(input))
            .map(|suggestion| CommandSuggestion::new(suggestion, None))
            .collect();
        Ok(Some(suggestions))
    }
}

impl DefaultNameArgConsumer for CommandTreeArgumentConsumer {
    fn default_name(&self) -> &'static str {
        "cmd"
    }
}
