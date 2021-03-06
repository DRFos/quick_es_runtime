//! utils for working with ES6 Modules

use crate::eserror::EsError;
use crate::esscript::EsScript;
use crate::quickjs_utils::atoms;
use crate::quickjs_utils::atoms::JSAtomRef;
use crate::quickjscontext::QuickJsContext;
use crate::quickjsruntime::QuickJsRuntime;
use crate::valueref::JSValueRef;
use core::ptr;
use libquickjs_sys as q;
use std::ffi::{CStr, CString};

/// compile a module, used for module loading
/// # Safety
/// please ensure the corresponding QuickJSContext is still valid
pub unsafe fn compile_module(
    context: *mut q::JSContext,
    script: EsScript,
) -> Result<JSValueRef, EsError> {
    let code = script.get_code();
    let code_c = CString::new(code).ok().unwrap();
    let filename_c = CString::new(script.get_path()).ok().unwrap();

    let value_raw = q::JS_Eval(
        context,
        code_c.as_ptr(),
        code.len() as _,
        filename_c.as_ptr(),
        (q::JS_EVAL_TYPE_MODULE | q::JS_EVAL_FLAG_COMPILE_ONLY) as i32,
    );

    // check for error
    let ret = JSValueRef::new(
        context,
        value_raw,
        false,
        true,
        format!("compile_module result of {}", script.get_path()).as_str(),
    );

    log::trace!("compile module yielded a {}", ret.borrow_value().tag);

    if ret.is_exception() {
        let ex_opt = QuickJsContext::get_exception(context);
        if let Some(ex) = ex_opt {
            Err(ex)
        } else {
            Err(EsError::new_str(
                "compile_module failed and could not get exception",
            ))
        }
    } else {
        Ok(ret)
    }
}

// get the ModuleDef obj from a JSValue, this is used for module loading
pub fn get_module_def(value: &JSValueRef) -> *mut q::JSModuleDef {
    assert!(value.is_module());
    unsafe { value.borrow_value().u.ptr as *mut q::JSModuleDef }
}

#[allow(dead_code)]
pub fn set_module_loader(q_js_rt: &QuickJsRuntime) {
    log::trace!("setting up module loader");

    let module_normalize: q::JSModuleNormalizeFunc = Some(js_module_normalize);
    let module_loader: q::JSModuleLoaderFunc = Some(js_module_loader);

    let opaque = std::ptr::null_mut();

    unsafe { q::JS_SetModuleLoaderFunc(q_js_rt.runtime, module_normalize, module_loader, opaque) }
}

/// detect if a script is module (contains import or export statements)
pub fn detect_module(source: &str) -> bool {
    let cstr = CString::new(source).expect("could not create CString due to null term in source");
    unsafe { q::JS_DetectModule(cstr.as_ptr(), source.len() as _) != 0 }
}

/// create new Module (JSModuleDef struct) which can be populated with exports after (and from) the init_func
/// # Safety
/// Please ensure the context passed is still valid
pub unsafe fn new_module(
    ctx: *mut q::JSContext,
    name: &str,
    init_func: q::JSModuleInitFunc,
) -> Result<*mut q::JSModuleDef, EsError> {
    let name_cstr = CString::new(name).map_err(|_e| EsError::new_str("CString failed"))?;
    Ok(q::JS_NewCModule(ctx, name_cstr.as_ptr(), init_func))
}

/// set an export in a JSModuleDef, this should be called AFTER the init_func(as passed to new_module()) is called
/// please note that you always need to use this in combination with add_module_export()
/// # Safety
/// Please ensure the context passed is still valid
pub unsafe fn set_module_export(
    ctx: *mut q::JSContext,
    module: *mut q::JSModuleDef,
    export_name: &str,
    js_val: JSValueRef,
) -> Result<(), EsError> {
    let name_cstr = CString::new(export_name).map_err(|_e| EsError::new_str("CString failed"))?;
    let res = q::JS_SetModuleExport(
        ctx,
        module,
        name_cstr.as_ptr(),
        js_val.clone_value_incr_rc(),
    );
    if res == 0 {
        Ok(())
    } else {
        Err(EsError::new_str("JS_SetModuleExport failed"))
    }
}

/// set an export in a JSModuleDef, this should be called BEFORE this init_func(as passed to new_module()) is called
/// # Safety
/// Please ensure the context passed is still valid
pub unsafe fn add_module_export(
    ctx: *mut q::JSContext,
    module: *mut q::JSModuleDef,
    export_name: &str,
) -> Result<(), EsError> {
    let name_cstr = CString::new(export_name).map_err(|_e| EsError::new_str("CString failed"))?;
    let res = q::JS_AddModuleExport(ctx, module, name_cstr.as_ptr());
    if res == 0 {
        Ok(())
    } else {
        Err(EsError::new_str("JS_SetModuleExport failed"))
    }
}

/// get the name of an JSModuleDef struct
/// # Safety
/// Please ensure the context passed is still valid
pub unsafe fn get_module_name(
    ctx: *mut q::JSContext,
    module: *mut q::JSModuleDef,
) -> Result<String, EsError> {
    let atom_raw = q::JS_GetModuleName(ctx, module);
    let atom_ref = JSAtomRef::new(ctx, atom_raw);
    atoms::to_string(ctx, &atom_ref)
}

unsafe extern "C" fn js_module_normalize(
    ctx: *mut q::JSContext,
    module_base_name: *const ::std::os::raw::c_char,
    module_name: *const ::std::os::raw::c_char,
    _opaque: *mut ::std::os::raw::c_void,
) -> *mut ::std::os::raw::c_char {
    log::trace!("js_module_normalize called.");

    let base_c = CStr::from_ptr(module_base_name);
    let base_str = base_c
        .to_str()
        .expect("could not convert module_base_name to str");
    let name_c = CStr::from_ptr(module_name);
    let name_str = name_c
        .to_str()
        .expect("could not convert module_name to str");

    log::trace!(
        "js_module_normalize called. base: {}. name: {}",
        base_str,
        name_str
    );

    QuickJsRuntime::do_with(|q_js_rt| {
        let q_ctx = q_js_rt.get_quickjs_context(ctx);
        for loader in &q_js_rt.module_loaders {
            if let Some(normalized_path) = loader.normalize_path(q_ctx, base_str, name_str) {
                let c_absolute_path = CString::new(normalized_path.as_str()).expect("fail");

                return c_absolute_path.into_raw();
            }
        }
        q_ctx.report_ex(format!("Module {} was not found", name_str).as_str());
        ptr::null_mut()
    })
}

unsafe extern "C" fn js_module_loader(
    ctx: *mut q::JSContext,
    module_name_raw: *const ::std::os::raw::c_char,
    _opaque: *mut ::std::os::raw::c_void,
) -> *mut q::JSModuleDef {
    log::trace!("js_module_loader called.");

    let module_name_c = CStr::from_ptr(module_name_raw);
    let module_name = module_name_c.to_str().expect("could not get module name");

    log::trace!("js_module_loader called: {}", module_name);

    QuickJsRuntime::do_with(|q_js_rt| {
        QuickJsContext::with_context(ctx, |q_ctx| {
            for module_loader in &q_js_rt.module_loaders {
                if module_loader.has_module(q_ctx, module_name) {
                    let mod_val_res = module_loader.load_module(q_ctx, module_name);
                    match mod_val_res {
                        Ok(mod_val) => {
                            return mod_val;
                        }
                        Err(e) => {
                            let err =
                                format!("Module load failed for {} because of: {}", module_name, e);
                            log::error!("{}", err);
                            q_ctx.report_ex(err.as_str());
                            return std::ptr::null_mut();
                        }
                    };
                }
            }
            std::ptr::null_mut()
        })
    })
}

#[cfg(test)]
pub mod tests {
    use crate::esruntime::tests::init_test_rt;
    use crate::esscript::EsScript;
    use crate::quickjs_utils::modules::detect_module;
    use std::time::Duration;

    #[test]
    fn test_native_modules() {
        let rt = init_test_rt();
        let mres = rt.eval_module_sync(EsScript::new(
            "test.mes",
            "import {a, b, c} from 'greco://testmodule1';\nconsole.log('testmodule1.a = %s, testmodule1.b = %s, testmodule1.c = %s', a, b, c);",
        ));
        match mres {
            Ok(_module_res) => {}
            Err(e) => panic!("test_native_modules failed: {}", e),
        }

        let res_prom = rt.eval_sync(EsScript::new("test_mod_nat_async.es", "(import('greco://someMod').then((module) => {return {a: module.a, b: module.b, c: module.c};}));")).ok().unwrap();
        let res = res_prom.get_promise_result_sync();
        let obj = res.ok().expect("prom failed");
        assert!(obj.is_object());
        let a = obj.get_object().get("a").expect("obj did not have a");
        assert_eq!(a.get_i32(), 1234);
        let b = obj.get_object().get("b").expect("obj did not have b");
        assert_eq!(b.get_i32(), 64834);
    }

    #[test]
    fn test_detect() {
        assert!(detect_module("import {} from 'foo.es';"));
        assert!(detect_module("export function a(){};"));
        assert!(!detect_module("import('foo.es').then((a) = {});"));
        assert!(!detect_module("let a = 1;"));
    }

    #[test]
    fn test_module_sandbox() {
        log::info!("> test_module_sandbox");

        let rt = init_test_rt();
        rt.add_to_event_queue_sync(|q_js_rt| {
            let q_ctx = q_js_rt.get_main_context();
            let res = q_ctx.eval_module(EsScript::new(
                "test1.mes",
                "export const name = 'foobar';\nconsole.log('evalling module');",
            ));

            if res.is_err() {
                panic!("parse module failed: {}", res.err().unwrap())
            }
            res.ok().expect("parse module failed");
        });

        rt.add_to_event_queue_sync(|q_js_rt| {
            let q_ctx = q_js_rt.get_main_context();
            let res = q_ctx.eval_module(EsScript::new(
                "test2.mes",
                "import {name} from 'test1.mes';\n\nconsole.log('imported name: ' + name);",
            ));

            if res.is_err() {
                panic!("parse module2 failed: {}", res.err().unwrap())
            }

            res.ok().expect("parse module2 failed");
        });

        rt.add_to_event_queue_sync(|q_js_rt| {
            let q_ctx = q_js_rt.get_main_context();
            let res = q_ctx.eval_module(EsScript::new(
                "test3.mes",
                "import {name} from 'notfound.mes';\n\nconsole.log('imported name: ' + name);",
            ));

            assert!(res.is_err());
            assert!(res
                .err()
                .unwrap()
                .get_message()
                .contains("Module notfound.mes was not found"));
        });

        rt.add_to_event_queue_sync(|q_js_rt| {
            let q_ctx = q_js_rt.get_main_context();
            let res = q_ctx.eval_module(EsScript::new(
                "test4.mes",
                "import {name} from 'invalid.mes';\n\nconsole.log('imported name: ' + name);",
            ));

            assert!(res.is_err());
            assert!(res
                .err()
                .unwrap()
                .get_message()
                .contains("Module load failed for invalid.mes"));
        });

        rt.add_to_event_queue_sync(|q_js_rt| {
            let q_ctx = q_js_rt.get_main_context();
            let res = q_ctx.eval_module(EsScript::new(
                "test2.mes",
                "import {name} from 'test1.mes';\n\nconsole.log('imported name: ' + name);",
            ));

            if res.is_err() {
                panic!("parse module2 failed: {}", res.err().unwrap())
            }

            res.ok().expect("parse module2 failed");
        });

        std::thread::sleep(Duration::from_secs(1));

        log::info!("< test_module_sandbox");
    }
}
