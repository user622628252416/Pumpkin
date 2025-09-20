use async_trait::async_trait;
use pumpkin_data::attributes::Attribute;
use pumpkin_protocol::java::client::play::{ArgumentType, CommandSuggestion, SuggestionProviders};

use crate::{
    command::{
        CommandSender,
        args::{Arg, ArgumentConsumer, ConsumedArgs, FindArg, GetClientSideArgParser},
        dispatcher::CommandError,
        tree::RawArgs,
    },
    server::Server,
};

pub struct AttributeArgumentConsumer;

impl GetClientSideArgParser for AttributeArgumentConsumer {
    fn get_client_side_parser(&self) -> ArgumentType<'_> {
        ArgumentType::Resource {
            identifier: "attribute",
        }
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<SuggestionProviders> {
        None
    }
}

#[async_trait]
impl ArgumentConsumer for AttributeArgumentConsumer {
    async fn consume<'a>(
        &'a self,
        _sender: &CommandSender,
        _server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> Option<Arg<'a>> {
        let mut attribute_name = args.pop()?.to_string();

        if !attribute_name.contains(':') {
            attribute_name = format!("minecraft:{}", &attribute_name);
        }
        let attribute = Attribute::find_by_name(&attribute_name)?;

        Some(Arg::Attribute(attribute_name, attribute))
    }

    async fn suggest<'a>(
        &'a self,
        _sender: &CommandSender,
        _server: &'a Server,
        _input: &'a str,
    ) -> Result<Option<Vec<CommandSuggestion>>, CommandError> {
        Ok(None)
    }
}

impl<'a> FindArg<'a> for AttributeArgumentConsumer {
    type Data = (&'a str, Attribute);

    fn find_arg(args: &'a ConsumedArgs, name: &str) -> Result<Self::Data, CommandError> {
        match args.get(name) {
            Some(Arg::Attribute(name, attr)) => Ok((name, *attr)),
            _ => Err(CommandError::InvalidConsumption(Some(name.to_string()))),
        }
    }
}
