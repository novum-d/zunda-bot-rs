use crate::reminder::service::ReminderService;
use crate::usecase::birth_list_usecase::BirthListUsecase;
use crate::usecase::birth_notify_usecase::BirthNotifyUsecase;
use crate::usecase::birth_reset_usecase::BirthResetUsecase;
use crate::usecase::birth_signup_usecase::BirthSignupUsecase;
use crate::usecase::guild_update_usecase::GuildUpdateUsecase;
use crate::usecase::seven_days_usecase::SevenDaysUsecase;
use serenity::all::Http;
use std::sync::Arc;

#[derive(Clone)]
pub struct Data {
    pub birth_list_usecase: BirthListUsecase,
    pub birth_signup_usecase: BirthSignupUsecase,
    pub birth_reset_usecase: BirthResetUsecase,
    pub birth_notify_usecase: BirthNotifyUsecase,
    pub guild_update_usecase: GuildUpdateUsecase,
    pub reminder_service: ReminderService,
    pub discord_http: Arc<Http>,
    pub seven_days_usecase: Option<SevenDaysUsecase>,
}
