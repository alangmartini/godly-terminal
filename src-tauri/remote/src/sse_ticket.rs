use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

const TICKET_TTL_SECS: u64 = 30;

/// One-time SSE connection tickets.
/// Eliminates the need to pass API key or device token in the SSE URL query string,
/// which would be logged by proxies, browser history, and server access logs.
pub struct SseTicketStore {
    tickets: Mutex<HashMap<String, Instant>>,
}

impl SseTicketStore {
    pub fn new() -> Self {
        Self {
            tickets: Mutex::new(HashMap::new()),
        }
    }

    /// Create a one-time ticket valid for 30 seconds.
    pub fn create(&self) -> String {
        let ticket = uuid::Uuid::new_v4().to_string();
        let mut tickets = self.tickets.lock().unwrap();
        // Clean expired tickets
        tickets.retain(|_, created| created.elapsed().as_secs() < TICKET_TTL_SECS);
        tickets.insert(ticket.clone(), Instant::now());
        ticket
    }

    /// Consume a ticket. Returns true if valid and not expired.
    /// Tickets are one-time use — consumed on first validation.
    pub fn consume(&self, ticket: &str) -> bool {
        let mut tickets = self.tickets.lock().unwrap();
        // Clean expired tickets
        tickets.retain(|_, created| created.elapsed().as_secs() < TICKET_TTL_SECS);
        tickets.remove(ticket).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_consume() {
        let store = SseTicketStore::new();
        let ticket = store.create();
        assert!(store.consume(&ticket));
    }

    #[test]
    fn ticket_is_one_time_use() {
        let store = SseTicketStore::new();
        let ticket = store.create();
        assert!(store.consume(&ticket));
        assert!(!store.consume(&ticket)); // second use fails
    }

    #[test]
    fn invalid_ticket_rejected() {
        let store = SseTicketStore::new();
        assert!(!store.consume("nonexistent"));
    }
}
