use std::sync::Arc;

use baml_types::{ir_type::TypeNonStreaming, tracing::events::TraceEvent};

pub type TraceEventWithMeta = TraceEvent<'static, TypeNonStreaming>;

pub trait Storage {
    fn put(&self, event: Arc<TraceEventWithMeta>);

    fn clear(&self);

    fn get_all(&self) -> Vec<Arc<TraceEventWithMeta>>;
}
