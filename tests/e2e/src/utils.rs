use std::fmt::Display;
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

pub async fn retry_async<F, Fut, T, E>(
    mut operation: F,
    retries: u16,
    delay_seconds: u16,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: Display,
{
    let mut attempts = 0;

    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) if attempts < retries => {
                eprintln!("Attempt {} failed: {}", attempts + 1, err);
                attempts += 1;
                eprintln!("Sleeping for {} seconds", delay_seconds);
                sleep(Duration::from_secs(delay_seconds.into())).await;
            }
            Err(err) => {
                eprintln!("Final attempt failed with error: {}", err);
                return Err(err);
            }
        }
    }
}
