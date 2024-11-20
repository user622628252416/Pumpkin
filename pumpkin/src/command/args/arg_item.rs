use async_trait::async_trait;
use pumpkin_protocol::client::play::{
    CommandSuggestion, ProtoCmdArgParser, ProtoCmdArgSuggestionType,
};
use pumpkin_world::item::item_registry::{self, Item};

use crate::{command::dispatcher::CommandError, server::Server};

use super::{
    super::{
        args::{ArgumentConsumer, RawArgs},
        CommandSender,
    },
    Arg, DefaultNameArgConsumer, FindArg, GetClientSideArgParser,
};

pub(crate) struct ItemArgumentConsumer;

impl GetClientSideArgParser for ItemArgumentConsumer {
    fn get_client_side_parser(&self) -> ProtoCmdArgParser {
        ProtoCmdArgParser::Resource { identifier: "item" }
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<ProtoCmdArgSuggestionType> {
        None
    }
}

#[async_trait]
impl ArgumentConsumer for ItemArgumentConsumer {
    async fn consume<'a>(
        &self,
        _sender: &CommandSender<'a>,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> Option<Arg<'a>> {
        let s = args.pop()?;

        let name = if s.contains(':') {
            s.to_string()
        } else {
            format!("minecraft:{s}")
        };

        // todo: get an actual item
        Some(Arg::Item(name))
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

impl DefaultNameArgConsumer for ItemArgumentConsumer {
    fn default_name(&self) -> &'static str {
        "item"
    }
}

impl<'a> FindArg<'a> for ItemArgumentConsumer {
    type Data = &'a Item;

    fn find_optional_arg(args: &'a super::ConsumedArgs, name: &'a str) -> Result<Option<Self::Data>, CommandError> {
        match args.get(name) {
            Some(Arg::Item(name)) => match item_registry::get_item(name) {
                Some(item) => Ok(Some(item)),
                None => Err(CommandError::GeneralCommandIssue(format!(
                    "Item {name} does not exist."
                ))),
            },
            Some(_) => Err(CommandError::InvalidConsumption(Some(name.to_string()))),
            None => Ok(None),
        }
    }
}
