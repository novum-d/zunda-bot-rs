use crate::usecase::birth_list_usecase::BirthListUsecase;
use crate::usecase::birth_reset_usecase::BirthResetUsecase;
use crate::usecase::birth_signup_usecase::BirthSignupUsecase;
use crate::usecase::guild_update_usecase::GuildUpdateUsecase;

pub struct Data {
    pub birth_list_usecase: BirthListUsecase,
    pub birth_signup_usecase: BirthSignupUsecase,
    pub birth_reset_usecase: BirthResetUsecase,
    pub guild_update_usecase: GuildUpdateUsecase,
}
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'c> = poise::Context<'c, Data, Error>;
