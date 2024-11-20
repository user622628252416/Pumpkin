use async_trait::async_trait;
use pumpkin_core::text::TextComponent;

use crate::command::args::arg_nbt::NbtArgConsumer;
use crate::command::args::arg_position_3d::Position3DArgumentConsumer;
use crate::command::args::arg_summonable_entity::SummonableEntityArgConsumer;
use crate::command::args::{ConsumedArgs, FindArgDefaultName};
use crate::command::tree::CommandTree;
use crate::command::tree_builder::{argument_default_name, require};
use crate::command::{CommandError, CommandExecutor, CommandSender};
use crate::entity::player::PermissionLvl;

const NAMES: [&str; 1] = ["summon"];

const DESCRIPTION: &str = "Summons entities.";

struct SummonExecutor;

#[async_trait]
impl CommandExecutor for SummonExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender<'a>,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let _entity = SummonableEntityArgConsumer.find_arg_default_name(args)?;
    
        let _pos = match Position3DArgumentConsumer.find_optional_arg_default_name(args) {
            Some(pos) => pos?,
            None => match sender.position() {
                Some(pos) => pos,
                None => return Err(CommandError::InvalidRequirement),
            }
        };

        let _nbt = match NbtArgConsumer.find_optional_arg_default_name(args) {
            Some(nbt) => nbt?,
            None => "",
        };


        sender
            .send_message(TextComponent::text("Entites are unfortunately not implemented yet."))
            .await;

        Ok(())
    }
}

pub fn init_command_tree<'a>() -> CommandTree<'a> {
    CommandTree::new(NAMES, DESCRIPTION).with_child(
        require(&|sender| sender.has_permission_lvl(PermissionLvl::Two)).with_child(
            argument_default_name(&SummonableEntityArgConsumer).with_child(
                require(&|sender| sender.is_player()).execute(&SummonExecutor)
            ).with_child(
                argument_default_name(&Position3DArgumentConsumer).with_child(
                    argument_default_name(&NbtArgConsumer)
                        .execute(&SummonExecutor)
                ).execute(&SummonExecutor)
            ),
        ),
    )
}
