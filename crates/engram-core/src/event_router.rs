use tokio::sync::broadcast;
use tracing::info;

use engram_types::events::EngramEvent;

/// Dispatches events to all layer agents via tokio broadcast channels.
/// engram-core never writes to Notion directly — it routes events to agents only.
pub struct EventRouter {
    sender: broadcast::Sender<EngramEvent>,
}

impl EventRouter {
    pub fn new() -> Self {
        // Buffer up to 256 events before lagging
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    /// Subscribe to the event stream. Each agent gets its own receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<EngramEvent> {
        self.sender.subscribe()
    }

    /// Dispatch an event to all subscribed agents.
    /// Cascade failures must not block the triggering event — each agent
    /// runs independently.
    pub fn dispatch(&self, event: EngramEvent) {
        let event_name = format!("{:?}", &event).chars().take(80).collect::<String>();
        match self.sender.send(event) {
            Ok(count) => {
                info!("Dispatched event to {count} agents: {event_name}...");
            }
            Err(e) => {
                // No subscribers yet — this is fine during startup
                tracing::warn!("No subscribers for event: {e}");
            }
        }
    }
}
