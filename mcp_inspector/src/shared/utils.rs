use anyhow::{Result, anyhow};
use std::time::Instant;

pub async fn measure_latency<F, Fut, T>(f: F) -> Result<(T, u64)>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let start = Instant::now();
    let res = f().await?;
    let elapsed = start.elapsed().as_millis() as u64;
    Ok((res, elapsed))
}

pub fn parse_command(cmd: &str) -> Result<(String, Vec<String>)> {
    // naive split by spaces respecting quotes (simple)
    let shell_words =
        shell_words::split(cmd).map_err(|e| anyhow!("failed to parse command '{}': {}", cmd, e))?;
    if shell_words.is_empty() {
        return Err(anyhow!("empty command"));
    }
    Ok((shell_words[0].clone(), shell_words[1..].to_vec()))
}
