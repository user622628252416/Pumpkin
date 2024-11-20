use async_trait::async_trait;
use pumpkin_macros::find_arg;
use pumpkin_protocol::client::play::{
    CommandSuggestion, ProtoCmdArgParser, ProtoCmdArgSuggestionType, StringProtoArgBehavior,
};

use crate::{command::dispatcher::CommandError, server::Server};

use super::{
    super::{
        args::{ArgumentConsumer, RawArgs},
        CommandSender,
    },
    Arg, DefaultNameArgConsumer, GetClientSideArgParser,
};

/// Consumes all remaining words/args. Does not consume if there is no word.
#[find_arg(&'a str, Arg::Msg(data) => data)]
pub(crate) struct MsgArgConsumer;

impl GetClientSideArgParser for MsgArgConsumer {
    fn get_client_side_parser(&self) -> ProtoCmdArgParser {
        ProtoCmdArgParser::String(StringProtoArgBehavior::GreedyPhrase)
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<ProtoCmdArgSuggestionType> {
        None
    }
}

#[async_trait]
impl ArgumentConsumer for MsgArgConsumer {
    async fn consume<'a>(
        &self,
        _sender: &CommandSender<'a>,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> Option<Arg<'a>> {
        let mut msg = args.pop()?.to_string();

        while let Some(word) = args.pop() {
            msg.push(' ');
            msg.push_str(word);
        }

        Some(Arg::Msg(msg))
    }

    async fn suggest<'a>(
        &self,
        _sender: &CommandSender<'a>,
        _server: &'a Server,
        _input: &'a str,
    ) -> Result<Option<Vec<CommandSuggestion<'a>>>, CommandError> {
        Ok(None)
    }
}

impl DefaultNameArgConsumer for MsgArgConsumer {
    fn default_name(&self) -> &'static str {
        "msg"
    }
}
