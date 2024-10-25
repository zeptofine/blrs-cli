use blrs::{fetching::authentication::GithubAuthentication, BLRSConfig};
use chrono::Utc;

#[derive(Debug, Clone)]
pub enum ConfigTask {
    UpdateGHAuth(GithubAuthentication),
    UpdateLastTimeChecked,
}

impl ConfigTask {
    pub fn eval(self, cfg: &mut BLRSConfig) {
        match self {
            Self::UpdateGHAuth(github_authentication) => {
                cfg.update_github_authentication(Some(github_authentication));
            }
            Self::UpdateLastTimeChecked => {
                let dt = Utc::now();
                cfg.history.last_time_checked = Some(dt);
            }
        }
    }
}
