use async_trait::async_trait;
use pumpkin_util::text::TextComponent;

use crate::command::{
    CommandExecutor, CommandSender,
    args::{
        Arg, ConsumedArgs, FindArg,
        bounded_num::{BoundedNumArgumentConsumer, Number},
        entity::EntityArgumentConsumer,
        resource::attribute::AttributeArgumentConsumer,
        resource_location::ResourceLocationArgumentConsumer,
    },
    dispatcher::CommandError,
    tree::{
        CommandTree,
        builder::{argument, literal},
    },
};

const NAMES: [&str; 1] = ["attribute"];
const DESCRIPTION: &str = "Read and write entity attributes";

const ARG_TARGET: &str = "target";
const ARG_ATTRIBUTE: &str = "attribute";
const ARG_SCALE: &str = "scale";
const ARG_ID: &str = "id";
const ARG_VALUE: &str = "value";

struct GetExecutor {
    base_value_only: bool,
}

#[async_trait]
impl CommandExecutor for GetExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let target = EntityArgumentConsumer::find_arg(args, ARG_TARGET)?;

        let attribute = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;

        let scale = match args.get(ARG_SCALE) {
            // default value
            None => Ok(1.0),
            // explicit value
            Some(Arg::Num(Ok(Number::F64(val)))) => Ok(*val),
            // explicit value out of bounds
            Some(Arg::Num(Err(e))) => Err(CommandError::from(*e)),
            // should never happen
            Some(_) => Err(CommandError::InvalidConsumption(Some(
                ARG_SCALE.to_string(),
            ))),
        }?;

        // todo
        let target_name = target.get_name().get_text();
        let is_base = self.base_value_only;
        sender.send_message(TextComponent::text(format!(
            "GetExecutor: is_base: {is_base:?}, target: {target_name:?}, attribute: {attribute}, scale: {scale}"
        ))).await;

        Ok(())
    }
}

struct ResetBaseValueExecutor;

#[async_trait]
impl CommandExecutor for ResetBaseValueExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let target = EntityArgumentConsumer::find_arg(args, ARG_TARGET)?;
        let attribute = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;

        // todo
        let target_name = target.get_name().get_text();
        sender
            .send_message(TextComponent::text(format!(
                "ResetBaseValueExecutor: target: {target_name:?}, attribute: {attribute}"
            )))
            .await;

        Ok(())
    }
}

struct SetBaseValueExecutor;

#[async_trait]
impl CommandExecutor for SetBaseValueExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let target = EntityArgumentConsumer::find_arg(args, ARG_TARGET)?;
        let attribute = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;
        let value = BoundedNumArgumentConsumer::<f64>::find_arg(args, ARG_VALUE)??;

        // todo
        let target_name = target.get_name().get_text();
        sender.send_message(TextComponent::text(format!(
            "SetBaseValueExecutor: target: {target_name:?}, attribute: {attribute}, value: {value}"
        ))).await;

        Ok(())
    }
}

/// How an attribute modifier modifies the attributes base value
#[derive(Debug, Copy, Clone)]
#[allow(clippy::enum_variant_names)]
enum ModifierOperation {
    AddValue,
    AddMultipliedBase,
    AddMultipliedTotal,
}

struct AddModifierExecutor(ModifierOperation);

#[async_trait]
impl CommandExecutor for AddModifierExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let target = EntityArgumentConsumer::find_arg(args, ARG_TARGET)?;
        let attribute = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;
        let value = BoundedNumArgumentConsumer::<f64>::find_arg(args, ARG_VALUE)??;
        let id = ResourceLocationArgumentConsumer::find_arg(args, ARG_ID)?;
        let operation = self.0;

        // todo
        let target_name = target.get_name().get_text();
        sender.send_message(TextComponent::text(format!(
            "AddModifierExecutor: type: {operation:?}, target: {target_name:?}, attribute: {attribute}, value: {value}, id: {id}"
        ))).await;

        Ok(())
    }
}

struct RemoveModifierExecutor;

#[async_trait]
impl CommandExecutor for RemoveModifierExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let target = EntityArgumentConsumer::find_arg(args, ARG_TARGET)?;

        let attribute = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;

        let id = ResourceLocationArgumentConsumer::find_arg(args, ARG_ID)?;

        // todo
        let target_name = target.get_name().get_text();
        sender
            .send_message(TextComponent::text(format!(
                "RemoveModifierExecutor: id: {id}, target: {target_name:?}, attribute: {attribute}"
            )))
            .await;

        Ok(())
    }
}

struct GetModifierExecutor;

#[async_trait]
impl CommandExecutor for GetModifierExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let target = EntityArgumentConsumer::find_arg(args, ARG_TARGET)?;

        let attribute = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;

        let scale = match args.get(ARG_SCALE) {
            // default value
            None => Ok(1.0),
            // explicit value
            Some(Arg::Num(Ok(Number::F64(val)))) => Ok(*val),
            // explicit value out of bounds
            Some(Arg::Num(Err(e))) => Err(CommandError::from(*e)),
            // should never happen
            Some(_) => Err(CommandError::InvalidConsumption(Some(
                ARG_SCALE.to_string(),
            ))),
        }?;

        let id = ResourceLocationArgumentConsumer::find_arg(args, ARG_ID)?;

        // todo
        let target_name = target.get_name().get_text();
        sender.send_message(TextComponent::text(format!(
            "GetModifierExecutor: id: {id}, target: {target_name:?}, attribute: {attribute}, scale: {scale}"
        ))).await;

        Ok(())
    }
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION).then(
        argument(ARG_TARGET, EntityArgumentConsumer).then(
            argument(ARG_ATTRIBUTE, AttributeArgumentConsumer)
                .then(
                    literal("get")
                        .then(
                            argument(ARG_SCALE, BoundedNumArgumentConsumer::<f64>::new()).execute(
                                GetExecutor {
                                    base_value_only: false,
                                },
                            ),
                        )
                        .execute(GetExecutor {
                            base_value_only: false,
                        }),
                )
                .then(
                    literal("base")
                        .then(
                            literal("get")
                                .then(argument(
                                    ARG_SCALE,
                                    BoundedNumArgumentConsumer::<f64>::new(),
                                ))
                                .execute(GetExecutor {
                                    base_value_only: true,
                                }),
                        )
                        .execute(GetExecutor {
                            base_value_only: true,
                        })
                        .then(
                            literal("set").then(
                                argument(ARG_VALUE, BoundedNumArgumentConsumer::<f64>::new())
                                    .execute(SetBaseValueExecutor),
                            ),
                        )
                        .then(literal("reset").execute(ResetBaseValueExecutor)),
                )
                .then(
                    literal("modifier")
                        .then(
                            literal("add").then(
                                argument(ARG_ID, ResourceLocationArgumentConsumer::new(true)).then(
                                    argument(ARG_VALUE, BoundedNumArgumentConsumer::<f64>::new())
                                        .then(literal("add_value").execute(AddModifierExecutor(
                                            ModifierOperation::AddValue,
                                        )))
                                        .then(literal("add_multiplied_base").execute(
                                            AddModifierExecutor(
                                                ModifierOperation::AddMultipliedBase,
                                            ),
                                        ))
                                        .then(literal("add_multiplied_total").execute(
                                            AddModifierExecutor(
                                                ModifierOperation::AddMultipliedTotal,
                                            ),
                                        )),
                                ),
                            ),
                        )
                        .then(
                            literal("remove").then(
                                argument(ARG_ID, ResourceLocationArgumentConsumer::new(true))
                                    .execute(RemoveModifierExecutor),
                            ),
                        )
                        .then(
                            literal("value").then(
                                literal("get").then(
                                    argument(ARG_ID, ResourceLocationArgumentConsumer::new(true))
                                        .then(
                                            argument(
                                                ARG_ID,
                                                ResourceLocationArgumentConsumer::new(true),
                                            )
                                            .then(
                                                argument(
                                                    ARG_SCALE,
                                                    BoundedNumArgumentConsumer::<f64>::new(),
                                                )
                                                .execute(GetModifierExecutor),
                                            )
                                            .execute(GetModifierExecutor),
                                        ),
                                ),
                            ),
                        ),
                ),
        ),
    )
}
