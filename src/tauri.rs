//! Thin typed wrapper around Tauri's IPC `invoke`.

use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn invoke_raw(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

fn js_err(e: JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

/// Call a Tauri command and deserialize its result.
pub async fn invoke<A: Serialize, R: DeserializeOwned>(cmd: &str, args: &A) -> Result<R, String> {
    let args = serde_wasm_bindgen::to_value(args).map_err(|e| e.to_string())?;
    let value = invoke_raw(cmd, args).await.map_err(js_err)?;
    serde_wasm_bindgen::from_value(value).map_err(|e| e.to_string())
}

/// Call a Tauri command, ignoring its (unit) result.
pub async fn invoke_unit<A: Serialize>(cmd: &str, args: &A) -> Result<(), String> {
    let args = serde_wasm_bindgen::to_value(args).map_err(|e| e.to_string())?;
    invoke_raw(cmd, args).await.map(|_| ()).map_err(js_err)
}

/// Empty argument object for commands without parameters.
#[derive(Serialize)]
pub struct NoArgs {}
