use teloxide::prelude::*;

use super::MemberManager;
use crate::config::SharedConfig;

pub(crate) async fn check_user_permission(
    bot: &Bot,
    msg: &Message,
    member_mgr: &MemberManager,
    config: &SharedConfig,
) -> bool {
    let sender_username = msg
        .from()
        .and_then(|u| u.username.clone())
        .unwrap_or_default();
    if !member_mgr
        .is_member_allowed(sender_username)
        .await
        .unwrap_or(false)
    {
        let _ = bot
            .send_message(msg.chat.id, &config.i18n.not_allowed_prompt)
            .reply_to_message_id(msg.id)
            .await;
        return false;
    }

    true
}
