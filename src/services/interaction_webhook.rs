use crate::handler::interaction::handle_component_interaction;
use crate::models::common::Data;
use crate::reminder::ui;
use crate::res::colors::EMBED_COLOR_WARNING;
use crate::usecase::birth_list_usecase::BirthListView;
use crate::usecase::birth_reset_usecase::webhook_reset_button_custom_id;
use crate::usecase::birth_signup_usecase::BirthSignupResult;
use crate::usecase::seven_days_usecase::Caller;
use anyhow::Context as _;
use serde::Deserialize;
use serde_json::{json, Value};
use serenity::all::{ApplicationId, GuildId, Interaction};

const INTERACTION_TYPE_PING: u8 = 1;
const INTERACTION_TYPE_APPLICATION_COMMAND: u8 = 2;
const INTERACTION_TYPE_COMPONENT: u8 = 3;
const INTERACTION_TYPE_MODAL_SUBMIT: u8 = 5;
const RESPONSE_TYPE_PONG: u8 = 1;
const RESPONSE_TYPE_CHANNEL_MESSAGE: u8 = 4;
const RESPONSE_TYPE_DEFERRED_CHANNEL_MESSAGE: u8 = 5;
const RESPONSE_TYPE_MODAL: u8 = 9;
const EPHEMERAL_FLAG: u64 = 64;
const BIRTH_SIGNUP_MODAL_ID: &str = "birth_signup";
const BIRTH_SIGNUP_INPUT_ID: &str = "birth_input";

pub struct WebhookHttpResponse {
    pub status: u16,
    pub body: String,
}

impl WebhookHttpResponse {
    pub fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            body: body.to_string(),
        }
    }

    pub fn empty(status: u16) -> Self {
        Self {
            status,
            body: String::new(),
        }
    }
}

pub async fn handle_interaction_body(
    data: Option<&Data>,
    body: &[u8],
) -> anyhow::Result<WebhookHttpResponse> {
    let interaction = serde_json::from_slice::<DiscordInteraction>(body)
        .context("Discord interaction JSON parse failed")?;

    match interaction.interaction_type {
        INTERACTION_TYPE_PING => Ok(WebhookHttpResponse::json(
            200,
            json!({ "type": RESPONSE_TYPE_PONG }),
        )),
        INTERACTION_TYPE_APPLICATION_COMMAND => {
            let Some(data) = data else {
                return Ok(discord_response(starting_message()));
            };
            handle_application_command(data, interaction).await
        }
        INTERACTION_TYPE_COMPONENT => {
            let Some(data) = data else {
                return Ok(discord_response(starting_message()));
            };
            handle_component(data, body).await
        }
        INTERACTION_TYPE_MODAL_SUBMIT => {
            let Some(data) = data else {
                return Ok(discord_response(starting_message()));
            };
            handle_modal_submit(data, interaction).await
        }
        _ => Ok(WebhookHttpResponse::json(
            400,
            json!({ "error": "unsupported interaction type" }),
        )),
    }
}

async fn handle_application_command(
    data: &Data,
    interaction: DiscordInteraction,
) -> anyhow::Result<WebhookHttpResponse> {
    let command_data = interaction
        .data
        .as_ref()
        .context("application command data missing")?;
    let route = ApplicationCommandRoute::from_command_data(command_data)
        .context("unsupported application command")?;

    match route {
        ApplicationCommandRoute::Hello => Ok(discord_response(message("こんにちは、なのだ!"))),
        ApplicationCommandRoute::BirthSignup => Ok(discord_response(signup_modal())),
        route => {
            let data = data.clone();
            tokio::spawn(async move {
                send_deferred_application_command_followup(data, interaction, route).await;
            });
            Ok(discord_response(deferred_message()))
        }
    }
}

async fn send_deferred_application_command_followup(
    data: Data,
    interaction: DiscordInteraction,
    route: ApplicationCommandRoute,
) {
    let Some(token) = interaction.token.clone() else {
        tracing::warn!("Discord interaction token missing for deferred followup");
        return;
    };
    let Some(application_id) = interaction
        .application_id
        .and_then(|application_id| u64::try_from(application_id).ok())
    else {
        tracing::warn!("Discord application id missing for deferred followup");
        return;
    };
    data.discord_http
        .set_application_id(ApplicationId::new(application_id));

    let followup = match handle_deferred_application_command(&data, interaction, route).await {
        Ok(response) => match followup_data_from_response(response) {
            Ok(followup) => followup,
            Err(e) => {
                tracing::warn!("Deferred interaction response conversion failed: {}", e);
                failed_followup_data()
            }
        },
        Err(e) => {
            tracing::warn!("Deferred interaction handling failed: {}", e);
            failed_followup_data()
        }
    };

    if let Err(e) = data
        .discord_http
        .create_followup_message(&token, &followup, Vec::new())
        .await
    {
        tracing::warn!("Discord deferred followup failed: {}", e);
    }
}

async fn handle_deferred_application_command(
    data: &Data,
    interaction: DiscordInteraction,
    route: ApplicationCommandRoute,
) -> anyhow::Result<WebhookHttpResponse> {
    let guild_id = interaction.guild_id;
    let channel_id = interaction.channel_id;
    let member_id = interaction.user_id();
    let role_ids = interaction
        .member
        .as_ref()
        .map(|member| member.roles.clone())
        .unwrap_or_default();
    let command_data = interaction
        .data
        .context("application command data missing")?;

    match route {
        ApplicationCommandRoute::Hello => Ok(discord_response(message("こんにちは、なのだ!"))),
        ApplicationCommandRoute::BirthList => {
            if let Err(e) = data.guild_update_usecase.invoke().await {
                tracing::warn!("Guild sync failed before webhook birth list: {}", e);
            }

            let guild_id = guild_id.context("guild id missing")?;
            let view = data
                .birth_list_usecase
                .build_view(GuildId::new(u64::try_from(guild_id)?))
                .await?;
            Ok(discord_response(birth_list_message(&view)))
        }
        ApplicationCommandRoute::BirthSignup => Ok(discord_response(signup_modal())),
        ApplicationCommandRoute::BirthReset => {
            let guild_id = guild_id.context("guild id missing")?;
            let member_id = member_id.context("user id missing")?;
            let view = data
                .birth_reset_usecase
                .build_confirmation_view(guild_id, Some(&format!("guild-{guild_id}")), member_id)
                .await?;
            if view.has_birth {
                Ok(discord_response(birth_reset_confirmation_message(
                    guild_id, member_id,
                )))
            } else {
                Ok(discord_response(embed_message(
                    "⚠️ 誕生日が登録されていないのだ",
                    EMBED_COLOR_WARNING,
                )))
            }
        }
        ApplicationCommandRoute::BirthRemindResume => {
            let guild_id = guild_id.context("guild id missing")?;
            let member_id = member_id.context("user id missing")?;
            data.reminder_service
                .resume_reminder(guild_id, member_id)
                .await?;
            Ok(discord_response(message("リマインドを再開したのだ！")))
        }
        ApplicationCommandRoute::SetupReminderChannel => {
            let guild_id = guild_id.context("guild id missing")?;
            let member_id = member_id.context("user id missing")?;
            if !data.reminder_service.is_admin_member(member_id).await? {
                return Ok(discord_response(message(
                    "このコマンドを実行する権限がないのだ。",
                )));
            }

            if let Err(e) = data.guild_update_usecase.invoke().await {
                tracing::warn!("Guild sync failed before webhook reminder setup: {}", e);
            }

            let session_id = data
                .reminder_service
                .create_selection_session(member_id, guild_id)?;
            Ok(discord_response(setup_reminder_channel_message(
                member_id,
                guild_id,
                &session_id,
            )))
        }
        ApplicationCommandRoute::SetupAddNotificationChannel => {
            manage_notification_channel(data, guild_id, member_id, &command_data, true).await
        }
        ApplicationCommandRoute::SetupRemoveNotificationChannel => {
            manage_notification_channel(data, guild_id, member_id, &command_data, false).await
        }
        ApplicationCommandRoute::SevenDaysStart
        | ApplicationCommandRoute::SevenDaysStatus
        | ApplicationCommandRoute::SevenDaysStop => {
            let Some(usecase) = &data.seven_days_usecase else {
                return Ok(discord_response(message(
                    "7DTD サーバーは設定されていないのだ。",
                )));
            };
            let caller = Caller {
                guild_id,
                channel_id,
                user_id: member_id,
                role_ids: &role_ids,
            };
            let admin = route.requires_seven_days_admin();
            if !usecase.authorize(&caller, admin) {
                tracing::warn!(?guild_id, ?channel_id, user_id = ?member_id, command = ?route, "unauthorized 7DTD command");
                return Ok(discord_response(message(
                    "このコマンドを実行する権限がないのだ。",
                )));
            }
            let _operation_lock = if requires_operation_lock(route) {
                match usecase.try_operation_lock().await {
                    Ok(Some(lock)) => Some(lock),
                    Ok(None) => {
                        return Ok(discord_response(message(
                            "7DTD サーバーは現在、別の開始または停止処理中なのだ。完了してからもう一度試してほしいのだ。",
                        )))
                    }
                    Err(error) => {
                        tracing::error!(error = %error, "7DTD operation lock failed");
                        return Ok(discord_response(message(
                            "7DTD サーバーの操作受付を確認できないのだ。少し待ってから再試行してほしいのだ。",
                        )))
                    }
                }
            } else {
                None
            };
            tracing::info!(?guild_id, ?channel_id, user_id = ?member_id, command = ?route, "starting 7DTD command");
            let result = match route {
                ApplicationCommandRoute::SevenDaysStart => usecase.start().await,
                ApplicationCommandRoute::SevenDaysStatus => usecase.status().await,
                ApplicationCommandRoute::SevenDaysStop => usecase.stop().await,
                _ => unreachable!(),
            };
            match result {
                Ok(content) => {
                    tracing::info!(command = ?route, "7DTD command completed");
                    Ok(discord_response(message(&content)))
                }
                Err(error) => {
                    tracing::error!(command = ?route, error = %error, "7DTD command failed");
                    Ok(discord_response(message(
                        "7DTD サーバーの操作に失敗したのだ。管理者にログ確認を依頼してほしいのだ。",
                    )))
                }
            }
        }
    }
}

async fn handle_component(data: &Data, body: &[u8]) -> anyhow::Result<WebhookHttpResponse> {
    let interaction = serde_json::from_slice::<Interaction>(body)
        .context("Discord component interaction JSON parse failed")?;
    let Interaction::Component(component) = interaction else {
        anyhow::bail!("interaction payload was not a component");
    };

    match handle_component_interaction(data, &component).await {
        Ok(true) => Ok(WebhookHttpResponse::empty(204)),
        Ok(false) => Ok(WebhookHttpResponse::json(
            400,
            json!({ "error": "unsupported component interaction" }),
        )),
        Err(e) => Err(e),
    }
}

async fn handle_modal_submit(
    data: &Data,
    interaction: DiscordInteraction,
) -> anyhow::Result<WebhookHttpResponse> {
    let guild_id = interaction.guild_id.context("guild id missing")?;
    let member_id = interaction.user_id().context("user id missing")?;
    let command_data = interaction.data.context("modal submit data missing")?;
    if command_data.custom_id.as_deref() != Some(BIRTH_SIGNUP_MODAL_ID) {
        return Ok(WebhookHttpResponse::json(
            400,
            json!({ "error": "unsupported modal submit" }),
        ));
    }

    let input_birth = command_data
        .modal_text_value(BIRTH_SIGNUP_INPUT_ID)
        .context("birth signup modal input missing")?;
    let result = data
        .birth_signup_usecase
        .register_birth(
            guild_id,
            Some(&format!("guild-{guild_id}")),
            member_id,
            &input_birth,
        )
        .await?;
    Ok(discord_response(signup_result_message(&result)))
}

async fn manage_notification_channel(
    data: &Data,
    guild_id: Option<i64>,
    member_id: Option<i64>,
    command_data: &ApplicationCommandData,
    add: bool,
) -> anyhow::Result<WebhookHttpResponse> {
    let guild_id = guild_id.context("guild id missing")?;
    let member_id = member_id.context("user id missing")?;
    if !data.reminder_service.is_admin_member(member_id).await? {
        return Ok(discord_response(message(
            "このコマンドを実行する権限がないのだ。",
        )));
    }

    let channel_id = command_data
        .subcommand_option_value("channel")
        .and_then(value_as_i64)
        .context("channel option missing")?;

    if add {
        data.reminder_service
            .add_notification_channel(guild_id, channel_id)
            .await?;
    } else {
        data.reminder_service
            .remove_notification_channel(guild_id, channel_id)
            .await?;
    }

    let content = if add {
        format!("<#{channel_id}> を通知チャンネルに追加したのだ！")
    } else {
        format!("<#{channel_id}> を通知チャンネルから削除したのだ！")
    };
    Ok(discord_response(message(&content)))
}

fn discord_response(body: Value) -> WebhookHttpResponse {
    WebhookHttpResponse::json(200, body)
}

fn message(content: &str) -> Value {
    json!({
        "type": RESPONSE_TYPE_CHANNEL_MESSAGE,
        "data": {
            "content": content,
            "flags": EPHEMERAL_FLAG
        }
    })
}

fn requires_operation_lock(route: ApplicationCommandRoute) -> bool {
    matches!(
        route,
        ApplicationCommandRoute::SevenDaysStart | ApplicationCommandRoute::SevenDaysStop
    )
}

fn starting_message() -> Value {
    message("起動処理中なのだ。数秒後にもう一度試してほしいのだ。")
}

fn deferred_message() -> Value {
    json!({
        "type": RESPONSE_TYPE_DEFERRED_CHANNEL_MESSAGE,
        "data": {
            "flags": EPHEMERAL_FLAG
        }
    })
}

fn failed_followup_data() -> Value {
    json!({
        "content": "処理に失敗したのだ。時間をおいて再試行してほしいのだ。",
        "flags": EPHEMERAL_FLAG
    })
}

fn followup_data_from_response(response: WebhookHttpResponse) -> anyhow::Result<Value> {
    if response.status != 200 {
        anyhow::bail!("deferred response status was {}", response.status);
    }

    let body = serde_json::from_str::<Value>(&response.body)
        .context("deferred response body JSON parse failed")?;
    if body.get("type").and_then(Value::as_u64) != Some(u64::from(RESPONSE_TYPE_CHANNEL_MESSAGE)) {
        anyhow::bail!("deferred response was not a channel message");
    }

    body.get("data")
        .cloned()
        .context("deferred response data missing")
}

fn embed_message(title: &str, color: u32) -> Value {
    json!({
        "type": RESPONSE_TYPE_CHANNEL_MESSAGE,
        "data": {
            "embeds": [{ "title": title, "color": color }],
            "flags": EPHEMERAL_FLAG
        }
    })
}

fn birth_list_message(view: &BirthListView) -> Value {
    let mut embed = json!({
        "title": view.title(),
        "color": view.color()
    });
    if let Some(description) = view.description() {
        embed["description"] = json!(description);
    }

    json!({
        "type": RESPONSE_TYPE_CHANNEL_MESSAGE,
        "data": {
            "embeds": [embed],
            "flags": EPHEMERAL_FLAG
        }
    })
}

fn signup_modal() -> Value {
    json!({
        "type": RESPONSE_TYPE_MODAL,
        "data": {
            "custom_id": BIRTH_SIGNUP_MODAL_ID,
            "title": "誕生日の通知登録",
            "components": [{
                "type": 1,
                "components": [{
                    "type": 4,
                    "custom_id": BIRTH_SIGNUP_INPUT_ID,
                    "label": "自身の誕生日を入力するのだ",
                    "style": 1,
                    "placeholder": "02/01",
                    "min_length": 5,
                    "max_length": 5,
                    "required": true
                }]
            }]
        }
    })
}

fn signup_result_message(result: &BirthSignupResult) -> Value {
    let mut data = json!({
        "embeds": [{
            "title": result.title(),
            "color": result.color()
        }],
        "flags": EPHEMERAL_FLAG
    });
    if let Some(content) = result.content() {
        data["content"] = json!(content);
    }

    json!({
        "type": RESPONSE_TYPE_CHANNEL_MESSAGE,
        "data": data
    })
}

fn birth_reset_confirmation_message(guild_id: i64, member_id: i64) -> Value {
    json!({
        "type": RESPONSE_TYPE_CHANNEL_MESSAGE,
        "data": {
            "content": "誕生日の通知登録を解除するのだ⚠️",
            "flags": EPHEMERAL_FLAG,
            "components": [{
                "type": 1,
                "components": [{
                    "type": 2,
                    "custom_id": webhook_reset_button_custom_id(guild_id, member_id),
                    "label": "解除",
                    "style": 4
                }]
            }]
        }
    })
}

fn setup_reminder_channel_message(member_id: i64, guild_id: i64, session_id: &str) -> Value {
    json!({
        "type": RESPONSE_TYPE_CHANNEL_MESSAGE,
        "data": {
            "content": "リマインドを送るユーザーを選ぶのだ！",
            "flags": EPHEMERAL_FLAG,
            "components": [{
                "type": 1,
                "components": [{
                    "type": 2,
                    "custom_id": ui::start_button_custom_id(member_id, guild_id, session_id),
                    "label": "ユーザーを選ぶのだ",
                    "style": 1
                }]
            }]
        }
    })
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str()?.parse::<i64>().ok())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationCommandRoute {
    Hello,
    BirthList,
    BirthSignup,
    BirthReset,
    BirthRemindResume,
    SetupReminderChannel,
    SetupAddNotificationChannel,
    SetupRemoveNotificationChannel,
    SevenDaysStart,
    SevenDaysStatus,
    SevenDaysStop,
}

impl ApplicationCommandRoute {
    fn requires_seven_days_admin(self) -> bool {
        matches!(self, Self::SevenDaysStart | Self::SevenDaysStop)
    }

    fn from_command_data(data: &ApplicationCommandData) -> Option<Self> {
        match data.name.as_str() {
            "hello" => Some(Self::Hello),
            "birth" => match data.options.first()?.name.as_str() {
                "list" => Some(Self::BirthList),
                "signup" => Some(Self::BirthSignup),
                "reset" => Some(Self::BirthReset),
                "remind" => {
                    let nested = data.options.first()?.options.first()?;
                    (nested.name == "resume").then_some(Self::BirthRemindResume)
                }
                _ => None,
            },
            "setup" => match data.options.first()?.name.as_str() {
                "reminder-channel" => Some(Self::SetupReminderChannel),
                "add-notification-channel" => Some(Self::SetupAddNotificationChannel),
                "remove-notification-channel" => Some(Self::SetupRemoveNotificationChannel),
                _ => None,
            },
            "7dtd" => match data.options.first()?.name.as_str() {
                "start" => Some(Self::SevenDaysStart),
                "status" => Some(Self::SevenDaysStatus),
                "stop" => Some(Self::SevenDaysStop),
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct DiscordInteraction {
    #[serde(rename = "type")]
    interaction_type: u8,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    application_id: Option<i64>,
    token: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    guild_id: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    channel_id: Option<i64>,
    member: Option<InteractionMember>,
    user: Option<InteractionUser>,
    data: Option<ApplicationCommandData>,
}

impl DiscordInteraction {
    fn user_id(&self) -> Option<i64> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .map(|user| user.id)
            .or_else(|| self.user.as_ref().map(|user| user.id))
    }
}

#[derive(Debug, Deserialize)]
struct InteractionMember {
    user: Option<InteractionUser>,
    #[serde(default, deserialize_with = "deserialize_i64_vec")]
    roles: Vec<i64>,
}

#[derive(Debug, Deserialize)]
struct InteractionUser {
    #[serde(deserialize_with = "deserialize_i64")]
    id: i64,
}

#[derive(Debug, Deserialize)]
struct ApplicationCommandData {
    #[serde(default)]
    name: String,
    #[serde(default)]
    custom_id: Option<String>,
    #[serde(default)]
    options: Vec<ApplicationCommandOption>,
    #[serde(default)]
    components: Vec<ModalActionRow>,
}

impl ApplicationCommandData {
    fn subcommand_option_value(&self, option_name: &str) -> Option<&Value> {
        self.options
            .first()?
            .options
            .iter()
            .find(|option| option.name == option_name)?
            .value
            .as_ref()
    }

    fn modal_text_value(&self, custom_id: &str) -> Option<String> {
        self.components
            .iter()
            .flat_map(|row| row.components.iter())
            .find(|component| component.custom_id == custom_id)?
            .value
            .clone()
    }
}

#[derive(Debug, Deserialize)]
struct ApplicationCommandOption {
    name: String,
    #[serde(default)]
    value: Option<Value>,
    #[serde(default)]
    options: Vec<ApplicationCommandOption>,
}

#[derive(Debug, Deserialize)]
struct ModalActionRow {
    #[serde(default)]
    components: Vec<ModalComponent>,
}

#[derive(Debug, Deserialize)]
struct ModalComponent {
    custom_id: String,
    value: Option<String>,
}

fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    value
        .map(|value| {
            value_as_i64(&value).ok_or_else(|| serde::de::Error::custom("invalid integer value"))
        })
        .transpose()
}

fn deserialize_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    value_as_i64(&value).ok_or_else(|| serde::de::Error::custom("invalid integer value"))
}

fn deserialize_i64_vec<'de, D>(deserializer: D) -> Result<Vec<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<Value>::deserialize(deserializer)?
        .iter()
        .map(|value| {
            value_as_i64(value).ok_or_else(|| serde::de::Error::custom("invalid integer value"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_data(body: &str) -> ApplicationCommandData {
        serde_json::from_str::<DiscordInteraction>(body)
            .expect("interaction should parse")
            .data
            .expect("command data should exist")
    }

    #[test]
    fn routes_hello_command() {
        let data = command_data(r#"{"type":2,"data":{"name":"hello","options":[]}}"#);

        assert_eq!(
            ApplicationCommandRoute::from_command_data(&data),
            Some(ApplicationCommandRoute::Hello)
        );
    }

    #[test]
    fn routes_nested_birth_remind_resume_command() {
        let data = command_data(
            r#"{"type":2,"data":{"name":"birth","options":[{"name":"remind","options":[{"name":"resume"}]}]}}"#,
        );

        assert_eq!(
            ApplicationCommandRoute::from_command_data(&data),
            Some(ApplicationCommandRoute::BirthRemindResume)
        );
    }

    #[test]
    fn routes_seven_days_commands() {
        for (subcommand, expected) in [
            ("start", ApplicationCommandRoute::SevenDaysStart),
            ("status", ApplicationCommandRoute::SevenDaysStatus),
            ("stop", ApplicationCommandRoute::SevenDaysStop),
        ] {
            let data = command_data(&format!(
                r#"{{"type":2,"data":{{"name":"7dtd","options":[{{"name":"{subcommand}"}}]}}}}"#
            ));
            assert_eq!(
                ApplicationCommandRoute::from_command_data(&data),
                Some(expected)
            );
        }
    }

    #[test]
    fn starting_and_stopping_seven_days_require_admin() {
        assert!(ApplicationCommandRoute::SevenDaysStart.requires_seven_days_admin());
        assert!(ApplicationCommandRoute::SevenDaysStop.requires_seven_days_admin());
        assert!(!ApplicationCommandRoute::SevenDaysStatus.requires_seven_days_admin());
    }

    #[test]
    fn only_start_and_stop_take_the_operation_lock() {
        assert!(requires_operation_lock(
            ApplicationCommandRoute::SevenDaysStart
        ));
        assert!(requires_operation_lock(
            ApplicationCommandRoute::SevenDaysStop
        ));
        assert!(!requires_operation_lock(
            ApplicationCommandRoute::SevenDaysStatus
        ));
    }

    #[test]
    fn reads_channel_option_from_setup_subcommand() {
        let data = command_data(
            r#"{"type":2,"data":{"name":"setup","options":[{"name":"add-notification-channel","options":[{"name":"channel","value":"123"}]}]}}"#,
        );

        assert_eq!(
            data.subcommand_option_value("channel")
                .and_then(value_as_i64),
            Some(123)
        );
    }

    #[test]
    fn reads_birth_signup_modal_value() {
        let data = command_data(
            r#"{"type":5,"data":{"name":"","custom_id":"birth_signup","components":[{"components":[{"custom_id":"birth_input","value":"02/01"}]}]}}"#,
        );

        assert_eq!(
            data.modal_text_value(BIRTH_SIGNUP_INPUT_ID),
            Some("02/01".to_string())
        );
    }

    #[test]
    fn parses_component_payload_without_command_name() {
        let interaction = serde_json::from_str::<DiscordInteraction>(
            r#"{"type":3,"data":{"custom_id":"birth_reset_confirm:1:2","component_type":2},"member":{"user":{"id":"2"}}}"#,
        )
        .expect("component interaction should parse without data.name");

        assert_eq!(interaction.interaction_type, INTERACTION_TYPE_COMPONENT);
        assert_eq!(
            interaction.data.and_then(|data| data.custom_id),
            Some("birth_reset_confirm:1:2".to_string())
        );
    }

    #[test]
    fn deferred_message_is_ephemeral() {
        assert_eq!(
            deferred_message(),
            json!({
                "type": 5,
                "data": {
                    "flags": 64
                }
            })
        );
    }

    #[test]
    fn extracts_followup_data_from_channel_message_response() {
        let response = discord_response(message("done"));

        let followup = followup_data_from_response(response).expect("followup data should extract");

        assert_eq!(
            followup,
            json!({
                "content": "done",
                "flags": 64
            })
        );
    }
}
