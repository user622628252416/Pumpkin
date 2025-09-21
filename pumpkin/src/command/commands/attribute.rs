use async_trait::async_trait;
use pumpkin_data::data_component_impl::Operation;
use pumpkin_util::text::TextComponent;

use crate::{
    command::{
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
    },
    entity::attribute_manager::AttributeError,
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

        let Some(target_as_living_entity) = target.get_living_entity() else {
            return Err(CommandError::CommandFailed(Box::new(
                TextComponent::translate("commands.attribute.failed.entity", [target.get_name()]),
            )));
        };

        let (attribute_name, attribute) = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;

        // todo: if /execute is ever fully implemented, this agrument becomes relevant
        let _scale = match args.get(ARG_SCALE) {
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

        if self.base_value_only {
            let Ok(base_value) = target_as_living_entity
                .attribute_manager
                .get_base(attribute)
            else {
                return Err(CommandError::CommandFailed(Box::new(
                    TextComponent::translate(
                        "commands.attribute.failed.no_attribute",
                        [
                            target.get_name(),
                            TextComponent::text(attribute_name.to_string()),
                        ],
                    ),
                )));
            };
            sender
                .send_message(TextComponent::translate(
                    "commands.attribute.base_value.get.success",
                    [
                        TextComponent::text(attribute_name.to_string()),
                        target.get_name(),
                        TextComponent::text(base_value.to_string()),
                    ],
                ))
                .await;
        } else {
            let Ok(value) = target_as_living_entity
                .attribute_manager
                .get_modified(attribute)
                .await
            else {
                return Err(CommandError::CommandFailed(Box::new(
                    TextComponent::translate(
                        "commands.attribute.failed.no_attribute",
                        [
                            target.get_name(),
                            TextComponent::text(attribute_name.to_string()),
                        ],
                    ),
                )));
            };
            sender
                .send_message(TextComponent::translate(
                    "commands.attribute.value.get.success",
                    [
                        TextComponent::text(attribute_name.to_string()),
                        target.get_name(),
                        TextComponent::text(value.to_string()),
                    ],
                ))
                .await;
        }

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

        let Some(target_as_living_entity) = target.get_living_entity() else {
            return Err(CommandError::CommandFailed(Box::new(
                TextComponent::translate("commands.attribute.failed.entity", [target.get_name()]),
            )));
        };

        let (attribute_name, attribute) = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;

        let Ok(value) = target_as_living_entity
            .attribute_manager
            .reset_base(&target_as_living_entity.entity, attribute)
            .await
        else {
            return Err(CommandError::CommandFailed(Box::new(
                TextComponent::translate(
                    "commands.attribute.failed.no_attribute",
                    [
                        target.get_name(),
                        TextComponent::text(attribute_name.to_string()),
                    ],
                ),
            )));
        };
        sender
            .send_message(TextComponent::translate(
                "commands.attribute.base_value.reset.success",
                [
                    TextComponent::text(attribute_name.to_string()),
                    target.get_name(),
                    TextComponent::text(value.to_string()),
                ],
            ))
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

        let Some(target_as_living_entity) = target.get_living_entity() else {
            return Err(CommandError::CommandFailed(Box::new(
                TextComponent::translate("commands.attribute.failed.entity", [target.get_name()]),
            )));
        };

        let (attribute_name, attribute) = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;

        let value = BoundedNumArgumentConsumer::<f64>::find_arg(args, ARG_VALUE)??;

        if target_as_living_entity
            .attribute_manager
            .set_base(&target_as_living_entity.entity, attribute, value)
            .await
            .is_err()
        {
            return Err(CommandError::CommandFailed(Box::new(
                TextComponent::translate(
                    "commands.attribute.failed.no_attribute",
                    [
                        target.get_name(),
                        TextComponent::text(attribute_name.to_string()),
                    ],
                ),
            )));
        };
        sender
            .send_message(TextComponent::translate(
                "commands.attribute.base_value.set.success",
                [
                    TextComponent::text(attribute_name.to_string()),
                    target.get_name(),
                    TextComponent::text(value.to_string()),
                ],
            ))
            .await;

        Ok(())
    }
}

struct AddModifierExecutor(Operation);

#[async_trait]
impl CommandExecutor for AddModifierExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let target = EntityArgumentConsumer::find_arg(args, ARG_TARGET)?;

        let Some(target_as_living_entity) = target.get_living_entity() else {
            return Err(CommandError::CommandFailed(Box::new(
                TextComponent::translate("commands.attribute.failed.entity", [target.get_name()]),
            )));
        };

        let (attribute_name, attribute) = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;
        let value = BoundedNumArgumentConsumer::<f64>::find_arg(args, ARG_VALUE)??;
        let id = ResourceLocationArgumentConsumer::find_arg(args, ARG_ID)?;
        let op = self.0;

        match target_as_living_entity
            .attribute_manager
            .add_modifier(
                &target_as_living_entity.entity,
                attribute,
                id.to_owned(),
                value,
                op,
            )
            .await
        {
            Ok(()) => {
                sender
                    .send_message(TextComponent::translate(
                        "commands.attribute.modifier.add.success",
                        [
                            TextComponent::text(id.to_owned()),
                            TextComponent::text(attribute_name.to_owned()),
                            target.get_name(),
                        ],
                    ))
                    .await;
            }
            Err(AttributeError::AttributeNotFound) => {
                return Err(CommandError::CommandFailed(Box::new(
                    TextComponent::translate(
                        "commands.attribute.failed.no_attribute",
                        [
                            target.get_name(),
                            TextComponent::text(attribute_name.to_owned()),
                        ],
                    ),
                )));
            }
            Err(AttributeError::ModifierAlreadyExists(attr)) => {
                return Err(CommandError::CommandFailed(Box::new(
                    TextComponent::translate(
                        "commands.attribute.failed.modifier_already_present",
                        [
                            TextComponent::text(id.to_owned()),
                            TextComponent::text(format!("{:?}", attr)),
                            target.get_name(),
                        ],
                    ),
                )));
            }
            Err(AttributeError::ModifierDoesNotExist) => unreachable!(),
        }

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

        let Some(target_as_living_entity) = target.get_living_entity() else {
            return Err(CommandError::CommandFailed(Box::new(
                TextComponent::translate("commands.attribute.failed.entity", [target.get_name()]),
            )));
        };

        let (attribute_name, attribute) = AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;

        let id = ResourceLocationArgumentConsumer::find_arg(args, ARG_ID)?;

        match target_as_living_entity
            .attribute_manager
            .remove_modifier(&target_as_living_entity.entity, attribute, id)
            .await
        {
            Ok(()) => {
                sender
                    .send_message(TextComponent::translate(
                        "commands.attribute.modifier.remove.success",
                        [
                            TextComponent::text(id.to_owned()),
                            TextComponent::text(attribute_name.to_owned()),
                            target.get_name(),
                        ],
                    ))
                    .await;
            }
            Err(AttributeError::AttributeNotFound) => {
                return Err(CommandError::CommandFailed(Box::new(
                    TextComponent::translate(
                        "commands.attribute.failed.no_attribute",
                        [
                            target.get_name(),
                            TextComponent::text(attribute_name.to_owned()),
                        ],
                    ),
                )));
            }
            Err(AttributeError::ModifierDoesNotExist) => {
                return Err(CommandError::CommandFailed(Box::new(
                    TextComponent::translate(
                        "commands.attribute.failed.no_modifier",
                        [
                            TextComponent::text(attribute_name.to_owned()),
                            target.get_name(),
                            TextComponent::text(id.to_owned()),
                        ],
                    ),
                )));
            }
            Err(AttributeError::ModifierAlreadyExists(_)) => unreachable!(),
        };

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

        let Some(target_as_living_entity) = target.get_living_entity() else {
            return Err(CommandError::CommandFailed(Box::new(
                TextComponent::translate("commands.attribute.failed.entity", [target.get_name()]),
            )));
        };

        let (attribute_name, _attribute) =
            AttributeArgumentConsumer::find_arg(args, ARG_ATTRIBUTE)?;

        // todo: if /execute is ever fully implemented, this agrument becomes relevant
        let _scale = match args.get(ARG_SCALE) {
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

        match target_as_living_entity
            .attribute_manager
            .get_modifier_value(id)
            .await
        {
            Ok(value) => {
                sender
                    .send_message(TextComponent::translate(
                        "commands.attribute.modifier.value.get.success",
                        [
                            TextComponent::text(id.to_owned()),
                            TextComponent::text(attribute_name.to_owned()),
                            target.get_name(),
                            TextComponent::text(value.to_string()),
                        ],
                    ))
                    .await;
            }
            Err(AttributeError::AttributeNotFound) => {
                return Err(CommandError::CommandFailed(Box::new(
                    TextComponent::translate(
                        "commands.attribute.failed.no_attribute",
                        [
                            target.get_name(),
                            TextComponent::text(attribute_name.to_owned()),
                        ],
                    ),
                )));
            }
            Err(AttributeError::ModifierDoesNotExist) => {
                return Err(CommandError::CommandFailed(Box::new(
                    TextComponent::translate(
                        "commands.attribute.failed.no_modifier",
                        [
                            TextComponent::text(attribute_name.to_owned()),
                            target.get_name(),
                            TextComponent::text(id.to_owned()),
                        ],
                    ),
                )));
            }
            Err(AttributeError::ModifierAlreadyExists(_)) => unreachable!(),
        };

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
                                        .then(
                                            literal("add_value")
                                                .execute(AddModifierExecutor(Operation::AddValue)),
                                        )
                                        .then(literal("add_multiplied_base").execute(
                                            AddModifierExecutor(Operation::AddMultipliedBase),
                                        ))
                                        .then(literal("add_multiplied_total").execute(
                                            AddModifierExecutor(Operation::AddMultipliedTotal),
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
    )
}
