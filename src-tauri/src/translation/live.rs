//! Bounds live work, cancels superseded revisions, and guards DB commits.
use crate::llm::provider::CancelToken;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct Ticket {
    pub key: String,
    pub id: String,
    pub session: String,
    pub cancel: Arc<CancelToken>,
}
#[derive(Default)]
struct Session {
    id: String,
    meeting: String,
    tickets: HashMap<String, Arc<Ticket>>,
}
pub struct LiveTranslations {
    session: Mutex<Session>,
    pub gate: Arc<tokio::sync::Semaphore>,
}

impl Default for LiveTranslations {
    fn default() -> Self {
        Self {
            session: Mutex::new(Session::default()),
            gate: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
impl LiveTranslations {
    pub fn clear(&self) {
        let mut session = self.session.lock().unwrap();
        for ticket in session.tickets.values() {
            ticket.cancel.cancel();
        }
        *session = Session::default();
    }
    pub fn begin(&self, meeting: String) -> String {
        let mut session = self.session.lock().unwrap();
        for ticket in session.tickets.values() {
            ticket.cancel.cancel();
        }
        let id = uuid::Uuid::new_v4().to_string();
        *session = Session {
            id: id.clone(),
            meeting,
            tickets: HashMap::new(),
        };
        id
    }
    pub fn end(&self, id: &str) {
        let mut session = self.session.lock().unwrap();
        if session.id == id {
            for ticket in session.tickets.values() {
                ticket.cancel.cancel();
            }
            *session = Session::default();
        }
    }
    pub fn reserve(
        &self,
        session_id: &str,
        meeting: &str,
        segment: &str,
        target: &str,
        id: String,
    ) -> Result<Arc<Ticket>, String> {
        let mut session = self.session.lock().map_err(|e| e.to_string())?;
        if session.id != session_id || session.meeting != meeting || session_id.is_empty() {
            return Err("Translation session ended".into());
        }
        let key = format!("{}:{}", segment, target);
        if session.tickets.len() >= 128 && !session.tickets.contains_key(&key) {
            return Err("Translation queue is full; pause or use a faster model".into());
        }
        let ticket = Arc::new(Ticket {
            key: key.clone(),
            id,
            session: session_id.into(),
            cancel: Arc::new(CancelToken::new()),
        });
        if let Some(previous) = session.tickets.insert(key, ticket.clone()) {
            previous.cancel.cancel();
        }
        Ok(ticket)
    }
    pub fn finish(&self, ticket: &Ticket, commit: impl FnOnce()) -> bool {
        let mut session = self.session.lock().unwrap();
        let current = !ticket.cancel.is_cancelled()
            && session.id == ticket.session
            && session
                .tickets
                .get(&ticket.key)
                .map(|t| t.id == ticket.id)
                .unwrap_or(false);
        if current {
            commit();
            session.tickets.remove(&ticket.key);
        }
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_pending_segments_but_allows_corrections() {
        let jobs = LiveTranslations::default();
        let session = jobs.begin("meeting".into());
        for index in 0..128 {
            jobs.reserve(
                &session,
                "meeting",
                &index.to_string(),
                "vi",
                index.to_string(),
            )
            .unwrap();
        }
        assert!(jobs
            .reserve(&session, "meeting", "overflow", "vi", "overflow".into())
            .is_err());
        assert!(jobs
            .reserve(&session, "meeting", "0", "vi", "correction".into())
            .is_ok());
    }
    #[test]
    fn superseded_text_and_previous_meetings_cannot_commit() {
        let jobs = LiveTranslations::default();
        let session = jobs.begin("meeting-1".into());
        let old = jobs
            .reserve(&session, "meeting-1", "segment", "vi", "1".into())
            .unwrap();
        let new = jobs
            .reserve(&session, "meeting-1", "segment", "vi", "2".into())
            .unwrap();
        assert!(old.cancel.is_cancelled());
        assert!(!jobs.finish(&old, || panic!("old revision committed")));
        assert!(jobs.finish(&new, || {}));
        let queued = jobs
            .reserve(&session, "meeting-1", "other", "ja", "3".into())
            .unwrap();
        let next = jobs.begin("meeting-2".into());
        assert!(queued.cancel.is_cancelled());
        jobs.end(&session);
        assert!(jobs
            .reserve(&next, "meeting-2", "next", "vi", "4".into())
            .is_ok());
        assert!(!jobs.finish(&queued, || panic!("old meeting committed")));
    }
}
