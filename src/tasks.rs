use blrs::BLRSConfig;
use chrono::Utc;

#[derive(Debug, Clone)]
pub enum ConfigTask {
    UpdateLastTimeChecked,
}

impl ConfigTask {
    pub fn eval(self, cfg: &mut BLRSConfig) {
        match self {
            Self::UpdateLastTimeChecked => {
                let dt = Utc::now();
                cfg.history.last_time_checked = Some(dt);
            }
        }
    }
}
