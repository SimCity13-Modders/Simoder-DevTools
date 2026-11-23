#![forbid(unsafe_code)]

//! Luau VM execution harness for `Simoder-Base` scripts (`tests/runner.luau`, `tests/services.luau`, `scripts/init.luau`).

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use mlua::{Lua, MultiValue, Result as LuaResult, Table};

/// Execute a `Simoder-Base` Luau module (such as `tests/runner.luau` or `tests/services.luau`)
/// inside a sandboxed Luau VM and invoke its `onEnable` callback.
pub fn run_base_luau_test_module(base_root: &Path, relative_entry: &str) -> LuaResult<()> {
    let lua = Lua::new();
    let base_root_norm = normalize_path(base_root);
    let scripts_dir = normalize_path(&base_root_norm.join("scripts"));
    let entry_path = normalize_path(&base_root_norm.join(relative_entry));
    let entry_dir = entry_path
        .parent()
        .unwrap_or(&base_root_norm)
        .to_path_buf();

    let dir_stack = Arc::new(Mutex::new(vec![entry_dir]));
    let stack_for_req = Arc::clone(&dir_stack);
    let root_for_req = base_root_norm.clone();
    let scripts_for_req = scripts_dir.clone();

    let require_fn = lua.create_function(move |lua_ctx, spec: String| {
        let current_dir = stack_for_req
            .lock()
            .expect("dir_stack lock")
            .last()
            .cloned()
            .unwrap_or_else(|| scripts_for_req.clone());
        let candidate =
            resolve_module_path(&root_for_req, &scripts_for_req, &current_dir, &spec)
                .ok_or_else(|| {
                    mlua::Error::RuntimeError(format!("cannot resolve module: {spec}"))
                })?;
        let module_dir = candidate
            .parent()
            .unwrap_or(&scripts_for_req)
            .to_path_buf();
        let source = fs::read_to_string(&candidate).map_err(mlua::Error::external)?;
        stack_for_req.lock().expect("dir_stack lock").push(module_dir);
        let eval_res = lua_ctx.load(&source).set_name(&spec).eval::<MultiValue>();
        stack_for_req.lock().expect("dir_stack lock").pop();
        eval_res
    })?;

    lua.globals().set("require", require_fn)?;
    lua.sandbox(true)?;

    let source = fs::read_to_string(&entry_path).map_err(mlua::Error::external)?;
    let module: Table = lua.load(&source).set_name(relative_entry).eval()?;
    let on_enable: mlua::Function = module.get("onEnable")?;
    on_enable.call::<()>(())?;
    Ok(())
}

fn resolve_module_path(
    base_root: &Path,
    scripts_dir: &Path,
    current_dir: &Path,
    spec: &str,
) -> Option<PathBuf> {
    let rel = format!("{spec}.luau");
    for anchor in [current_dir, scripts_dir, base_root] {
        let joined = normalize_path(&anchor.join(&rel));
        if joined.starts_with(base_root) && joined.is_file() {
            return Some(joined);
        }
    }
    None
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
