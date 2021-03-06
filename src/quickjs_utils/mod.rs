use crate::quickjsruntime::QuickJsRuntime;

pub mod arrays;
pub mod atoms;
pub mod bigints;
pub mod compile;
pub mod dates;
pub mod errors;
pub mod functions;
pub mod iterators;
pub mod json;
pub mod maps;
pub mod modules;
pub mod objects;
pub mod primitives;
pub mod promises;
pub mod sets;
pub mod typedarrays;

use crate::eserror::EsError;
use crate::quickjs_utils::objects::get_property;
use crate::quickjscontext::QuickJsContext;
use crate::valueref::{JSValueRef, TAG_NULL, TAG_UNDEFINED};
use libquickjs_sys as q;

// todo
// runtime and context in thread_local here
// all function (where applicable) get an Option<QuickJSRuntime> which if None will be gotten from the thread_local
// every function which returns a q::JSValue will return a OwnedValueRef to ensure values are freed on drop

pub fn gc(q_js_rt: &QuickJsRuntime) {
    log::trace!("GC called");
    unsafe { q::JS_RunGC(q_js_rt.runtime) }
    log::trace!("GC done");
}

pub fn new_undefined_ref() -> JSValueRef {
    JSValueRef::new_no_context(
        q::JSValue {
            u: q::JSValueUnion { int32: 0 },
            tag: TAG_UNDEFINED,
        },
        "new_undefined_ref",
    )
}

pub fn new_null() -> q::JSValue {
    q::JSValue {
        u: q::JSValueUnion { int32: 0 },
        tag: TAG_NULL,
    }
}

pub fn new_null_ref() -> JSValueRef {
    JSValueRef::new_no_context(new_null(), "null_ref")
}

pub fn get_global_q(context: &QuickJsContext) -> JSValueRef {
    unsafe { get_global(context.context) }
}
/// # Safety
/// When passing a context pointer please make sure the corresponding QuickJsContext is still valid
pub unsafe fn get_global(context: *mut q::JSContext) -> JSValueRef {
    let global = q::JS_GetGlobalObject(context);
    JSValueRef::new(context, global, false, true, "global")
}
/// # Safety
/// When passing a context pointer please make sure the corresponding QuickJsContext is still valid
pub unsafe fn get_constructor(
    context: *mut q::JSContext,
    constructor_name: &str,
) -> Result<JSValueRef, EsError> {
    let global_ref = get_global(context);

    let constructor_ref = get_property(context, &global_ref, constructor_name)?;

    if constructor_ref.is_null_or_undefined() {
        Err(EsError::new_string(format!(
            "not found: {}",
            constructor_name
        )))
    } else {
        Ok(constructor_ref)
    }
}

/// # Safety
/// When passing a context pointer please make sure the corresponding QuickJsContext is still valid
pub unsafe fn parse_args(
    context: *mut q::JSContext,
    argc: ::std::os::raw::c_int,
    argv: *mut q::JSValue,
) -> Vec<JSValueRef> {
    let arg_slice = std::slice::from_raw_parts(argv, argc as usize);
    arg_slice
        .iter()
        .map(|raw| JSValueRef::new(context, *raw, true, true, "quickjs_utils::parse_args"))
        .collect::<Vec<_>>()
}

#[cfg(test)]
pub mod tests {
    use crate::esruntime::tests::init_test_rt;
    use crate::quickjs_utils::get_global_q;

    #[test]
    fn test_global() {
        let rt = init_test_rt();
        let _io = rt.add_to_event_queue_sync(|q_js_rt| {
            let q_ctx = q_js_rt.get_main_context();

            let ct = get_global_q(q_ctx).get_ref_count();
            for _ in 0..5 {
                let global = get_global_q(q_ctx);
                assert_eq!(global.get_ref_count(), ct);
            }
        });
    }
}
