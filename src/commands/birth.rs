use crate::{select_member_by_id, update_guild_member_birth, Context, Error};
use chrono::NaiveDate;
use poise::{ChoiceParameter, CreateReply, Modal};
use serenity::all::CreateEmbed;

#[derive(ChoiceParameter)]
pub enum BirthAction {
    List,
    Signup,
    Edit,
    Reset,
    Update,
}

/// 誕生日コマンド
#[poise::command(slash_command)]
pub async fn birth(
    ctx: Context<'_>,
    #[description = "操作"] action: BirthAction,
) -> Result<(), Error> {
    match action {
        BirthAction::List => {
            ctx.say("List").await?;
        }
        BirthAction::Signup => {
            let pool = &ctx.data().pool;
            let guild_id = i64::from(ctx.guild_id().expect("Could not retrieve the Guild ID."));
            let member_id = i64::from(ctx.author().id);

            let member = select_member_by_id(
                pool,
                guild_id,
                member_id,
            ).await?;

            if let Some(member) = member {
                if let None = member.birth {
                    if let Context::Application(app_ctx) = ctx {
                        let data = BirthSignupModal::execute(app_ctx).await?;
                        if let Some(data) = data {
                            let birth = NaiveDate::parse_from_str(&data.birth_input, "%Y-%m-%d")?;
                            update_guild_member_birth(
                                pool,
                                guild_id,
                                member_id,
                                birth,
                            ).await?;

                            ctx.send(CreateReply::default()
                                .embed(CreateEmbed::new()
                                    .title("✅  誕生日の登録が完了いたしました。")
                                    .color(0x00ff00)) // オレンジ色
                                .content("登録された日付の12時に誕生日が通知されます。")
                                .ephemeral(true))
                                .await?;
                        }
                    }
                } else {
                    ctx.send(CreateReply::default()
                        .embed(CreateEmbed::new()
                            .title("⚠️ 誕生日はすでに登録済みです。")
                            .color(0xff9900)) // オレンジ色
                        .ephemeral(true))
                        .await?;
                }
            }
        }
        BirthAction::Edit => {}
        BirthAction::Reset => {
            ctx.say("Reset").await?;
        }
        BirthAction::Update => {
            ctx.say("Update").await?;
        }
    }
    Ok(())
}


#[derive(Debug, Modal)]
#[name = "誕生日の通知登録"] // 最初のタイトル
struct BirthSignupModal {
    #[name = "自身の誕生日を入力してください"] // フィールドのタイトル
    #[placeholder = "1999-12-10"]
    #[min_length = 10]
    #[max_length = 10]
    birth_input: String,
    // #[name = "2番目の入力ラベル"],
    // #[paragraph] // 単一行から複数行テキストボックスに変更
    // second_input: Option<String>, // Optionは任意入力を意味
}