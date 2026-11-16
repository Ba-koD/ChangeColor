pub(crate) const START_MARKER_NAME: &str = "======= COLOR START =======";
pub(crate) const END_MARKER_NAME: &str = "======= COLOR END =======";
pub(crate) const DEFAULT_DATA_PATH: &str = "./data/state.json";
pub(crate) const MAX_GRACE_DAYS: u8 = 7;
pub(crate) const ROLE_LIMIT: usize = 250;
/// GUILD_MEMBERS 인텐트가 없을 때 허용 역할 상실을 확인하는 주기(초)
pub(crate) const MEMBER_SYNC_INTERVAL_SECS: u64 = 180;
pub(crate) const CLEAR_COMMANDS: &str = "clear-commands";
pub(crate) const CLEAR_GLOBAL_COMMANDS: &str = "clear-global-commands";
pub(crate) const CLEAR_GUILD_COMMANDS: &str = "clear-guild-commands";
