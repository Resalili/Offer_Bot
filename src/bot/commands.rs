use teloxide::prelude::*;

#[derive(Debug)]
pub enum BotCommand {
    Start,
    ProfileCreate,
    PostJob,
    Unknown,
}

impl From<&str> for BotCommand {
    fn from(s: &str) -> Self {
        match s {
            "/start" => BotCommand::Start,
            "/profile_create" => BotCommand::ProfileCreate,
            "/post_job" => BotCommand::PostJob,
            _ => BotCommand::Unknown,
        }
    }
}
