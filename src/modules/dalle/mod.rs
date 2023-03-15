use anyhow::Error;
use teloxide::prelude::*;
use teloxide::types::BotCommand;

use crate::{
    config::SharedConfig,
    module_mgr::Module,
    modules::admin::{check_user_permission, MemberManager},
    types::{HandlerResult, TeloxideHandler},
    utils::dptree_ext,
};

async fn paint(
    bot: Bot,
    msg: Message,
    member_mgr: MemberManager,
    config: SharedConfig,
) -> HandlerResult {
    if !check_user_permission(&bot, &msg, &member_mgr, &config).await {
        return Ok(());
    }

    bot.send_message(msg.chat.id, &config.i18n.dalle_prompt)
        .await?;

    Ok(())
}

pub(crate) struct DallE;

#[async_trait]
impl Module for DallE {
    async fn register_dependency(&mut self, _dep_map: &mut DependencyMap) -> Result<(), Error> {
        Ok(())
    }

    fn handler_chain(&self) -> TeloxideHandler {
        dptree::entry().branch(
            Update::filter_message()
                .branch(dptree::filter_map(dptree_ext::command_filter("paint")).endpoint(paint)),
        )
    }

    fn commands(&self) -> Vec<BotCommand> {
        vec![BotCommand::new("paint", "Paint images with Dall-E")]
    }
}
