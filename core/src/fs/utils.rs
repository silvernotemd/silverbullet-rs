use web_time::{SystemTime, UNIX_EPOCH};

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64) // https://github.com/silverbulletmd/silverbullet/issues/1762
        .unwrap_or(0)
}
