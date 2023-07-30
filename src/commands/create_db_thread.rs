use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::application_command::ApplicationCommandInteraction;

use memorable_wordlist::kebab_case;
use serenity::prelude::Context;

use crate::components::show_configurable_session;
use crate::utils::*;

use crate::config::Config;
use crate::utils::interaction_reply;
use crate::DB;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    match command.guild_id {
        Some(id) => {
            let result: Result<Option<Config>, surrealdb::Error> =
                DB.select(("guild_config", id.to_string())).await;

            let config = match result {
                Ok(response) => {
                    match response {
                        Some(c) => {c}
                        None => return interaction_reply_ephemeral(command, ctx, ":warning: No config found for this server, please ask an administrator to configure the bot".to_string()).await
                    }
                }
                Err(e) => return interaction_reply_ephemeral(command, ctx, format!("Database error: {}", e)).await,
            };

            let message = command.data.resolved.messages.keys().next().unwrap();

            let channel = command
                .channel_id
                .create_public_thread(&ctx, message, |t| t.name(kebab_case(40)))
                .await?;

            let db = create_db_instance(&config).await?;

            show_configurable_session(&ctx, &channel, crate::ConnType::Thread, &config).await?;
            interaction_reply_ephemeral(command, ctx.clone(), format!(":information_source: You now have your own database instance! Head over to <#{}> to start writing SurrealQL!", channel.id.as_u64())).await?;

            register_db(ctx, db, channel, config, crate::ConnType::Thread, true).await?;
            Ok(())
        }
        None => {
            interaction_reply(
                command,
                ctx,
                ":warning: Direct messages are not currently supported".to_string(),
            )
            .await
        }
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("create_db_thread")
        .name_localized("en-US", "Create a DB Thread")
        .name_localized("en-GB", "Create a DB Thread")
        .kind(serenity::model::prelude::command::CommandType::Message)
}
