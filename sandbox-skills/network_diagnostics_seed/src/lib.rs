#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::panic::PanicInfo;

// -----------------------------------------------------------------------------
// Simple Bump Allocator (2MB static heap for Wasm)
// -----------------------------------------------------------------------------
struct BumpAllocator {
    heap: UnsafeCell<[u8; 1024 * 1024 * 2]>,
    next: UnsafeCell<usize>,
}

unsafe impl Sync for BumpAllocator {}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator {
    heap: UnsafeCell::new([0; 1024 * 1024 * 2]),
    next: UnsafeCell::new(0),
};

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();
        let mut next = *self.next.get();
        let remainder = next % align;
        if remainder != 0 { next += align - remainder; }
        let start = next;
        let end = start + size;
        if end > 1024 * 1024 * 2 { return core::ptr::null_mut(); }
        *self.next.get() = end;
        (*self.heap.get()).as_mut_ptr().add(start)
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

// -----------------------------------------------------------------------------
// OpenRhiza Wasm Host ABI & Display ABI
// -----------------------------------------------------------------------------
#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_request_display_mode(backend: u32, cols: u32, rows: u32, px_w: u32, px_h: u32);
    fn os_gui_set_object_bounds(handle: u32, x: u32, y: u32, w: u32, h: u32);
    fn os_gui_set_object_style(handle: u32, style: u32);
    fn os_gui_set_object_label(handle: u32, ptr: *const u8, len: u32);
}

fn log(s: &str) {
    unsafe { os_log(s.as_ptr(), s.len() as u32); }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop {} }

const STYLE_CHROME: u32 = 0;
const STYLE_SIDEBAR: u32 = 1;
const STYLE_PLAIN: u32 = 10;
const STYLE_ACCENT: u32 = 11;

// -----------------------------------------------------------------------------
// Virtual DOM Definitions
// -----------------------------------------------------------------------------
pub struct Node {
    pub id: u32,
    pub tag: String,
    pub text: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Box<Node>>,
    pub is_interactive: bool,
}

impl Node {
    fn new(id: u32, tag: &str, attrs: Vec<(String, String)>) -> Self {
        let is_interactive = tag.eq_ignore_ascii_case("a") || tag.eq_ignore_ascii_case("button") || tag.eq_ignore_ascii_case("input");
        Self {
            id,
            tag: tag.to_string(),
            text: String::new(),
            attributes: attrs,
            children: Vec::new(),
            is_interactive,
        }
    }
}

static mut DOM_ROOT: Option<Box<Node>> = None;
static mut NEXT_NODE_ID: u32 = 500;
static mut NETWORK_LOGS: Vec<String> = Vec::new();

unsafe fn find_node_mut<'a>(root: &'a mut Node, target_id: u32) -> Option<&'a mut Node> {
    if root.id == target_id { return Some(root); }
    for child in &mut root.children {
        if let Some(found) = find_node_mut(child, target_id) { return Some(found); }
    }
    None
}

// -----------------------------------------------------------------------------
// Network Diagnostics Commands (Agent API)
// -----------------------------------------------------------------------------
unsafe fn append_network_log(msg: &str) {
    NETWORK_LOGS.push(String::from(msg));
    log(&alloc::format!("[Diag Tool] {}", msg));
    render_dashboard();
}

#[no_mangle]
pub extern "C" fn agent_cmd_ping(ip_ptr: *const u8, len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(ip_ptr, len as usize);
        if let Ok(ip) = core::str::from_utf8(slice) {
            append_network_log(&alloc::format!("PING {} (56 data bytes)", ip));
            
            // Simulating a network failure or success randomly or statically
            if ip.contains("8.8.8.8") {
                append_network_log(&alloc::format!("64 bytes from {}: icmp_seq=1 ttl=115 time=12.4 ms", ip));
            } else {
                append_network_log(&alloc::format!("From {}: icmp_seq=1 Destination Host Unreachable", ip));
            }
            return 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn agent_cmd_dns_lookup(domain_ptr: *const u8, len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(domain_ptr, len as usize);
        if let Ok(domain) = core::str::from_utf8(slice) {
            append_network_log(&alloc::format!("DNS QUERY: {}", domain));
            append_network_log(&alloc::format!("Address: 142.250.190.46"));
            return 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn agent_cmd_reset_nic() -> u32 {
    unsafe {
        append_network_log(">>> RESET_NIC command received.");
        append_network_log("Sending ACPI Soft Reset to e1000 PCIe device...");
        append_network_log("Link down.");
        append_network_log("Link up at 1000 Mbps, full duplex.");
        append_network_log("NIC Self-Healing sequence completed.");
    }
    1
}

// -----------------------------------------------------------------------------
// HTML to DOM Parser (Simplified from Browser)
// -----------------------------------------------------------------------------
struct HtmlParser<'a> {
    html: &'a str,
    pos: usize,
}

impl<'a> HtmlParser<'a> {
    fn new(html: &'a str) -> Self { Self { html, pos: 0 } }
    fn peek(&self) -> Option<char> { self.html[self.pos..].chars().next() }
    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }
    fn parse_tag(&mut self) -> &'a str {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == '>' {
                let tag = &self.html[start..self.pos];
                self.advance();
                return tag;
            }
            self.advance();
        }
        &self.html[start..self.pos]
    }
}

fn parse_attributes(content: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.peek() { if c.is_whitespace() { break; } chars.next(); }
    while let Some(c) = chars.next() {
        if c.is_whitespace() { continue; }
        let mut key = String::new(); key.push(c);
        while let Some(&k) = chars.peek() { if k == '=' || k.is_whitespace() { break; } key.push(chars.next().unwrap()); }
        let mut val = String::new();
        while let Some(&n) = chars.peek() {
            if n.is_whitespace() { chars.next(); continue; }
            if n == '=' {
                chars.next(); 
                while let Some(&s) = chars.peek() { if !s.is_whitespace() { break; } chars.next(); }
                if let Some(&quote) = chars.peek() {
                    if quote == '"' || quote == '\'' {
                        chars.next(); 
                        while let Some(v) = chars.next() { if v == quote { break; } val.push(v); }
                    } else {
                        while let Some(v) = chars.next() { if v.is_whitespace() || v == '>' || v == '/' { break; } val.push(v); }
                    }
                }
                break;
            } else { break; }
        }
        if !key.is_empty() { attrs.push((key, val)); }
    }
    attrs
}

unsafe fn parse_html_to_dom(html: &str) {
    let mut parser = HtmlParser::new(html);
    let root = Box::new(Node::new(0, "document", Vec::new()));
    let mut stack: Vec<Box<Node>> = Vec::new();
    stack.push(root);

    while let Some(c) = parser.advance() {
        if c == '<' {
            let tag_content = parser.parse_tag();
            if tag_content.starts_with("!--") { continue; }
            let mut name = tag_content.split_whitespace().next().unwrap_or("").trim_start_matches('/');
            if name.ends_with('/') { name = name.trim_end_matches('/'); }
            let is_closing = tag_content.starts_with('/');
            let is_void = matches!(name.to_lowercase().as_str(), "br" | "hr" | "img" | "input" | "link" | "meta" | "!doctype");
            
            if !is_closing {
                let id = NEXT_NODE_ID;
                NEXT_NODE_ID += 1;
                let attrs = parse_attributes(tag_content);
                let new_node = Box::new(Node::new(id, name, attrs));
                if is_void {
                    if let Some(top) = stack.last_mut() { top.children.push(new_node); }
                } else {
                    stack.push(new_node);
                }
            } else {
                if stack.len() > 1 {
                    let completed_node = stack.pop().unwrap();
                    if let Some(top) = stack.last_mut() { top.children.push(completed_node); }
                }
            }
        } else {
            if let Some(top) = stack.last_mut() {
                if c.is_whitespace() {
                    if !top.text.ends_with(' ') && !top.text.is_empty() { top.text.push(' '); }
                } else {
                    top.text.push(c);
                }
            }
        }
    }
    while stack.len() > 1 {
        let completed_node = stack.pop().unwrap();
        if let Some(top) = stack.last_mut() { top.children.push(completed_node); }
    }
    if stack.len() == 1 { DOM_ROOT = Some(stack.pop().unwrap()); }
}

// -----------------------------------------------------------------------------
// Recursive GUI Layout Engine
// -----------------------------------------------------------------------------
struct LayoutContext {
    current_y: u32,
    margin_x: u32,
    content_width: u32,
}

unsafe fn layout_dom_node(node: &Node, ctx: &mut LayoutContext, depth: u32) {
    let tag = node.tag.to_lowercase();
    let text = node.text.trim();
    
    let mut style = STYLE_PLAIN;
    let mut height = 24;
    let mut indent = depth * 15;

    match tag.as_str() {
        "h1" => { style = STYLE_ACCENT; height = 46; indent = 0; }
        "h2" | "h3" => { style = STYLE_ACCENT; height = 34; indent = 0; }
        "button" => { style = STYLE_SIDEBAR; height = 30; }
        "input" => { style = STYLE_CHROME; height = 30; }
        "li" => { indent += 10; }
        _ => {}
    }

    for (k, v) in &node.attributes {
        if k.eq_ignore_ascii_case("class") {
            if v.contains("os-chrome") { style = STYLE_CHROME; }
            else if v.contains("os-sidebar") { style = STYLE_SIDEBAR; }
            else if v.contains("os-accent") { style = STYLE_ACCENT; }
        }
    }

    let mut is_log_area = false;
    for (k, v) in &node.attributes {
        if k.eq_ignore_ascii_case("id") && v.eq_ignore_ascii_case("log_output") {
            is_log_area = true;
        }
    }

    if is_log_area {
        // Draw the log area
        os_gui_set_object_bounds(node.id, ctx.margin_x + indent, ctx.current_y, ctx.content_width - indent, 200);
        os_gui_set_object_style(node.id, STYLE_CHROME);
        let header = alloc::format!("--- TERMINAL DIAGNOSTICS LOG ---");
        os_gui_set_object_label(node.id, header.as_ptr(), header.len() as u32);
        ctx.current_y += 30;

        for log_line in &NETWORK_LOGS {
            let log_id = NEXT_NODE_ID; NEXT_NODE_ID += 1;
            os_gui_set_object_bounds(log_id, ctx.margin_x + indent + 10, ctx.current_y, ctx.content_width - indent - 20, 20);
            os_gui_set_object_style(log_id, STYLE_PLAIN);
            os_gui_set_object_label(log_id, log_line.as_ptr(), log_line.len() as u32);
            ctx.current_y += 24;
        }
        ctx.current_y += 10;
        return; // skip children rendering for this node manually
    }

    if !text.is_empty() || !node.attributes.is_empty() || tag == "input" {
        let mut display_text = if tag == "li" { alloc::format!("* {}", text) } else { text.to_string() };
        
        if tag == "input" {
            let mut placeholder = "...";
            for (k, v) in &node.attributes { if k.eq_ignore_ascii_case("placeholder") { placeholder = v; } }
            display_text = if text.is_empty() { alloc::format!("[Input: {}]", placeholder) } else { alloc::format!("[Input: {}]", text) };
        }

        if !display_text.is_empty() {
            os_gui_set_object_bounds(node.id, ctx.margin_x + indent, ctx.current_y, ctx.content_width - indent, height);
            os_gui_set_object_style(node.id, style);
            os_gui_set_object_label(node.id, display_text.as_ptr(), display_text.len() as u32);
            ctx.current_y += height + 8;
        }
    }

    for child in &node.children {
        layout_dom_node(child, ctx, depth + 1);
    }
}

unsafe fn render_dashboard() {
    let html = "\
        <html>\n\
            <body>\n\
                <h1 class='os-accent'>OpenRhiza Auto-Troubleshooter</h1>\n\
                <p>Agent Diagnostics Console.</p>\n\
                <br/>\n\
                <div>\n\
                    <h3>Manual Agent Commands</h3>\n\
                    <ul>\n\
                        <li><button class='os-sidebar'>Run Ping (8.8.8.8)</button></li>\n\
                        <li><button class='os-sidebar'>Test Unreachable IP</button></li>\n\
                        <li><button class='os-accent'>Reset Network Card (NIC)</button></li>\n\
                    </ul>\n\
                </div>\n\
                <br/>\n\
                <div id='log_output'></div>\n\
            </body>\n\
        </html>\
    ";
    
    DOM_ROOT = None; 
    NEXT_NODE_ID = 500;
    parse_html_to_dom(html);

    if let Some(ref root) = DOM_ROOT {
        os_gui_set_object_bounds(100, 20, 20, 800, 40);
        os_gui_set_object_style(100, STYLE_CHROME);
        let title = b"Network Diagnostics VFS Window";
        os_gui_set_object_label(100, title.as_ptr(), title.len() as u32);

        let mut ctx = LayoutContext {
            current_y: 70,
            margin_x: 20,
            content_width: 800,
        };

        layout_dom_node(root, &mut ctx, 0);
    }
}

// -----------------------------------------------------------------------------
// Agent Automation Handlers
// -----------------------------------------------------------------------------
#[no_mangle]
pub extern "C" fn browser_click_element(id: u32) -> u32 {
    unsafe {
        log(&alloc::format!("[Diag Tool] CLICK on ID {}", id));
        let mut clicked_text = String::new();
        if let Some(ref mut root) = DOM_ROOT {
            if let Some(node) = find_node_mut(root, id) {
                clicked_text = node.text.clone();
            }
        }
        
        if clicked_text.contains("Run Ping") {
            let ip = "8.8.8.8";
            agent_cmd_ping(ip.as_ptr(), ip.len() as u32);
        } else if clicked_text.contains("Unreachable") {
            let ip = "192.168.99.99";
            agent_cmd_ping(ip.as_ptr(), ip.len() as u32);
        } else if clicked_text.contains("Reset Network") {
            agent_cmd_reset_nic();
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn agent_eval_js(script_ptr: *const u8, script_len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(script_ptr, script_len as usize);
        if let Ok(script) = core::str::from_utf8(slice) {
            log(&alloc::format!("[JS Engine] Diagnostic script: {}", script));
            let s = script.trim();
            if s.starts_with("ping(") {
                let inner = s[5..].trim_end_matches(|c| c == ')' || c == ';').trim_matches(|c| c == '"' || c == '\'');
                agent_cmd_ping(inner.as_ptr(), inner.len() as u32);
            } else if s.starts_with("reset_nic()") {
                agent_cmd_reset_nic();
            } else {
                log("[JS Engine] Unknown diag command.");
                return 0;
            }
        }
    }
    1
}

// -----------------------------------------------------------------------------
// Skill Entry Points
// -----------------------------------------------------------------------------
#[no_mangle]
pub extern "C" fn init_driver() {
    log("[Diag Tool] Network Auto-Troubleshooter Initialized.");
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log("[Diag Tool] Starting GUI Dashboard...");
    unsafe { 
        os_request_display_mode(2, 0, 0, 1024, 768); 
        NETWORK_LOGS.push(String::from("System Boot. Diagnostic module loaded."));
        NETWORK_LOGS.push(String::from("Awaiting LLM Agent commands..."));
        render_dashboard();
    }
    1
}
