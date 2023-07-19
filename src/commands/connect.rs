use std::cmp::Ordering;
use std::path::Path;

use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use serenity::model::prelude::command::CommandOptionType;
use serenity::model::prelude::{AttachmentType, GuildChannel};
use serenity::prelude::Context;
use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

use crate::{premade, utils::*, DBCONNS};

use crate::config::Config;
use crate::utils::{interaction_reply, interaction_reply_edit, interaction_reply_ephemeral};
use crate::DB;

pub async fn run(
    command: &ApplicationCommandInteraction,
    ctx: Context,
) -> Result<(), anyhow::Error> {
    if DBCONNS
        .lock()
        .await
        .contains_key(command.channel_id.as_u64())
    {
        interaction_reply_ephemeral(
            command,
            ctx,
            "This channel already has an associated database instance",
        )
        .await?;
        return Ok(());
    }
    match command.guild_id {
        Some(id) => {
            let result: Result<Option<Config>, surrealdb::Error> =
                DB.select(("guild_config", id.to_string())).await;

            let config = match result {
                Ok(response) => {
                    match response {
                        Some(c) => {c}
                        None => return interaction_reply_ephemeral(command, ctx, "No config found for this server, please ask an administrator to configure the bot".to_string()).await
                    }
                }
                Err(e) => return interaction_reply_ephemeral(command, ctx, format!("Database error: {}", e)).await,
            };

            let channel = command.channel_id.to_channel(&ctx).await?.guild().unwrap();

            let db = Surreal::new::<Mem>(()).await?;
            db.use_ns("test").use_db("test").await?;

            register_db(
                ctx.clone(),
                db.clone(),
                channel.clone(),
                config.clone(),
                crate::ConnType::ConnectedChannel,
                true,
            )
            .await?;

            match command.data.options.len().cmp(&1) {
                Ordering::Greater => {
                    interaction_reply_ephemeral(command, ctx, "Please only supply one arguement (you can use the up arrow to edit the previous command)").await?;
                    return Ok(());
                }
                Ordering::Equal => {
                    let op_option = command.data.options[0].clone();
                    match op_option.kind {
                        CommandOptionType::String => {
                            match op_option.value.unwrap().as_str().unwrap() {
                                "surreal_deal_mini" => {
                                    load_premade(
                                        ctx,
                                        db,
                                        channel,
                                        command,
                                        "surreal_deal_mini.surql",
                                        "surreal deal(mini)",
                                        Some("surreal_deal.png"),
                                    )
                                    .await?;
                                }
                                "surreal_deal" => {
                                    load_premade(
                                        ctx,
                                        db,
                                        channel,
                                        command,
                                        "surreal_deal.surql",
                                        "surreal deal",
                                        Some("surreal_deal.png"),
                                    )
                                    .await?;
                                }
                                _ => {
                                    interaction_reply_ephemeral(
                                        command,
                                        ctx,
                                        "Cannot find requested dataset",
                                    )
                                    .await?;
                                    return Ok(());
                                }
                            }
                        }
                        CommandOptionType::Attachment => {
                            if let Some(CommandDataOptionValue::Attachment(attachment)) =
                                op_option.resolved
                            {
                                interaction_reply(
                                    command,
                                    ctx.clone(),
                                    format!(
                                        "Your file ({}) is now being downloaded!!!",
                                        attachment.filename
                                    ),
                                )
                                .await?;
                                match attachment.download().await {
                                    Ok(data) => {
                                        interaction_reply_edit(command, ctx.clone(), format!("Your data is currently being loaded, soon you'll be able to query your dataset!!!")).await?;

                                        let db = db.clone();
                                        let (channel, ctx, command) =
                                            (channel.clone(), ctx.clone(), command.clone());
                                        tokio::spawn(async move {
                                            db.query(String::from_utf8_lossy(&data).into_owned())
                                                .await
                                                .unwrap();
                                            interaction_reply_edit(
                                                &command,
                                                ctx,
                                                format!("You now have your own database instance, head over to <#{}> to start writing SurrealQL to query your data!!!", channel.id.as_u64()),
                                            )
                                            .await
                                            .unwrap();
                                        });
                                    }
                                    Err(why) => {
                                        interaction_reply_edit(
                                            command,
                                            ctx,
                                            format!("Error with attachment: {}", why),
                                        )
                                        .await?;
                                        return Ok(());
                                    }
                                }
                            } else {
                                interaction_reply_edit(command, ctx, "Error with attachment")
                                    .await?;
                                return Ok(());
                            }
                        }
                        _ => {
                            interaction_reply_ephemeral(command, ctx, "Unsupported option type")
                                .await?;
                            return Ok(());
                        }
                    }
                }
                Ordering::Less => interaction_reply(command, ctx, format!("This channel is now connected to a SurrealDB instance, try writing some SurrealQL with the /query command!!!\n(note this channel will expire after {:#?} of inactivity)", config.ttl)).await?,
            };

            return Ok(());
        }
        None => {
            return interaction_reply(
                command,
                ctx,
                "Direct messages are not currently supported".to_string(),
            )
            .await;
        }
    }
}

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("connect")
        .description("Creates a SurrealDB instance and associates it with the current channel")
        .create_option(premade::register)
        .create_option(|option| {
            option
                .name("file")
                .description("a SurrealQL to load into the database instance")
                .kind(CommandOptionType::Attachment)
                .required(false)
        })
}

async fn load_premade(
    ctx: Context,
    db: Surreal<Db>,
    channel: GuildChannel,
    command: &ApplicationCommandInteraction,
    file_name: &'static str,
    name: &'static str,
    schema_name: Option<&'static str>,
) -> Result<(), anyhow::Error> {
    {
        interaction_reply(
            command,
            ctx.clone(),
            format!(
                "Data is currently being loaded, soon you'll be able to query the {} dataset!!!",
                name
            ),
        )
        .await?;
        let db = db.clone();
        let (channel, ctx, command) = (channel.clone(), ctx.clone(), command.clone());
        tokio::spawn(async move {
            match db.import(format!("premade/{}", file_name)).await {
                Ok(_) => {
                    interaction_reply_edit(
                        &command,
                        ctx.clone(),
                        format!(
                            "Data is now loaded and you can query the {} dataset with the /query command!!!",
                            name
                        ),
                    )
                    .await
                    .unwrap();
                    if let Some(scheme_file_name) = schema_name {
                        channel
                            .send_files(
                                ctx,
                                [AttachmentType::Path(&Path::new(&format!(
                                    "premade/{}",
                                    scheme_file_name
                                )))],
                                |m| m.content("schema:"),
                            )
                            .await
                            .unwrap();
                    }
                }
                Err(why) => {
                    interaction_reply_edit(&command, ctx, format!("Error loading data: {}", why))
                        .await
                        .unwrap();
                }
            };
        });
        Ok(())
    }
}
