// store in thread_local

use crate::eserror::EsError;
use crate::esruntime::EsRuntime;
use crate::esscript::EsScript;
use crate::quickjs_utils::modules::{
    add_module_export, compile_module, get_module_def, get_module_name, new_module,
    set_module_export,
};
use crate::quickjs_utils::{gc, modules, promises};
use crate::quickjscontext::QuickJsContext;
use crate::valueref::JSValueRef;
use libquickjs_sys as q;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_int;
use std::panic;
use std::sync::{Arc, Weak};

/// this is the internal abstract loader which is used to actually load the modules
pub trait ModuleLoader {
    /// the normalize methods is used to translate a possible relative path to an absolute path of a module
    /// it doubles as a method to see IF a module can actually be loaded by a module loader (return None if the module can not be found)
    fn normalize_path(&self, q_ctx: &QuickJsContext, ref_path: &str, path: &str) -> Option<String>;
    /// load the Module
    fn load_module(
        &self,
        q_ctx: &QuickJsContext,
        absolute_path: &str,
    ) -> Result<*mut q::JSModuleDef, EsError>;
    /// has module is used to check if a loader can provide a certain module, this is currently used to check which loader should init a native module
    fn has_module(&self, q_ctx: &QuickJsContext, absolute_path: &str) -> bool;
    /// init a module, currently used to init native modules
    /// # Safety
    /// be safe with the moduledef ptr
    unsafe fn init_module(
        &self,
        q_ctx: &QuickJsContext,
        module: *mut q::JSModuleDef,
    ) -> Result<(), EsError>;
}

// these are the external (util) loaders (todo move these to esruntime?)

pub trait ScriptModuleLoader {
    fn normalize_path(&self, ref_path: &str, path: &str) -> Option<String>;
    fn load_module(&self, absolute_path: &str) -> String;
}

pub struct ScriptModuleLoaderAdapter {
    inner: Box<dyn ScriptModuleLoader>,
}

impl ScriptModuleLoaderAdapter {
    pub fn new(loader: Box<dyn ScriptModuleLoader>) -> Self {
        Self { inner: loader }
    }
}

impl ModuleLoader for ScriptModuleLoaderAdapter {
    fn normalize_path(
        &self,
        _q_ctx: &QuickJsContext,
        ref_path: &str,
        path: &str,
    ) -> Option<String> {
        self.inner.normalize_path(ref_path, path)
    }

    fn load_module(
        &self,
        q_ctx: &QuickJsContext,
        absolute_path: &str,
    ) -> Result<*mut q::JSModuleDef, EsError> {
        let code = self.inner.load_module(absolute_path);
        let compiled_module =
            unsafe { compile_module(q_ctx.context, EsScript::new(absolute_path, code.as_str()))? };
        Ok(get_module_def(&compiled_module))
    }

    fn has_module(&self, q_ctx: &QuickJsContext, absolute_path: &str) -> bool {
        self.normalize_path(q_ctx, absolute_path, absolute_path)
            .is_some()
    }

    unsafe fn init_module(
        &self,
        _q_ctx: &QuickJsContext,
        _module: *mut q::JSModuleDef,
    ) -> Result<(), EsError> {
        Ok(())
    }
}

pub struct NativeModuleLoaderAdapter {
    inner: Box<dyn NativeModuleLoader>,
}

impl NativeModuleLoaderAdapter {
    pub fn new(loader: Box<dyn NativeModuleLoader>) -> Self {
        Self { inner: loader }
    }
}

impl ModuleLoader for NativeModuleLoaderAdapter {
    fn normalize_path(
        &self,
        q_ctx: &QuickJsContext,
        _ref_path: &str,
        path: &str,
    ) -> Option<String> {
        if self.inner.has_module(q_ctx, path) {
            Some(path.to_string())
        } else {
            None
        }
    }

    fn load_module(
        &self,
        q_ctx: &QuickJsContext,
        absolute_path: &str,
    ) -> Result<*mut q::JSModuleDef, EsError> {
        // create module
        let module = unsafe { new_module(q_ctx.context, absolute_path, Some(native_module_init))? };

        for name in self.inner.get_module_export_names(q_ctx, absolute_path) {
            unsafe { add_module_export(q_ctx.context, module, name)? }
        }

        //std::ptr::null_mut()
        Ok(module)
    }

    fn has_module(&self, q_ctx: &QuickJsContext, absolute_path: &str) -> bool {
        self.inner.has_module(q_ctx, absolute_path)
    }

    unsafe fn init_module(
        &self,
        q_ctx: &QuickJsContext,
        module: *mut q::JSModuleDef,
    ) -> Result<(), EsError> {
        let module_name = get_module_name(q_ctx.context, module)?;

        for (name, val) in self.inner.get_module_exports(q_ctx, module_name.as_str()) {
            set_module_export(q_ctx.context, module, name, val)?;
        }
        Ok(())
    }
}

unsafe extern "C" fn native_module_init(
    ctx: *mut q::JSContext,
    module: *mut q::JSModuleDef,
) -> c_int {
    let module_name = get_module_name(ctx, module)
        .ok()
        .expect("could not get name");
    log::trace!("native_module_init: {}", module_name);

    QuickJsRuntime::do_with(|q_js_rt| {
        QuickJsContext::with_context(ctx, |q_ctx| {
            for module_loader in &q_js_rt.module_loaders {
                if module_loader.has_module(q_ctx, module_name.as_str()) {
                    return match module_loader.init_module(q_ctx, module) {
                        Ok(_) => {
                            0 // ok
                        }
                        Err(e) => {
                            q_ctx.report_ex(
                                format!(
                                    "Failed to init native module: {} caused by {}",
                                    module_name, e
                                )
                                .as_str(),
                            );
                            1
                        }
                    };
                }
            }
            0 // ok
        })
    })
}

pub trait NativeModuleLoader {
    fn has_module(&self, q_ctx: &QuickJsContext, module_name: &str) -> bool;
    fn get_module_export_names(&self, q_ctx: &QuickJsContext, module_name: &str) -> Vec<&str>;
    fn get_module_exports(
        &self,
        q_ctx: &QuickJsContext,
        module_name: &str,
    ) -> Vec<(&str, JSValueRef)>;
}

thread_local! {
   /// the thread-local QuickJsRuntime
   /// this only exists for the worker thread of the EsEventQueue
   /// todo move rt init to toplevel stackframe (out of lazy init)
   /// so the thread_local should be a refcel containing a null reF? or a None
   pub(crate) static QJS_RT: RefCell<Option<QuickJsRuntime>> = {
       RefCell::new(None)
   };

}

pub type ContextInitHooks =
    Vec<Box<dyn Fn(&QuickJsRuntime, &QuickJsContext) -> Result<(), EsError>>>;

pub struct QuickJsRuntime {
    pub(crate) runtime: *mut q::JSRuntime,
    contexts: HashMap<String, QuickJsContext>,
    es_rt_ref: Option<Weak<EsRuntime>>,
    id: String,
    context_init_hooks: RefCell<ContextInitHooks>,
    pub(crate) module_loaders: Vec<Box<dyn ModuleLoader>>,
}

impl QuickJsRuntime {
    pub(crate) fn init_rt_for_current_thread(rt: QuickJsRuntime) {
        QJS_RT.with(|rc| {
            let opt = &mut *rc.borrow_mut();
            opt.replace(rt);
        })
    }

    pub fn add_context_init_hook<H>(&self, hook: H) -> Result<(), EsError>
    where
        H: Fn(&QuickJsRuntime, &QuickJsContext) -> Result<(), EsError> + 'static,
    {
        // todo, for each context run hook

        for ctx in self.contexts.values() {
            hook(self, ctx)?;
        }

        let hooks = &mut *self.context_init_hooks.borrow_mut();
        hooks.push(Box::new(hook));
        Ok(())
    }
    // todo, this needs to be static, create a context, then borrowmut and add it (do not borrow mut while instantiating context)
    // so actually needs to be called in a plain job to inner.TaskManager and not by add_to_esEventquueue
    // EsRuntime should have a util to do that
    // EsRuntime should have extra methods like eval_sync_ctx(ctx: &str, script: &EsScript) etc
    pub fn create_context(id: &str) -> Result<(), EsError> {
        let ctx = Self::do_with(|q_js_rt| {
            assert!(!q_js_rt.has_context(id));
            QuickJsContext::new(id.to_string(), q_js_rt)
        });

        QuickJsRuntime::do_with_mut(|q_js_rt| {
            q_js_rt.contexts.insert(id.to_string(), ctx);
        });

        Self::do_with(|q_js_rt| {
            let ctx = q_js_rt.get_context(&id);
            let hooks = &*q_js_rt.context_init_hooks.borrow();
            for hook in hooks {
                hook(q_js_rt, &ctx)?;
            }
            Ok(())
        })
    }
    pub fn drop_context(id: &str) {
        QuickJsRuntime::do_with(|m_rt| {
            m_rt.gc();
        });

        QuickJsRuntime::do_with(|rt| {
            let q_ctx = rt.get_context(id);
            q_ctx.free();
            rt.gc();
        });

        let ctx =
            QuickJsRuntime::do_with_mut(|m_rt| m_rt.contexts.remove(id).expect("no such context"));

        drop(ctx);
        QuickJsRuntime::do_with(|m_rt| {
            m_rt.gc();
        });
    }
    pub(crate) fn get_context_ids() -> Vec<String> {
        QuickJsRuntime::do_with(|q_js_rt| q_js_rt.contexts.iter().map(|c| c.0.clone()).collect())
    }
    pub fn get_context(&self, id: &str) -> &QuickJsContext {
        self.contexts.get(id).expect("no such context")
    }
    pub fn opt_context(&self, id: &str) -> Option<&QuickJsContext> {
        self.contexts.get(id)
    }
    pub fn has_context(&self, id: &str) -> bool {
        self.contexts.contains_key(id)
    }
    pub(crate) fn init_rt_ref(&mut self, rt_ref: Arc<EsRuntime>) {
        self.es_rt_ref = Some(Arc::downgrade(&rt_ref));
    }
    /// # Safety
    /// When passing a context pointer please make sure the corresponding QuickJsContext is still valid
    pub unsafe fn get_quickjs_context(&self, context: *mut q::JSContext) -> &QuickJsContext {
        let id = QuickJsContext::get_id(context);
        self.get_context(id)
    }
    pub fn get_rt_ref(&self) -> Option<Arc<EsRuntime>> {
        if let Some(rt_ref) = &self.es_rt_ref {
            rt_ref.upgrade()
        } else {
            None
        }
    }

    pub(crate) fn new(runtime: *mut q::JSRuntime) -> Self {
        log::trace!("creating new QuickJsRuntime");

        if runtime.is_null() {
            panic!("RuntimeCreationFailed");
        }

        // Configure memory limit if specified.
        //let memory_limit = None;
        //if let Some(limit) = memory_limit {
        //  unsafe {
        //q::JS_SetMemoryLimit(runtime, limit as _);
        //}
        //}

        let id = format!("q_{}", thread_id::get());

        let mut q_rt = Self {
            runtime,
            contexts: Default::default(),

            es_rt_ref: None,
            id,
            context_init_hooks: RefCell::new(vec![]),
            module_loaders: vec![],
        };

        modules::set_module_loader(&q_rt);
        promises::init_promise_rejection_tracker(&q_rt);

        let main_ctx = QuickJsContext::new("__main__".to_string(), &q_rt);
        q_rt.contexts.insert("__main__".to_string(), main_ctx);

        q_rt
    }

    pub fn get_main_context(&self) -> &QuickJsContext {
        // todo store this somewhere so we don't need a lookup in the map every time
        self.get_context("__main__")
    }

    /// run the garbage collector
    pub fn gc(&self) {
        gc(self);
    }

    pub fn do_with<C, R>(task: C) -> R
    where
        C: FnOnce(&QuickJsRuntime) -> R,
    {
        QJS_RT.with(|qjs_rc| {
            let qjs_rt = &*qjs_rc.borrow();
            task(
                qjs_rt
                    .as_ref()
                    .expect("runtime was not yet initialized for this thread"),
            )
        })
    }

    pub fn do_with_mut<C, R>(task: C) -> R
    where
        C: FnOnce(&mut QuickJsRuntime) -> R,
    {
        QJS_RT.with(|qjs_rc| {
            let qjs_rt = &mut *qjs_rc.borrow_mut();
            task(
                qjs_rt
                    .as_mut()
                    .expect("runtime was not yet initialized for this thread"),
            )
        })
    }

    pub fn run_pending_jobs_if_any(&self) {
        log::trace!("quick_js_rt.run_pending_jobs_if_any");
        while self.has_pending_jobs() {
            log::trace!("quick_js_rt.has_pending_jobs!");
            let res = self.run_pending_job();
            match res {
                Ok(_) => {
                    log::trace!("run_pending_job OK!");
                }
                Err(e) => {
                    log::error!("run_pending_job failed: {}", e);
                }
            }
        }
    }

    pub fn has_pending_jobs(&self) -> bool {
        let flag = unsafe { q::JS_IsJobPending(self.runtime) };
        flag > 0
    }

    pub fn run_pending_job(&self) -> Result<(), EsError> {
        let mut ctx: *mut q::JSContext = std::ptr::null_mut();
        let flag = unsafe {
            // ctx is a return arg here
            q::JS_ExecutePendingJob(self.runtime, &mut ctx)
        };
        if flag < 0 {
            let e = unsafe { QuickJsContext::get_exception(ctx) }
                .unwrap_or_else(|| EsError::new_str("Unknown exception while running pending job"));
            return Err(e);
        }
        Ok(())
    }

    pub fn get_id(&self) -> &str {
        self.id.as_str()
    }
}

impl Drop for QuickJsRuntime {
    fn drop(&mut self) {
        // drop contexts first, should be done when Dropping EsRuntime?
        log::trace!("drop QuickJsRuntime, dropping contexts");
        self.contexts.clear();
        log::trace!("drop QuickJsRuntime, after dropping contexts");

        log::trace!("before JS_FreeRuntime");
        unsafe { q::JS_FreeRuntime(self.runtime) };
        log::trace!("after JS_FreeRuntime");
    }
}

/// Helper for creating CStrings.
pub(crate) fn make_cstring(value: &str) -> Result<CString, EsError> {
    let res = CString::new(value);
    match res {
        Ok(val) => Ok(val),
        Err(_) => Err(EsError::new_string(format!(
            "could not create cstring from {}",
            value
        ))),
    }
}
