use axum::response::sse::Event;
use tokio::sync::broadcast;

#[derive(Clone)]
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

    pub fn send_event(&self, event: Event) -> usize {
        self.tx.send(event).unwrap_or(0)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}
