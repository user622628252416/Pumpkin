use async_trait::async_trait;
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

pub(crate) struct NbtArgConsumer;

impl GetClientSideArgParser for NbtArgConsumer {
    fn get_client_side_parser(&self) -> ProtoCmdArgParser {
        ProtoCmdArgParser::Nbt
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<ProtoCmdArgSuggestionType> {
        None
    }
}

#[async_trait]
impl ArgumentConsumer for NbtArgConsumer {
    async fn consume<'a>(
        &self,
        _sender: &CommandSender<'a>,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> Option<Arg<'a>> {
        let nbt = args.pop()?.to_string();

        Some(Arg::Nbt(nbt))
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

impl DefaultNameArgConsumer for NbtArgConsumer {
    fn default_name(&self) -> &'static str {
        "nbt"
    }

    fn get_argument_consumer(&self) -> &dyn ArgumentConsumer {
        &NbtArgConsumer
    }
}

impl<'a> FindArg<'a> for NbtArgConsumer {
    type Data = &'a str;

    fn find_optional_arg(
        args: &'a super::ConsumedArgs,
        name: &'a str,
    ) -> Option<Result<Self::Data, CommandError>> {
        match args.get(name) {
            Some(Arg::Nbt(data)) => Some(Ok(data)),
            Some(_) => Some(Err(CommandError::InvalidConsumption(Some(
                name.to_string(),
            )))),
            None => None,
        }
    }
}
