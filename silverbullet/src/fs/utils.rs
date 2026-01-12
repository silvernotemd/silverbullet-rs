#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64) // https://github.com/silverbulletmd/silverbullet/issues/1762
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn now() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64) // https://github.com/silverbulletmd/silverbullet/issues/1762
        .unwrap_or(0)
}
