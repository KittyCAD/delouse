use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    delouse::init()?;

    let handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            eprintln!("heartbeat");
        }
    });
    handle.await?;
    Ok(())
}
