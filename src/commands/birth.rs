use crate::models::common::{Context, Error};
use poise::ChoiceParameter;

#[derive(ChoiceParameter)]
pub enum BirthAction {
    #[name = "List: サーバー内メンバーの誕生日リスト表示"]
    List,
    #[name = "Signup: 自身の誕生日の通知登録"]
    Signup,
    #[name = "Reset: 自身の誕生日の通知解除"]
    Reset,
}

/// 誕生日コマンド birth
#[poise::command(slash_command)]
pub async fn birth(
    ctx: Context<'_>,
    #[description = "操作"] action: BirthAction,
) -> anyhow::Result<(), Error> {
    ctx.data().guild_update_usecase.invoke().await?;

    match action {
        BirthAction::List => {
            ctx.data().birth_list_usecase.invoke(ctx).await?;
        }
        BirthAction::Signup => {
            ctx.data().birth_signup_usecase.invoke(ctx).await?;
        }
        BirthAction::Reset => {
            ctx.data().birth_reset_usecase.invoke(ctx).await?;
        }
    }
    Ok(())
}
