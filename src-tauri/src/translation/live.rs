//! Bounds live work, cancels superseded revisions, and guards DB commits.
use crate::llm::provider::CancelToken;
use std::collections::{HashMap, VecDeque};
use super::TranslationContextTurn;
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
    history: VecDeque<(String, String, String, TranslationContextTurn)>,
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
            history: VecDeque::new(),
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
    pub fn context(&self, ticket: &Ticket, source: &str, target: &str) -> Vec<TranslationContextTurn> {
        let session = self.session.lock().unwrap();
        if session.id != ticket.session || ticket.cancel.is_cancelled() { return Vec::new(); }
        session.history.iter().filter(|(key, from, to, _)| key != &ticket.key && from == source && to == target)
            .rev().take(2).map(|(_, _, _, turn)| turn.clone()).collect::<Vec<_>>()
            .into_iter().rev().collect()
    }

    pub fn remember(&self, ticket: &Ticket, source: &str, target: &str, text: &str, translated: &str) {
        let mut session = self.session.lock().unwrap();
        if session.id != ticket.session || ticket.cancel.is_cancelled() { return; }
        session.history.retain(|(key, _, _, _)| key != &ticket.key);
        session.history.push_back((ticket.key.clone(), source.into(), target.into(), TranslationContextTurn {
            source_text: text.chars().take(240).collect(),
            translated_text: translated.chars().take(240).collect(),
        }));
        while session.history.len() > 12 { session.history.pop_front(); }
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
    fn context_is_bounded_to_the_current_language_pair_and_meeting() {
        let jobs = LiveTranslations::default();
        let session = jobs.begin("meeting".into());
        for n in 0..4 {
            let ticket = jobs.reserve(&session, "meeting", &n.to_string(), "ja", n.to_string()).unwrap();
            assert!(jobs.finish(&ticket, || {}));
            jobs.remember(&ticket, "vi", "ja", &format!("Câu {n}"), "翻訳");
        }
        let current = jobs.reserve(&session, "meeting", "current", "ja", "current".into()).unwrap();
        let context = jobs.context(&current, "vi", "ja");
        assert_eq!(context.len(), 2);
        assert_eq!(context[0].source_text, "Câu 2");
        assert!(jobs.context(&current, "ja", "vi").is_empty());
        let next = jobs.begin("next".into());
        jobs.remember(&current, "vi", "ja", "stale text", "古い");
        let ticket = jobs.reserve(&next, "next", "next", "ja", "next".into()).unwrap();
        assert!(jobs.context(&ticket, "vi", "ja").is_empty());
    }

    #[test]
    fn corrections_do_not_use_the_old_version_of_the_same_segment_as_context() {
        let jobs = LiveTranslations::default();
        let session = jobs.begin("meeting".into());
        let old = jobs.reserve(&session, "meeting", "same", "ja", "old".into()).unwrap();
        jobs.finish(&old, || {});
        jobs.remember(&old, "vi", "ja", "Chắc chắn thứ sáu", "金曜日です");
        let new = jobs.reserve(&session, "meeting", "same", "ja", "new".into()).unwrap();
        assert!(jobs.context(&new, "vi", "ja").is_empty());
    }

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
