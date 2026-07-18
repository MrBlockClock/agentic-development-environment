pub struct AnalyticsUploader;

impl AnalyticsUploader {
    pub fn new() -> Self {
        Self
    }

    pub async fn upload_batch(&self) -> Result<(), String> {
        // TODO: batch analytics events and upload to cloud
        Ok(())
    }
}
