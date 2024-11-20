use std::sync::Arc;

use async_trait::async_trait;
use pumpkin_macros::find_arg;
use pumpkin_protocol::client::play::{
    CommandSuggestion, ProtoCmdArgParser, ProtoCmdArgSuggestionType,
};

use crate::command::dispatcher::CommandError;
use crate::command::tree::RawArgs;
use crate::command::CommandSender;
use crate::server::Server;
use crate::entity::player::Player;

use super::super::args::ArgumentConsumer;
use super::{Arg, DefaultNameArgConsumer, GetClientSideArgParser};

/// Select zero, one or multiple players
#[find_arg(&'a [Arc<Player>], Arg::Players(data) => data)]
pub(crate) struct PlayersArgumentConsumer;

impl GetClientSideArgParser for PlayersArgumentConsumer {
    fn get_client_side_parser(&self) -> ProtoCmdArgParser {
        // todo: investigate why this does not accept target selectors
        ProtoCmdArgParser::Entity {
            flags: ProtoCmdArgParser::ENTITY_FLAG_PLAYERS_ONLY,
        }
    }

    fn get_client_side_suggestion_type_override(&self) -> Option<ProtoCmdArgSuggestionType> {
        None
    }
}

#[async_trait]
impl ArgumentConsumer for PlayersArgumentConsumer {
    async fn consume<'a>(
        &self,
        src: &CommandSender<'a>,
        server: &'a Server,
        args: &mut RawArgs<'a>,
    ) -> Option<Arg<'a>> {
        let s = args.pop()?;

        let players = match s {
            "@s" => match src {
                CommandSender::Player(p) => Some(vec![p.clone()]),
                _ => None,
            },
            #[allow(clippy::match_same_arms)]
            // todo: implement for non-players and remove this line
            "@n" | "@p" => match src {
                CommandSender::Player(p) => Some(vec![p.clone()]),
                // todo: implement for non-players: how should this behave when sender is console/rcon?
                _ => None,
            },
            "@r" => {
                if let Some(p) = server.get_random_player().await {
                    Some(vec![p.clone()])
                } else {
                    Some(vec![])
                }
            }
            "@a" | "@e" => Some(server.get_all_players().await),
            name => server.get_player_by_name(name).await.map(|p| vec![p]),
        };

        players.map(Arg::Players)
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

impl DefaultNameArgConsumer for PlayersArgumentConsumer {
    fn default_name(&self) -> &'static str {
        "player"
    }
}
