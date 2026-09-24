//! Automatic `cache_control` injection + Anthropic API constraint enforcement.
//!
//! The Anthropic Messages API caches input by inserting `cache_control`
//! markers on specific blocks. The API enforces two hard rules that any
//! caller must respect:
//!
//! 1. **Maximum 4 breakpoints per request.** Any request with more than
//!    4 `cache_control` markers is rejected with HTTP 400.
//! 2. **`prompt-caching-scope-2026-01-05` TTL ordering.** In evaluation
//!    order (tools → system → messages), once a 5-minute (default-TTL)
//!    block has been seen, any subsequent block with a longer TTL must
//!    not carry the explicit `ttl` field — the longer TTL would be
//!    silently downgraded server-side, but the request is rejected if
//!    declared.
//!
//! Both rules are always enforced by [`AnthropicRequestTranslator`] after
//! the (swappable) [`CacheControlStrategy`] runs. The strategy decides
//! *where* to place breakpoints; this module enforces *correctness*.
//!
//! [`AnthropicRequestTranslator`]: super::request::AnthropicRequestTranslator

use crate::types::{
    CacheControl, CacheTtl, ContentBlock, MessageContent, MessagesRequest, Role, SystemPrompt,
    TextBlock, TypedContentBlock,
};

/// Maximum number of `cache_control` markers Anthropic accepts per request.
pub const MAX_CACHE_BREAKPOINTS: usize = 4;

// ─── Strategy trait ─────────────────────────────────────────────────────────

/// A pluggable strategy for choosing where to inject `cache_control`
/// markers in an Anthropic [`MessagesRequest`].
///
/// Strategies run after the translator has produced the native request
/// from the canonical [`ChatRequest`] but before serialisation. The
/// translator always invokes [`enforce_breakpoint_cap`] and
/// [`normalize_ttl_ordering`] *after* the strategy so a strategy never
/// has to worry about API correctness rules.
///
/// The default strategy ([`DefaultCacheControlStrategy`]) is idempotent
/// — it skips injection entirely if any breakpoints are already present
/// — so users who set markers themselves on specific blocks don't get
/// double-injection.
///
/// [`ChatRequest`]: aigw_core::model::ChatRequest
pub trait CacheControlStrategy: Send + Sync {
    /// Mutate `req` to add `cache_control` markers.
    fn apply(&self, req: &mut MessagesRequest);
}

// ─── Default strategy ───────────────────────────────────────────────────────

/// Default Anthropic cache-injection strategy.
///
/// When no `cache_control` markers exist anywhere in the request, places
/// `cache_control: { type: "ephemeral" }` at three positions:
///
/// 1. The last [`Tool`] in `tools`.
/// 2. The last block of `system` (converting from [`SystemPrompt::Text`]
///    to [`SystemPrompt::Blocks`] if necessary).
/// 3. The last block of the second-to-last user message in `messages`
///    (converting from [`MessageContent::Text`] to
///    [`MessageContent::Blocks`] if necessary). Skipped if there are
///    fewer than two user messages.
///
/// If *any* `cache_control` marker is already present, the strategy is a
/// no-op — the user explicitly took control and we don't second-guess.
///
/// [`Tool`]: crate::types::Tool
pub struct DefaultCacheControlStrategy {
    /// Marker value to inject. Default is `{ type: "ephemeral" }` (no
    /// explicit TTL — the API uses 5 minutes server-side).
    pub marker: CacheControl,
    /// Minimum number of user messages required before the
    /// "second-to-last user message" injection runs. Default 2.
    pub min_user_messages: usize,
}

impl Default for DefaultCacheControlStrategy {
    fn default() -> Self {
        Self {
            marker: ephemeral_marker(),
            min_user_messages: 2,
        }
    }
}

impl CacheControlStrategy for DefaultCacheControlStrategy {
    fn apply(&self, req: &mut MessagesRequest) {
        if has_any_cache_control(req) {
            return;
        }
        inject_tools_cache(req, &self.marker);
        inject_system_cache(req, &self.marker);
        inject_messages_cache(req, &self.marker, self.min_user_messages);
    }
}

/// No-op strategy — disables automatic injection entirely while still
/// letting the translator apply the always-on correctness rules
/// ([`enforce_breakpoint_cap`] and [`normalize_ttl_ordering`]).
pub struct NoCacheControlStrategy;

impl CacheControlStrategy for NoCacheControlStrategy {
    fn apply(&self, _req: &mut MessagesRequest) {}
}

// ─── Construction helpers ───────────────────────────────────────────────────

/// Build a default `{ type: "ephemeral" }` marker (no explicit TTL).
#[must_use]
pub fn ephemeral_marker() -> CacheControl {
    CacheControl {
        r#type: "ephemeral".to_owned(),
        ttl: None,
    }
}

/// Build a `{ type: "ephemeral", ttl: <ttl> }` marker, e.g. with
/// [`CacheTtl::OneHour`] for a longer-than-default cache lifetime.
#[must_use]
pub fn ephemeral_marker_with_ttl(ttl: CacheTtl) -> CacheControl {
    CacheControl {
        r#type: "ephemeral".to_owned(),
        ttl: Some(ttl),
    }
}

// ─── Idempotency check ──────────────────────────────────────────────────────

fn has_any_cache_control(req: &MessagesRequest) -> bool {
    if let Some(tools) = &req.tools
        && tools.iter().any(|t| t.cache_control.is_some())
    {
        return true;
    }
    if let Some(SystemPrompt::Blocks(blocks)) = &req.system
        && blocks.iter().any(|b| b.cache_control.is_some())
    {
        return true;
    }
    for msg in &req.messages {
        if let MessageContent::Blocks(blocks) = &msg.content {
            for block in blocks {
                if block_has_cache_control(block) {
                    return true;
                }
            }
        }
    }
    false
}

fn block_has_cache_control(block: &ContentBlock) -> bool {
    match block {
        ContentBlock::Typed(t) => typed_block_cache_control(t).is_some(),
        ContentBlock::Raw(obj) => obj.contains_key("cache_control"),
    }
}

fn typed_block_cache_control(t: &TypedContentBlock) -> Option<&CacheControl> {
    match t {
        TypedContentBlock::Text { cache_control, .. }
        | TypedContentBlock::Image { cache_control, .. }
        | TypedContentBlock::ToolUse { cache_control, .. }
        | TypedContentBlock::ToolResult { cache_control, .. } => cache_control.as_ref(),
        TypedContentBlock::Thinking { .. } | TypedContentBlock::RedactedThinking { .. } => None,
    }
}

fn typed_block_cache_control_mut(t: &mut TypedContentBlock) -> Option<&mut Option<CacheControl>> {
    match t {
        TypedContentBlock::Text { cache_control, .. }
        | TypedContentBlock::Image { cache_control, .. }
        | TypedContentBlock::ToolUse { cache_control, .. }
        | TypedContentBlock::ToolResult { cache_control, .. } => Some(cache_control),
        TypedContentBlock::Thinking { .. } | TypedContentBlock::RedactedThinking { .. } => None,
    }
}

// ─── Injection — the three positions ────────────────────────────────────────

fn inject_tools_cache(req: &mut MessagesRequest, marker: &CacheControl) {
    if let Some(tools) = &mut req.tools
        && let Some(last) = tools.last_mut()
        && last.cache_control.is_none()
    {
        last.cache_control = Some(marker.clone());
    }
}

fn inject_system_cache(req: &mut MessagesRequest, marker: &CacheControl) {
    match req.system.as_mut() {
        Some(sp @ SystemPrompt::Text(_)) => {
            // Promote string → blocks so we have somewhere to attach
            // cache_control. Empty strings are left alone (no point
            // caching nothing).
            let SystemPrompt::Text(text) = sp else {
                unreachable!()
            };
            if text.is_empty() {
                return;
            }
            let promoted = vec![TextBlock {
                r#type: "text".to_owned(),
                text: std::mem::take(text),
                cache_control: Some(marker.clone()),
            }];
            *sp = SystemPrompt::Blocks(promoted);
        }
        Some(SystemPrompt::Blocks(blocks)) => {
            if let Some(last) = blocks.last_mut()
                && last.cache_control.is_none()
            {
                last.cache_control = Some(marker.clone());
            }
        }
        None => {}
    }
}

fn inject_messages_cache(
    req: &mut MessagesRequest,
    marker: &CacheControl,
    min_user_messages: usize,
) {
    let user_indices: Vec<usize> = req
        .messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == Role::User)
        .map(|(i, _)| i)
        .collect();

    if user_indices.len() < min_user_messages {
        return;
    }
    let target_idx = user_indices[user_indices.len() - 2];
    let msg = &mut req.messages[target_idx];

    // Promote string content → block array so we have somewhere to
    // attach cache_control.
    if let MessageContent::Text(text) = &msg.content {
        let promoted = vec![ContentBlock::Typed(TypedContentBlock::Text {
            text: text.clone(),
            cache_control: None,
        })];
        msg.content = MessageContent::Blocks(promoted);
    }

    if let MessageContent::Blocks(blocks) = &mut msg.content
        && let Some(last) = blocks.last_mut()
        && let ContentBlock::Typed(typed) = last
        && let Some(slot) = typed_block_cache_control_mut(typed)
        && slot.is_none()
    {
        *slot = Some(marker.clone());
    }
}

// ─── Counting ──────────────────────────────────────────────────────────────

/// Count `cache_control` markers in tools, system, and message content.
///
/// Public for tests and for callers wanting to verify their own
/// post-translation state.
#[must_use]
pub fn count_cache_controls(req: &MessagesRequest) -> usize {
    let mut count = 0;
    if let Some(tools) = &req.tools {
        count += tools.iter().filter(|t| t.cache_control.is_some()).count();
    }
    if let Some(SystemPrompt::Blocks(blocks)) = &req.system {
        count += blocks.iter().filter(|b| b.cache_control.is_some()).count();
    }
    for msg in &req.messages {
        if let MessageContent::Blocks(blocks) = &msg.content {
            for block in blocks {
                if block_has_cache_control(block) {
                    count += 1;
                }
            }
        }
    }
    count
}

// ─── Always-on rule 1: 4-breakpoint cap ─────────────────────────────────────

/// Strip excess `cache_control` markers down to `MAX_CACHE_BREAKPOINTS`
/// (4) per Anthropic's hard API limit.
///
/// Removal priority (lowest-value first):
///
/// 1. System blocks earliest-first (preserve last).
/// 2. Tool blocks earliest-first (preserve last).
/// 3. Message content blocks earliest-first.
pub fn enforce_breakpoint_cap(req: &mut MessagesRequest) {
    enforce_breakpoint_cap_with_max(req, MAX_CACHE_BREAKPOINTS);
}

/// As [`enforce_breakpoint_cap`] but with a custom maximum. Useful for
/// testing; production should use the constant.
pub fn enforce_breakpoint_cap_with_max(req: &mut MessagesRequest, max_blocks: usize) {
    let total = count_cache_controls(req);
    if total <= max_blocks {
        return;
    }
    let mut excess = total - max_blocks;

    // Phase 1: system blocks (preserve last).
    if let Some(SystemPrompt::Blocks(blocks)) = req.system.as_mut() {
        let last_cc = blocks.iter().rposition(|b| b.cache_control.is_some());
        for (i, block) in blocks.iter_mut().enumerate() {
            if excess == 0 {
                break;
            }
            if Some(i) != last_cc && block.cache_control.is_some() {
                block.cache_control = None;
                excess -= 1;
            }
        }
    }
    if excess == 0 {
        return;
    }

    // Phase 2: tool blocks (preserve last).
    if let Some(tools) = req.tools.as_mut() {
        let last_cc = tools.iter().rposition(|t| t.cache_control.is_some());
        for (i, t) in tools.iter_mut().enumerate() {
            if excess == 0 {
                break;
            }
            if Some(i) != last_cc && t.cache_control.is_some() {
                t.cache_control = None;
                excess -= 1;
            }
        }
    }
    if excess == 0 {
        return;
    }

    // Phase 3: message content blocks (earliest-first, no preservation).
    for msg in req.messages.iter_mut() {
        if excess == 0 {
            break;
        }
        if let MessageContent::Blocks(blocks) = &mut msg.content {
            for block in blocks.iter_mut() {
                if excess == 0 {
                    break;
                }
                clear_cache_control(block, &mut excess);
            }
        }
    }
}

fn clear_cache_control(block: &mut ContentBlock, excess: &mut usize) {
    match block {
        ContentBlock::Typed(t) => {
            if let Some(slot) = typed_block_cache_control_mut(t)
                && slot.is_some()
            {
                *slot = None;
                *excess -= 1;
            }
        }
        ContentBlock::Raw(obj) => {
            if obj.remove("cache_control").is_some() {
                *excess -= 1;
            }
        }
    }
}

// ─── Always-on rule 2: TTL ordering ─────────────────────────────────────────

/// Normalize `cache_control.ttl` for `prompt-caching-scope-2026-01-05`.
///
/// The API requires that, in evaluation order (tools → system → message
/// content), no block with an explicit TTL longer than the default 5
/// minutes appears *after* a block with the default short TTL. Violating
/// this returns HTTP 400.
///
/// This function walks the request in evaluation order and strips the
/// explicit `ttl` field from any longer-TTL block that follows a short
/// block. The block keeps its `cache_control` marker — only the `ttl`
/// override is removed, so the block falls back to the 5-minute default.
pub fn normalize_ttl_ordering(req: &mut MessagesRequest) {
    let mut seen_short = false;

    // Tools.
    if let Some(tools) = req.tools.as_mut() {
        for t in tools.iter_mut() {
            if let Some(cc) = t.cache_control.as_mut() {
                normalize_ttl(cc, &mut seen_short);
            }
        }
    }
    // System.
    if let Some(SystemPrompt::Blocks(blocks)) = req.system.as_mut() {
        for b in blocks.iter_mut() {
            if let Some(cc) = b.cache_control.as_mut() {
                normalize_ttl(cc, &mut seen_short);
            }
        }
    }
    // Messages.
    for msg in req.messages.iter_mut() {
        if let MessageContent::Blocks(blocks) = &mut msg.content {
            for block in blocks.iter_mut() {
                normalize_block_ttl(block, &mut seen_short);
            }
        }
    }
}

fn normalize_block_ttl(block: &mut ContentBlock, seen_short: &mut bool) {
    match block {
        ContentBlock::Typed(t) => {
            if let Some(slot) = typed_block_cache_control_mut(t)
                && let Some(cc) = slot.as_mut()
            {
                normalize_ttl(cc, seen_short);
            }
        }
        ContentBlock::Raw(obj) => {
            if let Some(cc_val) = obj.get_mut("cache_control")
                && let serde_json::Value::Object(cc_obj) = cc_val
            {
                let ttl = cc_obj
                    .get("ttl")
                    .and_then(|v| serde_json::from_value::<CacheTtl>(v.clone()).ok());
                match ttl {
                    Some(ttl) if ttl.is_extended() => {
                        if *seen_short {
                            cc_obj.remove("ttl");
                        }
                    }
                    _ => *seen_short = true,
                }
            }
        }
    }
}

fn normalize_ttl(cc: &mut CacheControl, seen_short: &mut bool) {
    match &cc.ttl {
        Some(ttl) if ttl.is_extended() => {
            if *seen_short {
                cc.ttl = None;
            }
        }
        _ => *seen_short = true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Message, Tool};

    fn user_text(text: &str) -> Message {
        Message {
            role: Role::User,
            content: MessageContent::Text(text.to_owned()),
        }
    }

    fn assistant_text(text: &str) -> Message {
        Message {
            role: Role::Assistant,
            content: MessageContent::Text(text.to_owned()),
        }
    }

    fn req_with(messages: Vec<Message>) -> MessagesRequest {
        MessagesRequest::builder()
            .model("claude-sonnet-4-20250514")
            .messages(messages)
            .max_tokens(1024_u64)
            .build()
    }

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_owned(),
            description: None,
            input_schema: serde_json::json!({"type":"object"}),
            cache_control: None,
        }
    }

    fn one_hour() -> CacheControl {
        ephemeral_marker_with_ttl(CacheTtl::OneHour)
    }

    // ── Default strategy: 3-position injection ────────────────────────────

    #[test]
    fn default_strategy_no_op_with_only_one_user_message() {
        let mut req = req_with(vec![user_text("hi")]);
        DefaultCacheControlStrategy::default().apply(&mut req);
        // Single user message → no message-side breakpoint added.
        assert_eq!(count_cache_controls(&req), 0);
    }

    #[test]
    fn default_strategy_injects_on_second_to_last_user_message() {
        let mut req = req_with(vec![
            user_text("first"),
            assistant_text("reply"),
            user_text("second"),
        ]);
        DefaultCacheControlStrategy::default().apply(&mut req);
        // First user message gets the breakpoint, second is untouched.
        let MessageContent::Blocks(blocks) = &req.messages[0].content else {
            panic!("expected Blocks");
        };
        let typed = match &blocks[0] {
            ContentBlock::Typed(t) => t,
            _ => panic!("expected Typed"),
        };
        let TypedContentBlock::Text { cache_control, .. } = typed else {
            panic!("expected Text");
        };
        assert!(cache_control.is_some());
        // Second user message untouched (still string).
        assert!(matches!(req.messages[2].content, MessageContent::Text(_)));
    }

    #[test]
    fn default_strategy_injects_on_last_tool() {
        let mut req = req_with(vec![user_text("hi")]);
        req.tools = Some(vec![tool("a"), tool("b")]);
        DefaultCacheControlStrategy::default().apply(&mut req);
        let tools = req.tools.as_ref().unwrap();
        assert!(tools[0].cache_control.is_none());
        assert!(tools[1].cache_control.is_some());
    }

    #[test]
    fn default_strategy_promotes_system_string_to_blocks() {
        let mut req = req_with(vec![user_text("hi")]);
        req.system = Some(SystemPrompt::Text("You are helpful.".into()));
        DefaultCacheControlStrategy::default().apply(&mut req);
        let SystemPrompt::Blocks(blocks) = req.system.as_ref().unwrap() else {
            panic!("expected promoted Blocks");
        };
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "You are helpful.");
        assert!(blocks[0].cache_control.is_some());
    }

    #[test]
    fn default_strategy_injects_on_last_system_block() {
        let mut req = req_with(vec![user_text("hi")]);
        req.system = Some(SystemPrompt::Blocks(vec![
            TextBlock {
                r#type: "text".into(),
                text: "first".into(),
                cache_control: None,
            },
            TextBlock {
                r#type: "text".into(),
                text: "second".into(),
                cache_control: None,
            },
        ]));
        DefaultCacheControlStrategy::default().apply(&mut req);
        let SystemPrompt::Blocks(blocks) = req.system.as_ref().unwrap() else {
            panic!()
        };
        assert!(blocks[0].cache_control.is_none());
        assert!(blocks[1].cache_control.is_some());
    }

    #[test]
    fn default_strategy_skips_when_any_cache_control_present() {
        let mut req = req_with(vec![user_text("first"), user_text("second")]);
        req.tools = Some(vec![Tool {
            name: "t".into(),
            description: None,
            input_schema: serde_json::json!({"type":"object"}),
            cache_control: Some(ephemeral_marker()),
        }]);
        DefaultCacheControlStrategy::default().apply(&mut req);
        // Only the user-set tool marker, no message breakpoints injected.
        assert_eq!(count_cache_controls(&req), 1);
    }

    #[test]
    fn default_strategy_skips_empty_system_string() {
        let mut req = req_with(vec![user_text("hi")]);
        req.system = Some(SystemPrompt::Text(String::new()));
        DefaultCacheControlStrategy::default().apply(&mut req);
        // Empty string isn't promoted.
        assert!(matches!(req.system, Some(SystemPrompt::Text(_))));
        assert_eq!(count_cache_controls(&req), 0);
    }

    #[test]
    fn no_cache_control_strategy_is_a_noop() {
        let mut req = req_with(vec![user_text("first"), user_text("second")]);
        req.tools = Some(vec![tool("a")]);
        NoCacheControlStrategy.apply(&mut req);
        assert_eq!(count_cache_controls(&req), 0);
    }

    // ── 4-breakpoint cap ─────────────────────────────────────────────────

    #[test]
    fn enforce_cap_strips_excess() {
        let mut req = req_with(vec![user_text("hi")]);
        req.tools = Some(vec![
            Tool {
                name: "a".into(),
                description: None,
                input_schema: serde_json::json!({"type":"object"}),
                cache_control: Some(ephemeral_marker()),
            },
            Tool {
                name: "b".into(),
                description: None,
                input_schema: serde_json::json!({"type":"object"}),
                cache_control: Some(ephemeral_marker()),
            },
            Tool {
                name: "c".into(),
                description: None,
                input_schema: serde_json::json!({"type":"object"}),
                cache_control: Some(ephemeral_marker()),
            },
        ]);
        req.system = Some(SystemPrompt::Blocks(vec![
            TextBlock {
                r#type: "text".into(),
                text: "s1".into(),
                cache_control: Some(ephemeral_marker()),
            },
            TextBlock {
                r#type: "text".into(),
                text: "s2".into(),
                cache_control: Some(ephemeral_marker()),
            },
        ]));
        // 5 total → must drop to 4.
        enforce_breakpoint_cap(&mut req);
        assert_eq!(count_cache_controls(&req), 4);
        // Last system block must be preserved.
        let SystemPrompt::Blocks(blocks) = req.system.as_ref().unwrap() else {
            panic!()
        };
        assert!(blocks[1].cache_control.is_some());
        // Last tool must be preserved.
        let tools = req.tools.as_ref().unwrap();
        assert!(tools[2].cache_control.is_some());
    }

    #[test]
    fn enforce_cap_under_limit_no_change() {
        let mut req = req_with(vec![user_text("hi")]);
        req.tools = Some(vec![Tool {
            name: "a".into(),
            description: None,
            input_schema: serde_json::json!({"type":"object"}),
            cache_control: Some(ephemeral_marker()),
        }]);
        enforce_breakpoint_cap(&mut req);
        assert_eq!(count_cache_controls(&req), 1);
    }

    // ── TTL ordering ─────────────────────────────────────────────────────

    #[test]
    fn normalize_ttl_strips_long_after_short() {
        let mut req = req_with(vec![user_text("hi")]);
        req.tools = Some(vec![
            Tool {
                name: "a".into(),
                description: None,
                input_schema: serde_json::json!({"type":"object"}),
                // No TTL → counts as short.
                cache_control: Some(ephemeral_marker()),
            },
            Tool {
                name: "b".into(),
                description: None,
                input_schema: serde_json::json!({"type":"object"}),
                // 1h → must be downgraded.
                cache_control: Some(one_hour()),
            },
        ]);
        normalize_ttl_ordering(&mut req);
        let tools = req.tools.as_ref().unwrap();
        assert!(tools[0].cache_control.as_ref().unwrap().ttl.is_none());
        // ttl was stripped because a short block came first.
        assert!(tools[1].cache_control.as_ref().unwrap().ttl.is_none());
    }

    #[test]
    fn normalize_ttl_preserves_long_when_no_short_seen() {
        let mut req = req_with(vec![user_text("hi")]);
        req.tools = Some(vec![
            Tool {
                name: "a".into(),
                description: None,
                input_schema: serde_json::json!({"type":"object"}),
                cache_control: Some(one_hour()),
            },
            Tool {
                name: "b".into(),
                description: None,
                input_schema: serde_json::json!({"type":"object"}),
                cache_control: Some(one_hour()),
            },
        ]);
        normalize_ttl_ordering(&mut req);
        let tools = req.tools.as_ref().unwrap();
        assert_eq!(
            tools[0].cache_control.as_ref().unwrap().ttl,
            Some(CacheTtl::OneHour)
        );
        assert_eq!(
            tools[1].cache_control.as_ref().unwrap().ttl,
            Some(CacheTtl::OneHour)
        );
    }

    #[test]
    fn normalize_ttl_walks_in_evaluation_order() {
        // Long-TTL system block follows short-TTL tool → must be stripped.
        let mut req = req_with(vec![user_text("hi")]);
        req.tools = Some(vec![Tool {
            name: "a".into(),
            description: None,
            input_schema: serde_json::json!({"type":"object"}),
            cache_control: Some(ephemeral_marker()),
        }]);
        req.system = Some(SystemPrompt::Blocks(vec![TextBlock {
            r#type: "text".into(),
            text: "s".into(),
            cache_control: Some(one_hour()),
        }]));
        normalize_ttl_ordering(&mut req);
        let SystemPrompt::Blocks(blocks) = req.system.as_ref().unwrap() else {
            panic!()
        };
        assert!(blocks[0].cache_control.as_ref().unwrap().ttl.is_none());
    }

    #[test]
    fn claude_code_style_ttl_strings_parse_and_normalize() {
        // The API (and Claude Code) spell TTLs as strings.
        let mut req: MessagesRequest = serde_json::from_value(serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 16,
            "system": [
                {"type": "text", "text": "a", "cache_control": {"type": "ephemeral", "ttl": "1h"}},
                {"type": "text", "text": "b", "cache_control": {"type": "ephemeral", "ttl": "5m"}},
            ],
            "messages": [{"role": "user", "content": [
                {"type": "text", "text": "hi", "cache_control": {"type": "ephemeral", "ttl": "1h"}},
            ]}],
        }))
        .unwrap();
        normalize_ttl_ordering(&mut req);
        let body = serde_json::to_value(&req).unwrap();
        assert_eq!(body["system"][0]["cache_control"]["ttl"], "1h");
        assert_eq!(body["system"][1]["cache_control"]["ttl"], "5m");
        // A 1h block after a 5m one loses its explicit TTL.
        assert!(
            body["messages"][0]["content"][0]["cache_control"]
                .get("ttl")
                .is_none()
        );
    }

    #[test]
    fn unknown_ttls_round_trip_and_count_as_extended() {
        let cc: CacheControl =
            serde_json::from_value(serde_json::json!({"type": "ephemeral", "ttl": "24h"})).unwrap();
        assert_eq!(cc.ttl, Some(CacheTtl::Other("24h".into())));
        assert!(cc.ttl.as_ref().unwrap().is_extended());
        assert_eq!(serde_json::to_value(&cc).unwrap()["ttl"], "24h");
        assert_eq!(serde_json::to_value(one_hour()).unwrap()["ttl"], "1h");
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    #[test]
    fn dyn_dispatch_compiles() {
        let _: Box<dyn CacheControlStrategy> = Box::new(DefaultCacheControlStrategy::default());
        let _: Box<dyn CacheControlStrategy> = Box::new(NoCacheControlStrategy);
    }
}
