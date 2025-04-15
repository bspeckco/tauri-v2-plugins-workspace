// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const COMMANDS: &[&str] = &[
    "load",
    "execute",
    "select",
    "close",
    "begin_transaction",
    "commit_transaction",
    "rollback_transaction",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        // .global_api_script_path("./api-iife.js") // REMOVED - Build fails without it, and it isn't auto-generated correctly
        .build();
}
