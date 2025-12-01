use std::{collections::HashMap, pin::Pin, sync::Arc};

use twilight_model::{
    application::{
        command::{Command, CommandType},
        interaction::{Interaction, InteractionContextType, InteractionData},
    },
    channel::message::MessageFlags,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
    oauth::ApplicationIntegrationType,
};
use twilight_util::builder::{
    command::CommandBuilder,
    message::{ContainerBuilder, TextDisplayBuilder},
};

type InteractionResult = anyhow::Result<InteractionResponse>;

type AsyncHandler<T> = Box<
    dyn Fn(Arc<Interaction>, Arc<T>) -> Pin<Box<dyn Future<Output = InteractionResult> + Send>>
        + Send
        + Sync,
>;

/// Commands that can be used via a context menu.
pub struct ContextCommands<T> {
    commands: HashMap<String, Arc<AsyncHandler<T>>>,
}

impl<S> ContextCommands<S> {
    /// Registers a context menu command.
    pub fn register<F, Fut>(&mut self, command: &str, handler: F)
    where
        F: Fn(Arc<Interaction>, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = InteractionResult> + Send + 'static,
    {
        let handler = Box::new(move |interaction, state| {
            Box::pin(handler(interaction, state))
                as Pin<Box<dyn Future<Output = InteractionResult> + Send>>
        });
        self.commands.insert(command.to_string(), Arc::new(handler));
    }

    /// Gets a registered context menu command.
    pub fn get(&self, name: &str) -> Option<&Arc<AsyncHandler<S>>> {
        self.commands.get(name)
    }

    /// Executes a context menu command if it exists.
    pub async fn execute(
        &self,
        interaction: Arc<Interaction>,
        state: Arc<S>,
    ) -> Option<InteractionResponse> {
        if let Some(InteractionData::ApplicationCommand(ref command)) = interaction.data
            && let Some(handler) = self.get(&command.name)
        {
            Some((handler)(interaction, state).await.unwrap_or_else(|e| {
                let container = ContainerBuilder::new()
                    .accent_color(Some(0xFF0000))
                    .component(TextDisplayBuilder::new(format!("An error occurred: {}", e)).build())
                    .build();
                InteractionResponse {
                    kind: InteractionResponseType::ChannelMessageWithSource,
                    data: Some(InteractionResponseData {
                        components: Some(vec![container.into()]),
                        flags: Some(MessageFlags::EPHEMERAL | MessageFlags::IS_COMPONENTS_V2),
                        ..Default::default()
                    }),
                }
            }))
        } else {
            None
        }
    }
}

impl<S> From<&ContextCommands<S>> for Vec<Command> {
    fn from(context_commands: &ContextCommands<S>) -> Vec<Command> {
        context_commands
            .commands
            .keys()
            .map(|name| {
                CommandBuilder::new(name, "", CommandType::Message)
                    .integration_types([
                        ApplicationIntegrationType::UserInstall,
                        ApplicationIntegrationType::GuildInstall,
                    ])
                    .contexts(vec![
                        InteractionContextType::Guild,
                        InteractionContextType::BotDm,
                        InteractionContextType::PrivateChannel,
                    ])
                    .build()
            })
            .collect()
    }
}

impl<S> Default for ContextCommands<S>
where
    S: Send + Sync + 'static,
{
    fn default() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }
}
