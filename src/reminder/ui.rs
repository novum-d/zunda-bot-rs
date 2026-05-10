use crate::reminder::service::ReminderUiCandidate;
use serenity::all::{
    ButtonStyle, CreateActionRow, CreateButton, CreateSelectMenu, CreateSelectMenuKind,
    CreateSelectMenuOption,
};
use std::collections::HashSet;

pub const PAGE_SIZE: usize = 25;
const OPEN_PREFIX: &str = "reminder_ui_open";
const PAGE_PREFIX: &str = "reminder_ui_page";
const SELECT_PREFIX: &str = "reminder_ui_select";
const RUN_PREFIX: &str = "reminder_ui_run";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReminderUiAction {
    Open {
        owner_id: i64,
        guild_id: i64,
        session_id: String,
    },
    Page {
        owner_id: i64,
        guild_id: i64,
        session_id: String,
        page: usize,
    },
    Select {
        owner_id: i64,
        guild_id: i64,
        session_id: String,
        page: usize,
    },
    Run {
        owner_id: i64,
        guild_id: i64,
        session_id: String,
    },
}

pub fn start_button_custom_id(owner_id: i64, guild_id: i64, session_id: &str) -> String {
    format!("{OPEN_PREFIX}:{owner_id}:{guild_id}:{session_id}")
}

pub fn parse_reminder_ui_custom_id(custom_id: &str) -> Option<ReminderUiAction> {
    let mut parts = custom_id.split(':');
    let prefix = parts.next()?;
    let owner_id = parts.next()?.parse::<i64>().ok()?;
    let guild_id = parts.next()?.parse::<i64>().ok()?;
    let session_id = parts.next()?.to_string();

    match prefix {
        OPEN_PREFIX if parts.next().is_none() => Some(ReminderUiAction::Open {
            owner_id,
            guild_id,
            session_id,
        }),
        RUN_PREFIX if parts.next().is_none() => Some(ReminderUiAction::Run {
            owner_id,
            guild_id,
            session_id,
        }),
        PAGE_PREFIX | SELECT_PREFIX => {
            let page = parts.next()?.parse::<usize>().ok()?;
            if parts.next().is_some() {
                return None;
            }

            if prefix == PAGE_PREFIX {
                Some(ReminderUiAction::Page {
                    owner_id,
                    guild_id,
                    session_id,
                    page,
                })
            } else {
                Some(ReminderUiAction::Select {
                    owner_id,
                    guild_id,
                    session_id,
                    page,
                })
            }
        }
        _ => None,
    }
}

pub fn start_components(owner_id: i64, guild_id: i64, session_id: &str) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![CreateButton::new(
        start_button_custom_id(owner_id, guild_id, session_id),
    )
    .label("ユーザーを選ぶのだ")
    .style(ButtonStyle::Primary)])]
}

pub fn build_selection_content(candidates: &[ReminderUiCandidate], page: usize) -> String {
    if candidates.is_empty() {
        return "誕生日が未登録のユーザーはいないのだ！".to_string();
    }

    let total_pages = total_pages(candidates.len());
    let current_page = page.min(total_pages.saturating_sub(1));
    let page_candidates = page_candidates(candidates, current_page);
    let mut lines = vec![
        format!(
            "誕生日が未登録のユーザーなのだ！（{} / {}ページ）",
            current_page + 1,
            total_pages
        ),
        format!("対象ユーザー：{}人（Botは除外済み）", candidates.len()),
        String::new(),
    ];

    for (index, candidate) in page_candidates.iter().enumerate() {
        lines.push(format!("{}. {}", index + 1, candidate.display_name));
    }

    lines.join("\n")
}

pub fn build_selection_components(
    owner_id: i64,
    guild_id: i64,
    session_id: &str,
    candidates: &[ReminderUiCandidate],
    selected_member_ids: &[i64],
    page: usize,
) -> Vec<CreateActionRow> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let selected_member_ids = selected_member_ids.iter().copied().collect::<HashSet<_>>();
    let total_pages = total_pages(candidates.len());
    let current_page = page.min(total_pages.saturating_sub(1));
    let page_candidates = page_candidates(candidates, current_page);
    let max_values = u8::try_from(page_candidates.len()).unwrap_or(25);

    let options = page_candidates
        .iter()
        .map(|candidate| {
            CreateSelectMenuOption::new(&candidate.display_name, candidate.member_id.to_string())
                .default_selection(selected_member_ids.contains(&candidate.member_id))
        })
        .collect::<Vec<_>>();

    let select = CreateSelectMenu::new(
        format!("{SELECT_PREFIX}:{owner_id}:{guild_id}:{session_id}:{current_page}"),
        CreateSelectMenuKind::String { options },
    )
    .placeholder("ユーザーを選択（複数可）")
    .min_values(0)
    .max_values(max_values);

    let previous_page = current_page.saturating_sub(1);
    let next_page = current_page + 1;
    let previous_button = CreateButton::new(format!(
        "{PAGE_PREFIX}:{owner_id}:{guild_id}:{session_id}:{previous_page}"
    ))
    .label("← 前へ")
    .style(ButtonStyle::Secondary)
    .disabled(current_page == 0);
    let next_button = CreateButton::new(format!(
        "{PAGE_PREFIX}:{owner_id}:{guild_id}:{session_id}:{next_page}"
    ))
    .label("次へ →")
    .style(ButtonStyle::Secondary)
    .disabled(current_page + 1 >= total_pages);
    let run_button = CreateButton::new(format!("{RUN_PREFIX}:{owner_id}:{guild_id}:{session_id}"))
        .label("実行するのだ")
        .style(ButtonStyle::Success)
        .disabled(selected_member_ids.is_empty());

    vec![
        CreateActionRow::SelectMenu(select),
        CreateActionRow::Buttons(vec![previous_button, next_button, run_button]),
    ]
}

pub fn page_member_ids(candidates: &[ReminderUiCandidate], page: usize) -> Vec<i64> {
    page_candidates(candidates, page)
        .iter()
        .map(|candidate| candidate.member_id)
        .collect()
}

fn page_candidates(candidates: &[ReminderUiCandidate], page: usize) -> &[ReminderUiCandidate] {
    let start = page.saturating_mul(PAGE_SIZE).min(candidates.len());
    let end = (start + PAGE_SIZE).min(candidates.len());
    &candidates[start..end]
}

fn total_pages(candidate_count: usize) -> usize {
    candidate_count.div_ceil(PAGE_SIZE).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(member_id: i64) -> ReminderUiCandidate {
        ReminderUiCandidate {
            member_id,
            display_name: format!("user{member_id}"),
        }
    }

    #[test]
    fn custom_id_round_trips_open_action() {
        let custom_id = start_button_custom_id(1, 2, "abc");

        assert_eq!(
            parse_reminder_ui_custom_id(&custom_id),
            Some(ReminderUiAction::Open {
                owner_id: 1,
                guild_id: 2,
                session_id: "abc".to_string(),
            })
        );
    }

    #[test]
    fn page_member_ids_returns_current_page_ids() {
        let candidates = (1..=30).map(candidate).collect::<Vec<_>>();

        assert_eq!(page_member_ids(&candidates, 1), vec![26, 27, 28, 29, 30]);
    }
}
