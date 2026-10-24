use std::collections::HashMap;

use anyhow::Context as _;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ForumConfig {
    pub server_id: u64,
    pub channel_id: u64,
    pub solved_tag_id: u64,
    pub whitelist: Vec<u64>,
    #[serde(deserialize_with = "id_map")]
    pub tag_roles: HashMap<u64, u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct HoneypotConfig {
    pub channel_id: u64,
    pub exempt_roles: Vec<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct UrlsConfig {
    pub site_url: String,
    pub uptime_url: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CooldownExemptConfig {
    pub user_ids: Vec<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_prefix")]
    pub prefix: String,
    #[serde(default)]
    pub forum: ForumConfig,
    #[serde(default)]
    pub honeypot: HoneypotConfig,
    // usernames excluded from leaderboards
    #[serde(default)]
    pub excluded: Vec<String>,
    #[serde(default)]
    pub urls: UrlsConfig,
    #[serde(default, rename = "cooldown_exempt_ids")]
    pub cooldown_exempt: CooldownExemptConfig,
}

fn default_prefix() -> String {
    "$".into()
}

// toml table keys are always strings, so tag -> role pairs need converting
fn id_map<'de, D: serde::Deserializer<'de>>(d: D) -> Result<HashMap<u64, u64>, D::Error> {
    let raw = HashMap::<String, u64>::deserialize(d)?;
    raw.into_iter()
        .map(|(k, v)| k.trim().parse::<u64>().map(|k| (k, v)).map_err(serde::de::Error::custom))
        .collect()
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
        let cfg: Config = toml::from_str(&text).with_context(|| format!("parsing {path}"))?;

        let critical = [
            ("forum.server_id", cfg.forum.server_id),
            ("forum.channel_id", cfg.forum.channel_id),
            ("forum.solved_tag_id", cfg.forum.solved_tag_id),
            ("honeypot.channel_id", cfg.honeypot.channel_id),
        ];
        for (name, val) in critical {
            if val == 0 {
                tracing::warn!("config: '{name}' is 0 - events for this value will be silently ignored");
            }
        }
        Ok(cfg)
    }

    pub fn is_cooldown_exempt(&self, user_id: u64) -> bool {
        self.cooldown_exempt.user_ids.contains(&user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repo_config() {
        let cfg = Config::load("config.toml").unwrap();
        assert_eq!(cfg.prefix, "+");
        assert_eq!(cfg.forum.tag_roles.len(), 4);
        assert_eq!(cfg.forum.tag_roles[&1226385419860447313], 1227988359910260737);
        assert!(cfg.is_cooldown_exempt(989106479699210310));
        assert_eq!(cfg.excluded, vec!["iteachchem".to_string()]);
    }
}
