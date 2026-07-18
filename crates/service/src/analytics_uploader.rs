pub struct AnalyticsUploader;

impl Default for AnalyticsUploader {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyticsUploader {
    pub fn new() -> Self {
        Self
    }

    pub async fn upload_batch(&self) -> Result<(), String> {
        // TODO: batch analytics events and upload to cloud
        Ok(())
    }
}
