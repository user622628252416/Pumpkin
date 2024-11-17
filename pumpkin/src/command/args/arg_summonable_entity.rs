use async_trait::async_trait;
use pumpkin_macros::find_arg;
use pumpkin_protocol::client::play::{
    CommandSuggestion, ProtoCmdArgParser, ProtoCmdArgSuggestionType,
};

use crate::{command::dispatcher::CommandError, server::Server};

use super::{
    super::{
        args::{ArgumentConsumer, RawArgs},
        CommandSender,
    },
    Arg, DefaultNameArgConsumer, FindArg, GetClientSideArgParser,
};

#[find_arg(&'a str, Arg::SummonableEntity(data) => data)]
pub(crate) struct SummonableEntityArgConsumer;

impl GetClientSideArgParser for SummonableEntityArgConsumer {
    fn get_client_side_parser(&self) -> ProtoCmdArgParser {
        ProtoCmdArgParser::Resource {
            identifier: "entity_type",
        }
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<ProtoCmdArgSuggestionType> {
        None
    }
}

#[async_trait]
impl ArgumentConsumer for SummonableEntityArgConsumer {
    async fn consume<'a>(
        &self,
        _sender: &CommandSender<'a>,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> Option<Arg<'a>> {
        let entity = args.pop()?.to_string();


        Some(Arg::SummonableEntity(entity))
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

impl DefaultNameArgConsumer for SummonableEntityArgConsumer {
    fn default_name(&self) -> &'static str {
        "entity"
    }

    fn get_argument_consumer(&self) -> &dyn ArgumentConsumer {
        &SummonableEntityArgConsumer
    }
}
