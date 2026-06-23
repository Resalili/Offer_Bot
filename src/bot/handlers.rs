use axum::{extract::State, http::StatusCode, Json};
use teloxide::prelude::*;
use std::sync::Arc;
use tracing::info;

use crate::AppState;

pub async fn webhook_handler(
    State(state): State<Arc<AppState>>,
    Json(update): Json<teloxide::types::Update>,
) -> StatusCode {
    info!("received update: {:?}", update);

    match update.kind {
        teloxide::types::UpdateKind::Message(msg) => {
            let chat_id = msg.chat.id;
            let text_opt = msg.text().map(|s| s.to_string());
            let photo_file_id = msg.photo().as_ref().and_then(|v| v.last().map(|p| p.file.id.to_string()));
            let from_name = msg.from().map(|u| u.first_name.clone());
            let user_id_i64 = chat_id.to_string().parse::<i64>().unwrap_or(0);

            let db = state.db.clone();
            let bot = state.bot.clone();

            tokio::spawn(async move {
                tracing::info!("handler: incoming text={:?} photo={:?} user={}", text_opt, photo_file_id, user_id_i64);

                // Command / button text handling
                if let Some(text) = text_opt.clone() {
                    if text.starts_with("/start") {
                        // Check if user already exists and has a nickname; if not, show only Create Profile
                        if let Ok(existing) = crate::db::repo::get_user(&db, user_id_i64).await {
                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                            let keyboard = if let Some(u) = existing {
                                if u.nickname.is_some() {
                                    serde_json::json!({
                                        "inline_keyboard": [
                                            [ {"text": "Показати профіль", "callback_data": "show_profile"}, {"text": "Редагувати профіль", "callback_data": "edit_profile"} ],
                                            [ {"text": "Створити оффер", "callback_data": "create_job"}, {"text": "Пошук робіт", "callback_data": "search_jobs"} ],
                                            [ {"text": "Мої оффери", "callback_data": "show_my_jobs"} ]
                                        ]
                                    })
                                } else {
                                    serde_json::json!({
                                        "inline_keyboard": [ [ {"text": "Створити профіль", "callback_data": "create_profile"} ] ]
                                    })
                                }
                            } else {
                                serde_json::json!({
                                    "inline_keyboard": [ [ {"text": "Створити профіль", "callback_data": "create_profile"} ] ]
                                })
                            };
                            let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Привіт! Обери дію:", "reply_markup": keyboard});
                            let client = reqwest::Client::new();
                            let _ = client.post(&url).json(&body).send().await;
                            tracing::info!("sent /start inline keyboard to {}", chat_id);
                        }

                    } else if text.starts_with("/profile_create") || text == "Створити профіль" {
                        let name = from_name.clone().unwrap_or("User".to_string());
                        let user = crate::services::user::User { id: user_id_i64, name, nickname: None, avatar: None, description: None, skills: None, stage: Some(1), job_stage: Some(0), job_draft_title: None, job_draft_budget: None, job_draft_skills: None, job_draft_description: None };
                        let _ = crate::db::repo::save_user(&db, user).await;
                        let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                        let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Напиши свій нікнейм:"});
                        let client = reqwest::Client::new();
                        let _ = client.post(&url).json(&body).send().await;
                        tracing::info!("initiated profile for user {}", user_id_i64);

                    } else if text == "Опублікувати роботу" || text == "Створити оффер" {
                        let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                        let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Надішліть роботу в форматі: Назва | бюджет | навички (через кому). Приклад: Дизайн логотипу | 100 | логотип, дизайн"});
                        let client = reqwest::Client::new();
                        let _ = client.post(&url).json(&body).send().await;
                        tracing::info!("asked user {} to send job details", user_id_i64);

                    } else if text.starts_with("/post_job") || text.contains('|') {
                        // Accept either /post_job Title | budget | skills OR a plain message with '|' when user was asked
                        let rest = if text.starts_with("/post_job") { text.trim_start_matches("/post_job").trim() } else { text.as_str() };
                        let mut parts = rest.split('|');
                        let title = parts.next().unwrap_or("Untitled").trim().to_string();
                        let budget = parts.next().and_then(|b| b.trim().parse::<i32>().ok()).unwrap_or(0);
                        let skills = parts.next().map(|s| s.trim().to_string());
                        let description = parts.next().map(|s| s.trim().to_string());
                        let creator_id = msg.chat.id.to_string().parse::<i64>().unwrap_or(0);
                        let job = crate::services::job::Job { id: 0, title: title.clone(), budget, skills: skills.clone(), description: description.clone(), creator_id };
                        let job = crate::db::repo::save_job(&db, job).await.ok();
                        if let Some(j) = job {
                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                            let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": format!("Job створено: {} (budget: ${})", j.title, j.budget)});
                            let client = reqwest::Client::new();
                            let _ = client.post(&url).json(&body).send().await;
                            // reset user's job_stage
                            let _ = crate::db::repo::update_user_job_stage(&db, creator_id, 0).await;
                            tracing::info!("saved job id {} for user {}", j.id, creator_id);
                        }

                    } else if text.starts_with("/search") || text == "Пошук робіт" {
                        // matching: find best job for this user based on skills
                        let creator_id = msg.chat.id.to_string().parse::<i64>().unwrap_or(0);
                        let mut reply = "No jobs found".to_string();
                        if let Ok(Some(user)) = crate::db::repo::get_user(&db, creator_id).await {
                            let user_skills: Vec<String> = user
                                .skills
                                .clone()
                                .unwrap_or_default()
                                .split(',')
                                .map(|s| s.trim().to_lowercase())
                                .filter(|s| !s.is_empty())
                                .collect();

                                if let Ok(jobs) = crate::db::repo::get_all_jobs(&db).await {
                                use crate::utils::scoring::score;
                                let mut best: Option<(i32, crate::services::job::Job)> = None;
                                    for job in jobs.into_iter() {
                                    // title token overlap
                                    let title_tokens: Vec<String> = job.title
                                        .split(|c: char| !c.is_alphanumeric())
                                        .map(|s| s.trim().to_lowercase())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    let title_overlap = title_tokens.iter().filter(|t| user_skills.contains(t)).count() as i32;
                                    // skill overlap between user.skills and job.skills
                                    let job_skills: Vec<String> = job.skills.clone().unwrap_or_default()
                                        .split(',')
                                        .map(|s| s.trim().to_lowercase())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    let skill_overlap = job_skills.iter().filter(|s| user_skills.contains(s)).count() as i32;
                                    let budget_fit = if job.budget > 0 { 1 } else { 0 };
                                    let s = score(skill_overlap, title_overlap, budget_fit);
                                    if best.as_ref().map(|(sc,_)| *sc).unwrap_or(-1) < s {
                                        best = Some((s, job.clone()));
                                    }
                                }
                                    if let Some((_, best_job)) = best {
                                    reply = format!("Matched job: {} (budget: ${})\nНавички: {}", best_job.title, best_job.budget, best_job.skills.clone().unwrap_or_default());
                                    tracing::info!("matched job id={} for user {}", best_job.id, creator_id);
                                }
                            }
                        }
                        let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                        let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": reply});
                        let client = reqwest::Client::new();
                        let _ = client.post(&url).json(&body).send().await;
                    }
                }

                // Stepwise job creation using job_stage and per-user drafts
                if let Some(text) = text_opt.clone() {
                    if let Ok(Some(u)) = crate::db::repo::get_user(&db, user_id_i64).await {
                        match u.job_stage.unwrap_or(0) {
                            1 => {
                                // stage 1: received title
                                // save title
                                let _ = crate::db::repo::update_user_job_draft(&db, user_id_i64, Some(text.clone()), None, None, None).await;
                                let _ = crate::db::repo::update_user_job_stage(&db, user_id_i64, 2).await;
                                let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                let keyboard = serde_json::json!({"inline_keyboard": [[ {"text": "Повернутися в меню", "callback_data": "main_menu" } ]]});
                                let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Отримав назву. Тепер надішліть бюджет (ціле число):", "reply_markup": keyboard});
                                let client = reqwest::Client::new();
                                let _ = client.post(&url).json(&body).send().await;
                            }
                            2 => {
                                // stage 2: received budget
                                if let Ok(b) = text.trim().parse::<i32>() {
                                    let _ = crate::db::repo::update_user_job_draft(&db, user_id_i64, None, Some(b), None, None).await;
                                    let _ = crate::db::repo::update_user_job_stage(&db, user_id_i64, 3).await;
                                    let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                    let keyboard = serde_json::json!({"inline_keyboard": [[ {"text": "Повернутися в меню", "callback_data": "main_menu" } ]]});
                                    let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Бюджет збережено. Тепер надішліть опис оффера:", "reply_markup": keyboard});
                                    let client = reqwest::Client::new();
                                    let _ = client.post(&url).json(&body).send().await;
                                } else {
                                    let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                    let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Невірний формат бюджету. Надішліть ціле число, наприклад: 100"});
                                    let client = reqwest::Client::new();
                                    let _ = client.post(&url).json(&body).send().await;
                                }
                            }
                            3 => {
                                // stage 3: received description
                                let _ = crate::db::repo::update_user_job_draft(&db, user_id_i64, None, None, None, Some(text.clone())).await;
                                let _ = crate::db::repo::update_user_job_stage(&db, user_id_i64, 4).await;
                                let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                let keyboard = serde_json::json!({"inline_keyboard": [[ {"text": "Повернутися в меню", "callback_data": "main_menu" } ]]});
                                let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Опис збережено. Тепер надішліть навички (через кому):", "reply_markup": keyboard});
                                let client = reqwest::Client::new();
                                let _ = client.post(&url).json(&body).send().await;
                            }
                            4 => {
                                // stage 4: received skills -> show preview and ask for confirmation
                                let _ = crate::db::repo::update_user_job_draft(&db, user_id_i64, None, None, Some(text.clone()), None).await;
                                if let Ok(Some(final_u)) = crate::db::repo::get_user(&db, user_id_i64).await {
                                    let title = final_u.job_draft_title.clone().unwrap_or_else(|| "Untitled".to_string());
                                    let budget = final_u.job_draft_budget.unwrap_or(0);
                                    let skills = final_u.job_draft_skills.clone().unwrap_or_default();
                                    let desc = final_u.job_draft_description.clone().unwrap_or_default();
                                    let preview = format!("Прев'ю оффера:\n{}\nБюджет: ${}\nНавички: {}\nОпис: {}\n\nПідтвердьте публікацію:", title, budget, skills, desc);
                                    let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                    let keyboard = serde_json::json!({
                                        "inline_keyboard": [
                                            [ {"text": "Підтвердити", "callback_data": "confirm_job"}, {"text": "Скасувати", "callback_data": "cancel_job"} ],
                                            [ {"text": "Повернутися в меню", "callback_data": "main_menu"} ]
                                        ]
                                    });
                                    let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": preview, "reply_markup": keyboard});
                                    let client = reqwest::Client::new();
                                    let _ = client.post(&url).json(&body).send().await;
                                    let _ = crate::db::repo::update_user_job_stage(&db, user_id_i64, 5).await; // awaiting confirmation
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // handle non-command text as part of profile flow
                if let Some(text) = text_opt {
                    tracing::info!("profile-flow: received text='{}' for user={}", text, user_id_i64);
                    if let Ok(Some(u)) = crate::db::repo::get_user(&db, user_id_i64).await {
                        tracing::info!("profile-flow: db returned user={:?}", u);
                        if u.stage == Some(1) {
                            let _ = crate::db::repo::update_user_fields(&db, user_id_i64, Some(text.clone()), None, None, None).await;
                            let _ = crate::db::repo::update_user_stage(&db, user_id_i64, 2).await;
                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                            let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Надішли свою аватарку (photo):"});
                            let client = reqwest::Client::new();
                            let _ = client.post(&url).json(&body).send().await;
                            tracing::info!("profile-flow: nickname saved for user={}", user_id_i64);
                        } else if u.stage == Some(3) {
                            // stage 3: profile description
                            let _ = crate::db::repo::update_user_fields(&db, user_id_i64, None, None, Some(text.clone()), None).await;
                            let _ = crate::db::repo::update_user_stage(&db, user_id_i64, 4).await;
                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                            let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Опис збережено. Тепер надішліть свої навички (через кому):"});
                            let client = reqwest::Client::new();
                            let _ = client.post(&url).json(&body).send().await;
                        } else if u.stage == Some(4) {
                            // stage 4: skills -> finish profile
                            let _ = crate::db::repo::update_user_fields(&db, user_id_i64, None, None, None, Some(text.clone())).await;
                            let _ = crate::db::repo::update_user_stage(&db, user_id_i64, 0).await;
                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                            let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Профіль завершено."});
                            let client = reqwest::Client::new();
                            let _ = client.post(&url).json(&body).send().await;
                            // send main menu after profile completion
                            let keyboard = serde_json::json!({
                                "inline_keyboard": [
                                    [ {"text": "Показати профіль", "callback_data": "show_profile"}, {"text": "Редагувати профіль", "callback_data": "edit_profile"} ],
                                    [ {"text": "Створити оффер", "callback_data": "create_job"}, {"text": "Пошук робіт", "callback_data": "search_jobs"} ],
                                    [ {"text": "Мої оффери", "callback_data": "show_my_jobs"} ]
                                ]
                            });
                            let body2 = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Головне меню:", "reply_markup": keyboard});
                            let _ = client.post(&url).json(&body2).send().await;
                        }
                    }
                }

                // handle photo upload for avatar (when stage==2)
                if let Some(file_id) = photo_file_id {
                        if let Ok(Some(u)) = crate::db::repo::get_user(&db, user_id_i64).await {
                        if u.stage == Some(2) {
                            let _ = crate::db::repo::update_user_fields(&db, user_id_i64, None, Some(file_id.clone()), None, None).await;
                            let _ = crate::db::repo::update_user_stage(&db, user_id_i64, 3).await;
                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                            let body = serde_json::json!({"chat_id": chat_id.to_string(), "text": "Аватар отримано. Тепер надішли опис профілю (коротко):"});
                            let client = reqwest::Client::new();
                            let _ = client.post(&url).json(&body).send().await;
                        }
                    }
                }
            });
        }
            teloxide::types::UpdateKind::CallbackQuery(cbq) => {
            if let Some(data) = cbq.data.clone() {
                let chat_id = if let Some(m) = cbq.message.as_ref() { m.chat().id.to_string() } else { cbq.from.id.to_string() };
                let user_id_i64 = cbq.from.id.0 as i64;
                    let db = state.db.clone();
                    tokio::spawn(async move {
                        tracing::info!("callback received from {} data={}", user_id_i64, data);
                        match data.as_str() {
                            "create_profile" => {
                                let name = cbq.from.first_name.clone();
                                let user = crate::services::user::User { id: user_id_i64, name, nickname: None, avatar: None, description: None, skills: None, stage: Some(1), job_stage: Some(0), job_draft_title: None, job_draft_budget: None, job_draft_skills: None, job_draft_description: None };
                                let _ = crate::db::repo::save_user(&db, user).await;
                                let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                let body = serde_json::json!({"chat_id": chat_id, "text": "Напиши свій нікнейм:"});
                                let client = reqwest::Client::new();
                                let _ = client.post(&url).json(&body).send().await;
                            }
                            "create_job" => {
                                // start stepwise job creation: ask only for title
                                let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                let _ = crate::db::repo::update_user_job_stage(&db, user_id_i64, 1).await;
                                let keyboard = serde_json::json!({"inline_keyboard": [[ {"text": "Повернутися в меню", "callback_data": "main_menu" } ]]});
                                let body = serde_json::json!({"chat_id": chat_id, "text": "Надішліть назву оффера:", "reply_markup": keyboard});
                                let client = reqwest::Client::new();
                                let _ = client.post(&url).json(&body).send().await;
                            }
                        "confirm_job" => {
                            if let Ok(Some(final_u)) = crate::db::repo::get_user(&db, user_id_i64).await {
                                let title = final_u.job_draft_title.clone().unwrap_or_else(|| "Untitled".to_string());
                                let budget = final_u.job_draft_budget.unwrap_or(0);
                                let skills = final_u.job_draft_skills.clone();
                                let desc = final_u.job_draft_description.clone();
                                let job = crate::services::job::Job { id: 0, title: title.clone(), budget, skills: skills.clone(), description: desc.clone(), creator_id: user_id_i64 };
                                    match crate::db::repo::save_job(&db, job).await {
                                        Ok(j) => {
                                            tracing::info!("confirm_job: saved job id={} skills={:?}", j.id, j.skills);
                                            let _ = crate::db::repo::update_user_job_stage(&db, user_id_i64, 0).await;
                                            let _ = crate::db::repo::clear_user_job_draft(&db, user_id_i64).await;
                                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                            let body = serde_json::json!({"chat_id": chat_id, "text": format!("Job опубліковано: {} (budget: ${})", j.title, j.budget)});
                                            let client = reqwest::Client::new();
                                            let _ = client.post(&url).json(&body).send().await;
                                            tracing::info!("confirm_job: sent confirmation message for job id={}", j.id);
                                            // send main menu after publishing
                                            let keyboard = serde_json::json!({
                                                "inline_keyboard": [
                                                    [ {"text": "Показати профіль", "callback_data": "show_profile"}, {"text": "Редагувати профіль", "callback_data": "edit_profile"} ],
                                                    [ {"text": "Створити оффер", "callback_data": "create_job"}, {"text": "Пошук робіт", "callback_data": "search_jobs"} ],
                                                    [ {"text": "Мої оффери", "callback_data": "show_my_jobs"} ]
                                                ]
                                            });
                                            let body2 = serde_json::json!({"chat_id": chat_id, "text": "Головне меню:", "reply_markup": keyboard});
                                            let _ = client.post(&url).json(&body2).send().await;
                                            tracing::info!("confirm_job: sent main menu after publishing job id={}", j.id);
                                        }
                                        Err(e) => {
                                            tracing::error!("confirm_job: failed to save job: {:?}", e);
                                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                            let body = serde_json::json!({"chat_id": chat_id, "text": "Помилка при збереженні оффера."});
                                            let client = reqwest::Client::new();
                                            let _ = client.post(&url).json(&body).send().await;
                                        }
                                    }
                            }
                        }
                        "cancel_job" => {
                            let _ = crate::db::repo::clear_user_job_draft(&db, user_id_i64).await;
                            let _ = crate::db::repo::update_user_job_stage(&db, user_id_i64, 0).await;
                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                            let body = serde_json::json!({"chat_id": chat_id, "text": "Створення оффера скасовано."});
                            let client = reqwest::Client::new();
                            let _ = client.post(&url).json(&body).send().await;
                            // return to main menu
                            let keyboard = serde_json::json!({
                                "inline_keyboard": [
                                    [ {"text": "Показати профіль", "callback_data": "show_profile"}, {"text": "Редагувати профіль", "callback_data": "edit_profile"} ],
                                    [ {"text": "Створити оффер", "callback_data": "create_job"}, {"text": "Пошук робіт", "callback_data": "search_jobs"} ],
                                    [ {"text": "Мої оффери", "callback_data": "show_my_jobs"} ]
                                ]
                            });
                            let body2 = serde_json::json!({"chat_id": chat_id, "text": "Головне меню:", "reply_markup": keyboard});
                            let _ = client.post(&url).json(&body2).send().await;
                        }
                        "main_menu" => {
                            // send main menu
                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                            let keyboard = serde_json::json!({
                                "inline_keyboard": [
                                    [ {"text": "Показати профіль", "callback_data": "show_profile"}, {"text": "Редагувати профіль", "callback_data": "edit_profile"} ],
                                    [ {"text": "Створити оффер", "callback_data": "create_job"}, {"text": "Пошук робіт", "callback_data": "search_jobs"} ],
                                    [ {"text": "Мої оффери", "callback_data": "show_my_jobs"} ]
                                ]
                            });
                            let body = serde_json::json!({"chat_id": chat_id, "text": "Головне меню:", "reply_markup": keyboard});
                            let client = reqwest::Client::new();
                            let _ = client.post(&url).json(&body).send().await;
                        }
                        "edit_profile" => {
                            let _ = crate::db::repo::update_user_stage(&db, user_id_i64, 1).await;
                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                            let body = serde_json::json!({"chat_id": chat_id, "text": "Редагування профілю: напишіть новий нікнейм:"});
                            let client = reqwest::Client::new();
                            let _ = client.post(&url).json(&body).send().await;
                        }
                            "search_jobs" => {
                                // improved search: consider both user skills and job skills
                                let mut reply = "No jobs found".to_string();
                                if let Ok(Some(user)) = crate::db::repo::get_user(&db, user_id_i64).await {
                                    let user_skills: Vec<String> = user
                                        .skills
                                        .clone()
                                        .unwrap_or_default()
                                        .split(',')
                                        .map(|s| s.trim().to_lowercase())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    tracing::info!("search_jobs: user_skills={:?} for user={}", user_skills, user_id_i64);
                                    if let Ok(jobs) = crate::db::repo::get_all_jobs(&db).await {
                                        use crate::utils::scoring::score;
                                            // collect candidate jobs where at least one skill or title token overlaps
                                            let mut candidates: Vec<(i32, i32, i32, crate::services::job::Job)> = Vec::new();
                                            for job in jobs.into_iter().filter(|j| j.creator_id != user_id_i64) {
                                                let title_tokens: Vec<String> = job.title
                                                    .split(|c: char| !c.is_alphanumeric())
                                                    .map(|s| s.trim().to_lowercase())
                                                    .filter(|s| !s.is_empty())
                                                    .collect();
                                                let title_overlap = title_tokens.iter().filter(|t| user_skills.contains(t)).count() as i32;
                                                let job_skills: Vec<String> = job.skills.clone().unwrap_or_default()
                                                    .split(',')
                                                    .map(|s| s.trim().to_lowercase())
                                                    .filter(|s| !s.is_empty())
                                                    .collect();
                                                let skill_overlap = job_skills.iter().filter(|s| user_skills.contains(s)).count() as i32;
                                                let budget_fit = if job.budget > 0 { 1 } else { 0 };
                                                let s = score(skill_overlap, title_overlap, budget_fit);
                                                tracing::info!("search_jobs: eval job_id={} title='{}' job_skills={:?} skill_overlap={} title_overlap={} score={}", job.id, job.title, job_skills, skill_overlap, title_overlap, s);
                                                if skill_overlap > 0 || title_overlap > 0 {
                                                    candidates.push((s, skill_overlap, title_overlap, job.clone()));
                                                }
                                            }
                                            if !candidates.is_empty() {
                                                candidates.sort_by(|a,b| b.0.cmp(&a.0));
                                                let (_score, _sk, _ti, best_job) = candidates[0].clone();
                                                reply = format!("Matched job: {} (budget: ${})\nНавички: {}", best_job.title, best_job.budget, best_job.skills.clone().unwrap_or_default());
                                                tracing::info!("search_jobs: matched job id={} for user={}", best_job.id, user_id_i64);
                                            } else {
                                                tracing::info!("search_jobs: no candidate jobs matching user_skills for user={}", user_id_i64);
                                            }
                                    }
                                }
                                let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                let body = serde_json::json!({"chat_id": chat_id, "text": reply});
                                let client = reqwest::Client::new();
                                let _ = client.post(&url).json(&body).send().await;
                            }
                            "show_profile" => {
                                if let Ok(Some(user)) = crate::db::repo::get_user(&db, user_id_i64).await {
                                    let mut profile_text = format!("Профіль: {}\n", user.name);
                                    if let Some(n) = user.nickname.clone() { profile_text.push_str(&format!("Нік: {}\n", n)); }
                                    if let Some(d) = user.description.clone() { profile_text.push_str(&format!("Опис: {}\n", d)); }
                                    if let Some(s) = user.skills.clone() { profile_text.push_str(&format!("Навички: {}\n", s)); }
                                    let keyboard = serde_json::json!({
                                        "inline_keyboard": [
                                            [ {"text": "Редагувати профіль", "callback_data": "edit_profile" } ],
                                            [ {"text": "Повернутися в меню", "callback_data": "main_menu" } ]
                                        ]
                                    });
                                    if let Some(av) = user.avatar.clone() {
                                        // send photo with caption and edit button
                                        let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                        let url = format!("https://api.telegram.org/bot{}/sendPhoto", token);
                                        let body = serde_json::json!({"chat_id": chat_id, "photo": av, "caption": profile_text, "reply_markup": keyboard});
                                        let client = reqwest::Client::new();
                                        let _ = client.post(&url).json(&body).send().await;
                                    } else {
                                        let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                        let body = serde_json::json!({"chat_id": chat_id, "text": profile_text, "reply_markup": keyboard});
                                        let client = reqwest::Client::new();
                                        let _ = client.post(&url).json(&body).send().await;
                                    }
                                }
                            }
                            "show_my_jobs" => {
                                if let Ok(jobs) = crate::db::repo::get_all_jobs(&db).await {
                                    let my_jobs: Vec<_> = jobs.into_iter().filter(|j| j.creator_id == user_id_i64).collect();
                                    if my_jobs.is_empty() {
                                        let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                        let body = serde_json::json!({"chat_id": chat_id, "text": "У вас немає створених офферів."});
                                        let client = reqwest::Client::new();
                                        let _ = client.post(&url).json(&body).send().await;
                                    } else {
                                        for j in my_jobs {
                                            let text = format!("{}\nБюджет: ${}\nНавички: {}", j.title, j.budget, j.skills.clone().unwrap_or_default());
                                            let token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
                                            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                                            let body = serde_json::json!({"chat_id": chat_id, "text": text});
                                            let client = reqwest::Client::new();
                                            let _ = client.post(&url).json(&body).send().await;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    });
                }
            }
            _ => {}
    }

    StatusCode::OK
}
