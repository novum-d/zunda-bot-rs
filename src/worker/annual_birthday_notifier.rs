use crate::models::common::Error;
use crate::usecase::birth_notify_usecase::BirthNotifyUsecase;
use chrono::{Local, NaiveTime, TimeZone};
use chrono_tz::Asia::Tokyo;
use std::time::Duration;

pub struct AnnualBirthdayNotifier;

impl AnnualBirthdayNotifier {
    pub async fn new(birth_notify_usecase: BirthNotifyUsecase) -> anyhow::Result<(), Error> {
        // 初回の誕生日チェックまでの時間を調節
        let now = Tokyo.from_utc_datetime(&Local::now().naive_utc());
        let noon = NaiveTime::from_hms_opt(12, 0, 0).expect("Invalid time.");
        let today_noon = now.date_naive().and_time(noon);
        let next_noon = if now.naive_local() < today_noon {
            today_noon
        } else {
            // 通知チェックの時刻を過ぎていた場合は、チェック時刻を明日に振替
            today_noon + chrono::Duration::days(1)
        };
        let wait = u64::try_from((next_noon - now.naive_local()).num_seconds().max(0))?;

        tokio::time::sleep(Duration::from_secs(wait)).await;

        // 誕生日チェック
        birth_notify_usecase.invoke().await?;

        // 以降は24時間ごとに正午(12:00)のタイミングで誕生日チェック実行
        loop {
            tokio::time::sleep(Duration::from_secs(60 * 60 * 24)).await;
            birth_notify_usecase.invoke().await?;
        }
    }
}
