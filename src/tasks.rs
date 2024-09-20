use blrs::fetching::authentication::GithubAuthentication;

#[derive(Debug, Clone)]
pub enum ConfigTask {
    UpdateGHAuth(GithubAuthentication),
    UpdateLastTimeChecked,
}
