//! The `mail.source.*` **command-palette descriptors**.
//!
//! Each descriptor is named EXACTLY after its verb, which is what makes the catalog's visibility gate
//! free: `tools.catalog` keeps a tool only if `authorize_tool(principal, ws, <name>)` passes, so an
//! admin sees these commands and a member simply does not — no new cap, no `if` in the catalog.
//!
//! `register`'s schema is form-shaped so the palette can render a working mailbox form rather than a
//! free-text JSON box. Fields the record defaults are declared but not required, so the shortest
//! useful registration is five values (id, host, username, secretPath, and a format if you want the
//! ingest half).

use lb_mcp::ToolDescriptor;
use serde_json::{json, Value};

/// Every descriptor this service contributes.
pub fn mail_descriptors() -> Vec<ToolDescriptor> {
    vec![
        register_descriptor(),
        list_descriptor(),
        check_descriptor(),
        poll_descriptor(),
    ]
}

fn register_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string",
                "x-lb": { "label": "Id", "description": "Workspace-unique id for this mailbox" } },
            "name": { "type": "string", "x-lb": { "label": "Name" } },
            "host": { "type": "string",
                "x-lb": { "label": "IMAP host", "description": "e.g. imap.gmail.com" } },
            "port": { "type": "number", "x-lb": { "label": "Port", "description": "993 for implicit TLS" } },
            "tls": { "type": "string",
                "x-lb": { "widget": "select", "options": ["implicit", "none"], "label": "TLS" } },
            "mailbox": { "type": "string", "x-lb": { "label": "Mailbox", "description": "INBOX" } },
            "username": { "type": "string", "x-lb": { "label": "Username" } },
            "auth": { "type": "string",
                "x-lb": { "widget": "select", "options": ["plain", "login", "xoauth2"], "label": "Auth" } },
            "secretPath": { "type": "string",
                "x-lb": { "label": "Secret path",
                          "description": "Where the password / refresh token is SEALED. The value never lives on this record." } },
            // An ARRAY, matching the record — `tools::validate_args` enforces the declared type
            // before dispatch, so declaring the palette's convenient comma-separated STRING here
            // made the honest API shape a 400 ("arg `allowSenders` must be string"). Found by
            // registering a source on a live node; the whole suite was green. One shape, and the
            // schema is the contract.
            "allowSenders": { "type": "array", "items": { "type": "string" },
                "x-lb": { "widget": "tags", "label": "Allowed senders",
                          "description": "Addresses or @domains. Empty admits every sender." } },
            "pollSeconds": { "type": "number", "x-lb": { "label": "Poll seconds" } },
            "channel": { "type": "string",
                "x-lb": { "label": "Inbox channel", "description": "Where arriving mail appears" } }
        },
        "required": ["id", "host", "username", "secretPath"]
    })
}

fn register_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "mail.source.register".to_string(),
        title: "Watch a mailbox (IMAP → inbox + ingest)".to_string(),
        group: "mail".to_string(),
        input_schema: Some(register_schema()),
        result: None,
    }
}

/// The roster, rendered as an interactive table with the two actions an operator reaches for:
/// check the credentials, and stop it.
fn list_render() -> Value {
    json!({
        "v": 2,
        "view": "table",
        "source": { "tool": "mail.source.list", "args": {} },
        "options": { "rowControls": [
            { "kind": "button", "buttonLabel": "Check",
              "action": { "tool": "mail.source.check", "argsTemplate": { "id": "${id}" } } },
            { "kind": "button", "buttonLabel": "Poll now",
              "action": { "tool": "mail.source.poll", "argsTemplate": { "id": "${id}" } } },
            { "kind": "switch", "label": "paused",
              "action": { "tool": "mail.source.pause", "argsTemplate": { "id": "${id}", "paused": "{{value}}" } } }
        ] },
        "fieldConfig": {
            "defaults": {},
            "overrides": [
                { "matcher": { "id": "byName", "options": "secretPath" },
                  "properties": [ { "id": "displayName", "value": "Secret path" },
                                  { "id": "description", "value": "Where the credential is sealed — never the value" } ] },
                { "matcher": { "id": "byName", "options": "lastError" },
                  "properties": [ { "id": "displayName", "value": "Last error" } ] },
                { "matcher": { "id": "byName", "options": "cursor" },
                  "properties": [ { "id": "hide", "value": true } ] },
                { "matcher": { "id": "byName", "options": "oauth" },
                  "properties": [ { "id": "hide", "value": true } ] },
                { "matcher": { "id": "byName", "options": "attachments" },
                  "properties": [ { "id": "hide", "value": true } ] }
            ]
        },
        "tools": ["mail.source.list", "mail.source.check", "mail.source.poll", "mail.source.pause"]
    })
}

fn list_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "mail.source.list".to_string(),
        title: "List watched mailboxes".to_string(),
        group: "mail".to_string(),
        input_schema: Some(json!({ "type": "object", "properties": {} })),
        result: Some(list_render()),
    }
}

fn check_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        // It opens a connection to a third-party mailbox server. Declared honestly so an
        // exfiltration-guarded agent run excludes it (the field is self-declared by design).
        emits_external: true,
        name: "mail.source.check".to_string(),
        title: "Test a mailbox's credentials (imports nothing)".to_string(),
        group: "mail".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "id": { "type": "string", "x-lb": { "label": "Source id" } } },
            "required": ["id"]
        })),
        result: None,
    }
}

fn poll_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: true,
        name: "mail.source.poll".to_string(),
        title: "Poll a mailbox now (imports)".to_string(),
        group: "mail".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "x-lb": { "label": "Source id" } },
                "limit": { "type": "number", "x-lb": { "label": "Max messages" } }
            },
            "required": ["id"]
        })),
        result: None,
    }
}
