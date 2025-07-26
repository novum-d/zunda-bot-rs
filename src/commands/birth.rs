use crate::{Context, Error};
use poise::ChoiceParameter;

#[derive(ChoiceParameter)]
pub enum BirthAction {
    List,
    Register,
    Edit,
    Reset,
    Update,
}

#[poise::command(slash_command)]
pub async fn birth(
    ctx: Context<'_>,
    #[description = "操作"] action: BirthAction,
) -> Result<(), Error> {
    match action {
        BirthAction::List => {
            let members = get_registered_members().await;
            ctx.say(format!("誕生日登録済みメンバー: {:?}", members)).await?;
        }
        BirthAction::Register => {
            let members = get_unregistered_members().await;
            ctx.say(format!("誕生日未登録メンバー: {:?}", members)).await?;
        }
        BirthAction::Edit => {
            let members = get_registered_members().await;
            ctx.say(format!("誕生日編集対象メンバー: {:?}", members)).await?;
        }
        BirthAction::Reset => {
            let members = get_registered_members().await;
            ctx.say(format!("誕生日通知解除対象メンバー: {:?}", members)).await?;
        }
        BirthAction::Update => {
            sync_guild_members().await;
            ctx.say("メンバー情報を更新しました").await?;
        }
    }
    Ok(())
}

// --- ダミーDB操作関数 ---
async fn get_registered_members() -> Vec<String> {
    vec!["Alice".to_string(), "Bob".to_string()]
}
async fn get_unregistered_members() -> Vec<String> {
    vec!["Charlie".to_string()]
}
async fn sync_guild_members() {}