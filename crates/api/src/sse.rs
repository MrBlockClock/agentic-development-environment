use axum::response::sse::Event;
use tokio::sync::broadcast;

pub struct SseManager {
    tx: broadcast::Sender<Event>,
}

impl Default for SseManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SseManager {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { tx }
    }

    pub fn send_event(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}
