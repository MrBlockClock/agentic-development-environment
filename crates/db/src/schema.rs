use tracing::info;

pub fn migrate() -> Result<(), String> {
    info!("Running database migrations...");
    // TODO: implement SQLx/refinery migrations
    Ok(())
}
