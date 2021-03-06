//! Set utils, these methods can be used to manage Set objects from rust
//! see [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Sap) for more on Sets

use crate::eserror::EsError;
use crate::quickjs_utils::objects::{construct_object, is_instance_of_by_name};
use crate::quickjs_utils::{functions, get_constructor, iterators, objects, primitives};
use crate::quickjscontext::QuickJsContext;
use crate::valueref::JSValueRef;
use libquickjs_sys as q;

/// create new instance of Set
/// # Example
/// ```rust
/// use quickjs_runtime::esruntimebuilder::EsRuntimeBuilder;
/// use quickjs_runtime::valueref::JSValueRef;
/// use quickjs_runtime::quickjs_utils::sets::new_set_q;
///
/// let rt = EsRuntimeBuilder::new().build();
/// rt.add_to_event_queue_sync(|q_js_rt| {
///    let q_ctx = q_js_rt.get_main_context();
///    let my_set: JSValueRef = new_set_q(q_ctx).ok().unwrap();
/// });
/// ```
pub fn new_set_q(q_ctx: &QuickJsContext) -> Result<JSValueRef, EsError> {
    unsafe { new_set(q_ctx.context) }
}

/// create new instance of Set
/// # Safety
/// please ensure the passed JSContext is still valid
pub unsafe fn new_set(ctx: *mut q::JSContext) -> Result<JSValueRef, EsError> {
    let map_constructor = get_constructor(ctx, "Set")?;
    construct_object(ctx, &map_constructor, vec![])
}

/// see if a JSValueRef is an instance of Set
pub fn is_set_q(q_ctx: &QuickJsContext, obj: &JSValueRef) -> Result<bool, EsError> {
    unsafe { is_set(q_ctx.context, obj) }
}

/// see if a JSValueRef is an instance of Set
/// # Safety
/// please ensure the passed JSContext is still valid
pub unsafe fn is_set(ctx: *mut q::JSContext, obj: &JSValueRef) -> Result<bool, EsError> {
    is_instance_of_by_name(ctx, obj, "Set")
}

/// add a value to the Set
/// # Example
/// ```rust
/// use quickjs_runtime::esruntimebuilder::EsRuntimeBuilder;
/// use quickjs_runtime::valueref::JSValueRef;
/// use quickjs_runtime::quickjs_utils::primitives;
/// use quickjs_runtime::quickjs_utils::sets::{new_set_q, add_q};
///
/// let rt = EsRuntimeBuilder::new().build();
/// rt.add_to_event_queue_sync(|q_js_rt| {
///    let q_ctx = q_js_rt.get_main_context();
///    let my_set: JSValueRef = new_set_q(q_ctx).ok().unwrap();
///    let value = primitives::from_i32(23);
///    add_q(q_ctx, &my_set, value).ok().unwrap();
/// });
/// ```
pub fn add_q(
    q_ctx: &QuickJsContext,
    set: &JSValueRef,
    val: JSValueRef,
) -> Result<JSValueRef, EsError> {
    unsafe { add(q_ctx.context, set, val) }
}

/// add a value to a Set
/// # Safety
/// please ensure the passed JSContext is still valid
pub unsafe fn add(
    ctx: *mut q::JSContext,
    set: &JSValueRef,
    val: JSValueRef,
) -> Result<JSValueRef, EsError> {
    functions::invoke_member_function(ctx, set, "add", vec![val])
}

/// delete a value from a set
/// # Example
/// ```rust
/// use quickjs_runtime::esruntimebuilder::EsRuntimeBuilder;
/// use quickjs_runtime::valueref::JSValueRef;
/// use quickjs_runtime::quickjs_utils::primitives;
/// use quickjs_runtime::quickjs_utils::sets::{add_q, new_set_q, delete_q};
///
/// let rt = EsRuntimeBuilder::new().build();
/// rt.add_to_event_queue_sync(|q_js_rt| {
///    let q_ctx = q_js_rt.get_main_context();
///    let my_set: JSValueRef = new_set_q(q_ctx).ok().unwrap();
///    let value = primitives::from_i32(23);
///    add_q(q_ctx, &my_set, value.clone()).ok().unwrap();
///    delete_q(q_ctx, &my_set, value).ok().unwrap();
/// });
/// ```
pub fn delete_q(
    q_ctx: &QuickJsContext,
    set: &JSValueRef,
    value: JSValueRef,
) -> Result<bool, EsError> {
    unsafe { delete(q_ctx.context, set, value) }
}

/// delete a value from a set
/// # Safety
/// please ensure the passed JSContext is still valid
pub unsafe fn delete(
    ctx: *mut q::JSContext,
    set: &JSValueRef,
    value: JSValueRef,
) -> Result<bool, EsError> {
    let res = functions::invoke_member_function(ctx, set, "delete", vec![value])?;
    primitives::to_bool(&res)
}

/// check whether a Set has a certain value
/// # Example
/// ```rust
/// use quickjs_runtime::esruntimebuilder::EsRuntimeBuilder;
/// use quickjs_runtime::valueref::JSValueRef;
/// use quickjs_runtime::quickjs_utils::primitives;
/// use quickjs_runtime::quickjs_utils::sets::{new_set_q, add_q, has_q};
///
/// let rt = EsRuntimeBuilder::new().build();
/// rt.add_to_event_queue_sync(|q_js_rt| {
///    let q_ctx = q_js_rt.get_main_context();
///    let my_set: JSValueRef = new_set_q(q_ctx).ok().unwrap();
///    let value = primitives::from_i32(23);
///    add_q(q_ctx, &my_set, value.clone()).ok().unwrap();
///    let bln_has = has_q(q_ctx, &my_set, value).ok().unwrap();
///    assert!(bln_has);
/// });
/// ```
pub fn has_q(q_ctx: &QuickJsContext, set: &JSValueRef, key: JSValueRef) -> Result<bool, EsError> {
    unsafe { has(q_ctx.context, set, key) }
}

/// check whether a Set has a value
/// # Safety
/// please ensure the passed JSContext is still valid
pub unsafe fn has(
    ctx: *mut q::JSContext,
    set: &JSValueRef,
    key: JSValueRef,
) -> Result<bool, EsError> {
    let res = functions::invoke_member_function(ctx, set, "has", vec![key])?;
    primitives::to_bool(&res)
}

/// get the number of entries in a Set
/// # Example
/// ```rust
/// use quickjs_runtime::esruntimebuilder::EsRuntimeBuilder;
/// use quickjs_runtime::valueref::JSValueRef;
/// use quickjs_runtime::quickjs_utils::primitives;
/// use quickjs_runtime::quickjs_utils::sets::{add_q, new_set_q, size_q};
///
/// let rt = EsRuntimeBuilder::new().build();
/// rt.add_to_event_queue_sync(|q_js_rt| {
///    let q_ctx = q_js_rt.get_main_context();
///    let my_set: JSValueRef = new_set_q(q_ctx).ok().unwrap();
///    let value = primitives::from_i32(23);
///    add_q(q_ctx, &my_set, value).ok().unwrap();
///    let i_size = size_q(q_ctx, &my_set).ok().unwrap();
///    assert_eq!(i_size, 1);
/// });
/// ```
pub fn size_q(q_ctx: &QuickJsContext, set: &JSValueRef) -> Result<i32, EsError> {
    unsafe { size(q_ctx.context, set) }
}

/// get the number of entries in a Set
/// # Safety
/// please ensure the passed JSContext is still valid
pub unsafe fn size(ctx: *mut q::JSContext, set: &JSValueRef) -> Result<i32, EsError> {
    let res = objects::get_property(ctx, &set, "size")?;
    primitives::to_i32(&res)
}

/// remove all entries from a Set
/// # Example
/// ```rust
/// use quickjs_runtime::esruntimebuilder::EsRuntimeBuilder;
/// use quickjs_runtime::valueref::JSValueRef;
/// use quickjs_runtime::quickjs_utils::primitives;
/// use quickjs_runtime::quickjs_utils::sets::{size_q, clear_q, add_q, new_set_q};
///
/// let rt = EsRuntimeBuilder::new().build();
/// rt.add_to_event_queue_sync(|q_js_rt| {
///    let q_ctx = q_js_rt.get_main_context();
///    let my_set: JSValueRef = new_set_q(q_ctx).ok().unwrap();
///    let value = primitives::from_i32(23);
///    add_q(q_ctx, &my_set, value).ok().unwrap();
///    clear_q(q_ctx, &my_set).ok().unwrap();
///    let i_size = size_q(q_ctx, &my_set).ok().unwrap();
///    assert_eq!(i_size, 0);
/// });
/// ```
pub fn clear_q(q_ctx: &QuickJsContext, map: &JSValueRef) -> Result<(), EsError> {
    unsafe { clear(q_ctx.context, map) }
}

/// remove all entries from a Set
/// # Safety
/// please ensure the passed JSContext is still valid
pub unsafe fn clear(ctx: *mut q::JSContext, set: &JSValueRef) -> Result<(), EsError> {
    let _ = functions::invoke_member_function(ctx, &set, "clear", vec![])?;
    Ok(())
}

/// iterate over all values of a Set
/// # Example
/// ```rust
/// use quickjs_runtime::esruntimebuilder::EsRuntimeBuilder;
/// use quickjs_runtime::valueref::JSValueRef;
/// use quickjs_runtime::quickjs_utils::primitives;
/// use quickjs_runtime::quickjs_utils::sets::{new_set_q, add_q, values_q};
///
/// let rt = EsRuntimeBuilder::new().build();
/// rt.add_to_event_queue_sync(|q_js_rt| {
///    let q_ctx = q_js_rt.get_main_context();
///    let my_set: JSValueRef = new_set_q(q_ctx).ok().unwrap();
///    let value = primitives::from_i32(23);
///    add_q(q_ctx, &my_set, value).ok().unwrap();
///    let mapped_values = values_q(q_ctx, &my_set, |value| {Ok(123)}).ok().unwrap();
///    assert_eq!(mapped_values.len(), 1);
/// });
/// ```
pub fn values_q<C: Fn(JSValueRef) -> Result<R, EsError>, R>(
    q_ctx: &QuickJsContext,
    set: &JSValueRef,
    consumer_producer: C,
) -> Result<Vec<R>, EsError> {
    unsafe { values(q_ctx.context, set, consumer_producer) }
}

/// iterate over all values of a Set
/// # Safety
/// please ensure the passed JSContext is still valid
pub unsafe fn values<C: Fn(JSValueRef) -> Result<R, EsError>, R>(
    ctx: *mut q::JSContext,
    set: &JSValueRef,
    consumer_producer: C,
) -> Result<Vec<R>, EsError> {
    let iter_ref = functions::invoke_member_function(ctx, &set, "values", vec![])?;

    iterators::iterate(ctx, &iter_ref, consumer_producer)
}
