use crate::eserror::EsError;
use crate::esruntime::EsRuntime;
use std::sync::Arc;

pub mod console;
pub mod fetch;
pub mod set_timeout;
pub mod setimmediate;

pub fn init(es_rt: Arc<EsRuntime>) -> Result<(), EsError> {
    log::trace!("features::init");

    let es_rt2 = es_rt.clone();
    es_rt.add_to_event_queue_sync(move |q_js_rt| {
        console::init(q_js_rt)?;
        fetch::init(es_rt2)?;
        setimmediate::init(q_js_rt)?;
        set_timeout::init(q_js_rt)?;
        Ok(())
    })
}
