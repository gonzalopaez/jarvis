use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    pub audit_id: String,
    pub request_id: String,
    pub subject: String,
    pub capability: Option<String>,
    pub target: Option<String>,
    pub outcome: &'static str,
}

pub trait AuditSink {
    fn record(&self, event: AuditEvent);
}

#[derive(Debug, Default)]
pub struct MemoryAuditSink {
    events: Mutex<Vec<AuditEvent>>,
}

impl MemoryAuditSink {
    pub fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().expect("audit lock poisoned").clone()
    }
}

impl AuditSink for MemoryAuditSink {
    fn record(&self, event: AuditEvent) {
        self.events.lock().expect("audit lock poisoned").push(event);
    }
}

impl<T> AuditSink for Arc<T>
where
    T: AuditSink + ?Sized,
{
    fn record(&self, event: AuditEvent) {
        self.as_ref().record(event);
    }
}
