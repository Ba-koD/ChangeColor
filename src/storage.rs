use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use poise::serenity_prelude as serenity;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::crypto::{StateCipher, is_encrypted};
use crate::{Error, user_error};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct Store {
    #[serde(default)]
    pub(crate) guilds: BTreeMap<u64, GuildConfig>,
}

impl Store {
    pub(crate) fn guild_mut(&mut self, guild_id: serenity::GuildId) -> &mut GuildConfig {
        self.guilds.entry(guild_id.get()).or_default()
    }

    pub(crate) fn guild_config(&self, guild_id: serenity::GuildId) -> GuildConfig {
        self.guilds
            .get(&guild_id.get())
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GuildConfig {
    #[serde(default)]
    pub(crate) allowed_role_ids: BTreeSet<u64>,
    #[serde(default)]
    pub(crate) loss_policy: LossPolicy,
    #[serde(default)]
    pub(crate) anchor_role_id: Option<u64>,
    #[serde(default)]
    pub(crate) marker_start_role_id: Option<u64>,
    #[serde(default)]
    pub(crate) marker_end_role_id: Option<u64>,
    #[serde(default)]
    pub(crate) color_roles: BTreeMap<String, u64>,
    #[serde(default)]
    pub(crate) users: BTreeMap<u64, UserColorState>,
}

impl Default for GuildConfig {
    fn default() -> Self {
        Self {
            allowed_role_ids: BTreeSet::new(),
            loss_policy: LossPolicy::RemoveImmediate,
            anchor_role_id: None,
            marker_start_role_id: None,
            marker_end_role_id: None,
            color_roles: BTreeMap::new(),
            users: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub(crate) enum LossPolicy {
    Keep,
    RemoveImmediate,
    RemoveAfter { grace_days: u8 },
}

impl Default for LossPolicy {
    fn default() -> Self {
        Self::RemoveImmediate
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct UserColorState {
    #[serde(default)]
    pub(crate) last_hex: Option<String>,
    #[serde(default)]
    pub(crate) current_role_id: Option<u64>,
    #[serde(default)]
    pub(crate) lost_eligibility_at: Option<i64>,
}

pub(crate) struct Storage {
    path: PathBuf,
    cipher: Option<StateCipher>,
    inner: RwLock<Store>,
}

impl Storage {
    /// Loads the state file. With a cipher, an encrypted file is decrypted and a
    /// plaintext file from an older version is re-written encrypted right away.
    pub(crate) fn load(path: PathBuf, cipher: Option<StateCipher>) -> Result<Self, Error> {
        let (store, needs_encryption) = if path.exists() {
            let bytes = std::fs::read(&path)?;
            if is_encrypted(&bytes) {
                let cipher = cipher.as_ref().ok_or_else(|| {
                    user_error("state file is encrypted but DATA_ENCRYPTION_KEY is not set")
                })?;
                (serde_json::from_slice(&cipher.decrypt(&bytes)?)?, false)
            } else {
                (serde_json::from_slice(&bytes)?, cipher.is_some())
            }
        } else {
            (Store::default(), false)
        };

        if needs_encryption {
            write_state(&path, cipher.as_ref(), &store)?;
        }
        Ok(Self {
            path,
            cipher,
            inner: RwLock::new(store),
        })
    }

    pub(crate) async fn guild_config(&self, guild_id: serenity::GuildId) -> GuildConfig {
        self.inner.read().await.guild_config(guild_id)
    }

    pub(crate) async fn update_guild<R>(
        &self,
        guild_id: serenity::GuildId,
        update: impl FnOnce(&mut GuildConfig) -> R,
    ) -> Result<R, Error> {
        let mut store = self.inner.write().await;
        let result = update(store.guild_mut(guild_id));
        self.persist(&store)?;
        Ok(result)
    }

    pub(crate) async fn guild_ids(&self) -> Vec<serenity::GuildId> {
        self.inner
            .read()
            .await
            .guilds
            .keys()
            .map(|id| serenity::GuildId::new(*id))
            .collect()
    }

    fn persist(&self, store: &Store) -> Result<(), Error> {
        write_state(&self.path, self.cipher.as_ref(), store)
    }
}

fn write_state(path: &Path, cipher: Option<&StateCipher>, store: &Store) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp_path = tmp_path_for(path);
    let mut bytes = serde_json::to_vec_pretty(store)?;
    if let Some(cipher) = cipher {
        bytes = cipher.encrypt(&bytes)?;
    }
    std::fs::write(&tmp_path, bytes)?;
    std::fs::rename(tmp_path, path)?;
    Ok(())
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut tmp_path = path.to_path_buf();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!("{extension}.tmp"))
        .unwrap_or_else(|| "tmp".to_string());
    tmp_path.set_extension(extension);
    tmp_path
}
