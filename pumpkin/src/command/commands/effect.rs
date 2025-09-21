use crate::TextComponent;
use crate::command::args::bool::BoolArgConsumer;
use crate::command::args::bounded_num::BoundedNumArgumentConsumer;
use crate::command::args::entities::EntitiesArgumentConsumer;
use crate::command::args::resource::effect::EffectTypeArgumentConsumer;
use crate::command::args::{Arg, ConsumedArgs, FindArg, FindArgDefaultName};
use crate::command::dispatcher::CommandError;
use crate::command::dispatcher::CommandError::InvalidConsumption;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::{argument, literal};
use crate::command::{CommandExecutor, CommandSender};
use crate::server::Server;
use async_trait::async_trait;
use pumpkin_data::potion::Effect;
use pumpkin_util::text::color::{Color, NamedColor};

const NAMES: [&str; 1] = ["effect"];

const DESCRIPTION: &str = "Adds or removes the status effects of players and other entities.";

const ARG_CLEAR: &str = "clear";
const ARG_GIVE: &str = "give";
const ARG_EFFECT: &str = "effect";
const ARG_TARGETS: &str = "targets";
const ARG_SECOND: &str = "seconds";
const ARG_INFINITE: &str = "infinite";
const ARG_AMPLIFIER: &str = "amplifier";
const ARG_HIDE_PARTICLE: &str = "hideParticles";

enum Time {
    Base,
    Specified,
    Infinite,
}

enum Amplifier {
    Base,
    Specified,
}

struct GiveExecutor(Time, Amplifier, bool);

#[async_trait]
impl CommandExecutor for GiveExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let targets = EntitiesArgumentConsumer::find_arg(args, ARG_TARGETS)?;

        let Some(Arg::Effect(effect)) = args.get(ARG_EFFECT) else {
            return Err(InvalidConsumption(Some(ARG_EFFECT.into())));
        };

        //duration is in tick, so * 20 (not for the infinite because -1*20 cause visual glitch)
        let second = match self.0 {
            Time::Base => 30 * 20,
            Time::Specified => {
                BoundedNumArgumentConsumer::new()
                    .name("seconds")
                    .min(1)
                    .max(1_000_000)
                    .find_arg_default_name(args)??
                    * 20
            }
            Time::Infinite => -1,
        };

        let amplifier: i32 = match self.1 {
            Amplifier::Base => 0,
            Amplifier::Specified => BoundedNumArgumentConsumer::new()
                .name("amplifier")
                .min(0)
                .max(255)
                .find_arg_default_name(args)??,
        };

        let mut hide_particles = self.2;
        //if false -> parameter is referred
        if !hide_particles {
            let Some(Arg::Bool(hide_particle)) = args.get(ARG_HIDE_PARTICLE) else {
                return Err(InvalidConsumption(Some(ARG_HIDE_PARTICLE.into())));
            };

            hide_particles = *hide_particle;
        }

        let mut failed = 0;

        for target in targets {
            let Some(living_target) = target.get_living_entity() else {
                continue;
            };

            let current_effect_of_target = living_target.get_effect(effect).await;

            if current_effect_of_target
                .map(|e| e.amplifier as i32 >= amplifier)
                .unwrap_or(false)
            {
                failed += 1;
            } else {
                living_target
                    .add_effect(&Effect {
                        effect_type: effect,
                        duration: second,
                        amplifier: amplifier as u8,
                        ambient: false, //this is not a beacon effect
                        show_particles: hide_particles,
                        show_icon: true,
                        blend: true, //Currently only used in the DARKNESS effect to apply extra void fog and adjust the gamma value for lighting.
                    })
                    .await;
            }
        }

        let translation_name = TextComponent::translate(effect.translation_key.to_string(), []);

        if failed == targets.len() {
            sender
                .send_message(
                    TextComponent::translate("commands.effect.give.failed", [])
                        .color(Color::Named(NamedColor::Red)),
                )
                .await;
        } else if targets.len() == 1 {
            sender
                .send_message(TextComponent::translate(
                    "commands.effect.give.success.single",
                    [translation_name, targets[0].get_display_name().await],
                ))
                .await;
        } else {
            sender
                .send_message(TextComponent::translate(
                    "commands.effect.give.success.multiple",
                    [
                        translation_name,
                        TextComponent::text(targets.len().to_string()),
                    ],
                ))
                .await;
        }

        Ok(())
    }
}

struct ClearExecutor {
    /// true = delete every effect, false = only one
    clear_all: bool,
}

#[async_trait]
impl CommandExecutor for ClearExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let targets = EntitiesArgumentConsumer::find_arg(args, ARG_TARGETS)?;

        let effect;
        if self.clear_all {
            let mut effect_number = 0;
            for target in targets {
                let Some(living_target) = target.get_living_entity() else {
                    continue;
                };
                let effect_number_temp = living_target.remove_all_effects().await;
                if effect_number_temp > effect_number {
                    effect_number = effect_number_temp;
                }
            }

            //if the player or everyplayer don't have any effect
            if effect_number == 0 {
                sender
                    .send_message(
                        TextComponent::translate("commands.effect.clear.everything.failed", [])
                            .color(Color::Named(NamedColor::Red)),
                    )
                    .await;
            }
            //a player have at least 1 effect
            else if targets.len() == 1 {
                sender
                    .send_message(TextComponent::translate(
                        "commands.effect.clear.everything.success.single",
                        [targets[0].get_display_name().await],
                    ))
                    .await;
            } else {
                sender
                    .send_message(TextComponent::translate(
                        "commands.effect.clear.everything.success.multiple",
                        [TextComponent::text(targets.len().to_string())],
                    ))
                    .await;
            }
        } else {
            let Some(Arg::Effect(effect_type)) = args.get(ARG_EFFECT) else {
                return Err(InvalidConsumption(Some(ARG_EFFECT.into())));
            };

            effect = *effect_type;
            let mut has_effect = vec![];

            for target in targets {
                has_effect.push(match target.get_living_entity() {
                    None => false,
                    Some(living_entity) => living_entity.remove_effect(effect_type).await,
                });
            }

            if has_effect.contains(&false) {
                //contain false for 1 player == don't have
                if targets.len() == 1 || !has_effect.contains(&true) {
                    sender
                        .send_message(
                            TextComponent::translate("commands.effect.clear.specific.failed", [])
                                .color(Color::Named(NamedColor::Red)),
                        )
                        .await;
                }
            } else {
                //true for 1 player = have the effect
                if targets.len() == 1 {
                    sender
                        .send_message(TextComponent::translate(
                            "commands.effect.clear.specific.success.single",
                            [
                                TextComponent::translate(effect.translation_key, []),
                                targets[0].get_display_name().await,
                            ],
                        ))
                        .await;
                }
                //contain true for everyplayer = at least 1 player have the effect
                else {
                    sender
                        .send_message(TextComponent::translate(
                            "commands.effect.clear.specific.success.multiple",
                            [
                                TextComponent::translate(effect.translation_key, []),
                                TextComponent::text(targets.len().to_string()),
                            ],
                        ))
                        .await;
                }
            }
        }

        Ok(())
    }
}

#[allow(clippy::redundant_closure_for_method_calls)]
pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .then(
            literal(ARG_CLEAR).then(
                argument(ARG_TARGETS, EntitiesArgumentConsumer)
                    .execute(ClearExecutor { clear_all: true })
                    .then(
                        argument(ARG_EFFECT, EffectTypeArgumentConsumer)
                            .execute(ClearExecutor { clear_all: false }),
                    ),
            ),
        )
        .then(
            literal(ARG_GIVE).then(
                argument(ARG_TARGETS, EntitiesArgumentConsumer).then(
                    argument(ARG_EFFECT, EffectTypeArgumentConsumer)
                        .execute(GiveExecutor(Time::Base, Amplifier::Base, true))
                        //for specified time
                        .then(
                            argument(
                                ARG_SECOND,
                                BoundedNumArgumentConsumer::new()
                                    .name("seconds")
                                    .min(0)
                                    .max(1_000_000),
                            )
                            .execute(GiveExecutor(Time::Specified, Amplifier::Base, true))
                            .then(
                                argument(
                                    ARG_AMPLIFIER,
                                    BoundedNumArgumentConsumer::new()
                                        .name("amplifier")
                                        .min(0)
                                        .max(255),
                                )
                                .execute(GiveExecutor(Time::Specified, Amplifier::Specified, true))
                                .then(
                                    argument(ARG_HIDE_PARTICLE, BoolArgConsumer).execute(
                                        GiveExecutor(Time::Specified, Amplifier::Specified, false),
                                    ),
                                ),
                            ),
                        )
                        .then(
                            literal(ARG_INFINITE)
                                .execute(GiveExecutor(Time::Infinite, Amplifier::Base, true))
                                .then(
                                    argument(
                                        ARG_AMPLIFIER,
                                        BoundedNumArgumentConsumer::new()
                                            .name("amplifier")
                                            .min(0)
                                            .max(255),
                                    )
                                    .execute(GiveExecutor(
                                        Time::Infinite,
                                        Amplifier::Specified,
                                        true,
                                    ))
                                    .then(
                                        argument(ARG_HIDE_PARTICLE, BoolArgConsumer).execute(
                                            GiveExecutor(
                                                Time::Infinite,
                                                Amplifier::Specified,
                                                false,
                                            ),
                                        ),
                                    ),
                                ),
                        ),
                ),
            ),
        )
}
