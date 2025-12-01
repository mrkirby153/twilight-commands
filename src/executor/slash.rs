use std::{collections::HashMap, fmt::Debug, marker::PhantomData, pin::Pin, sync::Arc};

use anyhow::Result;
use twilight_model::{
    application::{
        command::Command,
        interaction::{
            Interaction, InteractionContextType, application_command::CommandDataOption,
        },
    },
    channel::message::MessageFlags,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
};
use twilight_util::builder::{
    command::{CommandBuilder, SubCommandBuilder, SubCommandGroupBuilder},
    message::{ContainerBuilder, TextDisplayBuilder},
};

type CommandResponse = Result<InteractionResponse>;

trait AsyncHandler<S>: Send + Sync {
    fn handle(
        &self,
        interaction: Arc<Interaction>,
        interaction_data: Vec<CommandDataOption>,
        state: Arc<S>,
    ) -> Pin<Box<dyn Future<Output = CommandResponse> + Send>>;
}

struct TypedAsyncHandler<C, S, F, Fut>
where
    C: crate::commands::Command,
    F: Fn(C, Arc<Interaction>, Arc<S>) -> Fut + Send + Sync,
    Fut: Future<Output = CommandResponse> + Send + 'static,
    S: Send + Sync + 'static,
{
    handler: F,
    _phantom: PhantomData<(C, S)>,
}

impl<C: crate::commands::Command, S, F, Fut> AsyncHandler<S> for TypedAsyncHandler<C, S, F, Fut>
where
    F: Fn(C, Arc<Interaction>, Arc<S>) -> Fut + Send + Sync,
    Fut: Future<Output = CommandResponse> + Send + 'static,
    S: Send + Sync + 'static,
{
    fn handle(
        &self,
        interaction: Arc<Interaction>,
        interaction_data: Vec<CommandDataOption>,
        state: Arc<S>,
    ) -> Pin<Box<dyn Future<Output = CommandResponse> + Send>> {
        let command_data = C::from_command_data(interaction_data);
        let command_data = match command_data {
            Ok(data) => data,
            Err(_) => {
                return Box::pin(async {
                    Ok(InteractionResponse {
                        kind: twilight_model::http::interaction::InteractionResponseType::ChannelMessageWithSource,
                        data: Some(InteractionResponseData {
                            content: Some("Failed to parse command data.".to_string()),
                            flags: Some(MessageFlags::EPHEMERAL),
                            ..Default::default()
                        }),
                    })
                });
            }
        };

        let fut = (self.handler)(command_data, Arc::clone(&interaction), state);
        Box::pin(fut)
    }
}

struct CommandInfo<S> {
    handler: Box<dyn AsyncHandler<S>>,
    options: Vec<crate::arguments::CommandOption>,
    description: &'static str,
}

enum CommandTree<S>
where
    S: Send + Sync + 'static,
{
    Node(HashMap<String, CommandTree<S>>),
    Leaf(CommandInfo<S>),
}

impl<S> CommandTree<S>
where
    S: Send + Sync + 'static,
{
    fn new() -> Self {
        CommandTree::Node(HashMap::new())
    }

    fn insert(&mut self, path: &[String], info: CommandInfo<S>) {
        match self {
            CommandTree::Node(children) => {
                if path.is_empty() {
                    return;
                }
                let key = &path[0];
                if path.len() == 1 {
                    children.insert(key.clone(), CommandTree::Leaf(info));
                } else {
                    let child = children.entry(key.clone()).or_insert_with(CommandTree::new);
                    child.insert(&path[1..], info);
                }
            }
            CommandTree::Leaf(_) => {
                panic!("Cannot insert into a leaf node");
            }
        }
    }

    fn get(&self, path: &[String]) -> Option<&CommandInfo<S>> {
        match self {
            CommandTree::Node(children) => {
                if path.is_empty() {
                    return None;
                }
                let key = &path[0];
                let child = children.get(key)?;
                if path.len() == 1 {
                    match child {
                        CommandTree::Leaf(info) => Some(info),
                        CommandTree::Node(_) => None,
                    }
                } else {
                    child.get(&path[1..])
                }
            }
            CommandTree::Leaf(_) => None,
        }
    }
}

impl<S> Default for CommandTree<S>
where
    S: Send + Sync + 'static,
{
    fn default() -> Self {
        CommandTree::new()
    }
}

impl<S> Debug for CommandTree<S>
where
    S: Send + Sync + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandTree::Node(children) => {
                write!(f, "Node {{ ")?;
                for (key, child) in children {
                    write!(f, "{}: {:?}, ", key, child)?;
                }
                write!(f, "}}")
            }
            CommandTree::Leaf(_) => write!(f, "Leaf"),
        }
    }
}

pub struct CommandExecutor<S>
where
    S: Send + Sync + 'static,
{
    commands: CommandTree<S>,
}

impl<S> CommandExecutor<S>
where
    S: Send + Sync + 'static,
{
    /// Register an async command handler
    pub fn register<C, F, Fut>(&mut self, handler: F)
    where
        C: crate::commands::Command,
        F: Fn(C, Arc<Interaction>, Arc<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = CommandResponse> + Send + 'static,
    {
        let handler = TypedAsyncHandler {
            handler,
            _phantom: std::marker::PhantomData,
        };

        let name = C::name().to_string();
        let command_info = CommandInfo {
            handler: Box::new(handler),
            options: C::options(),
            description: C::description(),
        };

        let path = name.split(' ').map(String::from).collect::<Vec<_>>();
        self.commands.insert(&path, command_info);
    }

    /// Executes a command with the given name
    pub async fn execute(
        &self,
        name: &str,
        interaction: Arc<Interaction>,
        options: Vec<CommandDataOption>,
        state: Arc<S>,
    ) -> Option<InteractionResponse> {
        let path = name.split(' ').map(String::from).collect::<Vec<_>>();
        let handler = self.commands.get(&path)?;

        Some(
            handler
                .handler
                .handle(interaction, options, state)
                .await
                .unwrap_or_else(|e| {
                    let container = ContainerBuilder::new()
                        .accent_color(Some(0xAA0000))
                        .component(
                            TextDisplayBuilder::new(format!("An error occurred: {}", e)).build(),
                        )
                        .build();

                    InteractionResponse {
                        kind: InteractionResponseType::ChannelMessageWithSource,
                        data: Some(InteractionResponseData {
                            components: Some(vec![container.into()]),
                            flags: Some(MessageFlags::EPHEMERAL | MessageFlags::IS_COMPONENTS_V2),
                            ..Default::default()
                        }),
                    }
                }),
        )
    }

    /// Realizes the command tree into a list of `Command`s for registration with Discord
    pub fn build_commands(&self) -> Vec<Command> {
        let mut commands: Vec<Command> = Vec::new();

        if let CommandTree::Node(children) = &self.commands {
            for (name, child) in children.iter() {
                let mut command;

                match child {
                    CommandTree::Leaf(info) => {
                        // This is a top-level command
                        command = CommandBuilder::new(
                            name,
                            info.description,
                            twilight_model::application::command::CommandType::ChatInput,
                        )
                        .contexts(vec![
                            InteractionContextType::Guild,
                            InteractionContextType::BotDm,
                            InteractionContextType::PrivateChannel,
                        ]);
                        for option in &info.options {
                            command = command.option(option.clone());
                        }
                    }
                    CommandTree::Node(subcommand_or_group) => {
                        command = CommandBuilder::new(
                            name,
                            "No description provided",
                            twilight_model::application::command::CommandType::ChatInput,
                        )
                        .contexts(vec![
                            InteractionContextType::Guild,
                            InteractionContextType::BotDm,
                            InteractionContextType::PrivateChannel,
                        ]);
                        for (grandchild_name, grandchild) in subcommand_or_group.iter() {
                            match grandchild {
                                CommandTree::Leaf(info) => {
                                    // This is a subcommand
                                    let mut subcommand =
                                        SubCommandBuilder::new(grandchild_name, info.description);
                                    for option in &info.options {
                                        subcommand = subcommand.option(option.clone());
                                    }
                                    command = command.option(subcommand.build());
                                }
                                CommandTree::Node(_) => {
                                    // This is a subcommand group
                                    if let CommandTree::Node(sub_subcommands) = grandchild {
                                        let subcommand_group = SubCommandGroupBuilder::new(
                                            grandchild_name,
                                            "No description provided",
                                        );
                                        let mut subcommands = Vec::new();

                                        for (subchild_name, subchild) in sub_subcommands.iter() {
                                            if let CommandTree::Leaf(info) = subchild {
                                                let mut subcommand = SubCommandBuilder::new(
                                                    subchild_name,
                                                    info.description,
                                                );
                                                for option in &info.options {
                                                    subcommand = subcommand.option(option.clone());
                                                }
                                                subcommands.push(subcommand);
                                            }
                                        }
                                        command = command.option(
                                            subcommand_group.subcommands(subcommands).build(),
                                        );
                                    } else {
                                        panic!("Expected Node for subcommand group");
                                    }
                                }
                            }
                        }
                    }
                }
                commands.push(command.build());
            }

            commands
        } else {
            panic!("Root of command tree must be a node");
        }
    }
}

impl<S> From<&CommandExecutor<S>> for Vec<Command>
where
    S: Send + Sync + 'static,
{
    fn from(executor: &CommandExecutor<S>) -> Self {
        executor.build_commands()
    }
}

impl<S> Default for CommandExecutor<S>
where
    S: Send + Sync + 'static,
{
    fn default() -> Self {
        CommandExecutor {
            commands: CommandTree::new(),
        }
    }
}
