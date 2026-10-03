#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::{String, ToString};
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::panic::PanicInfo;

// -----------------------------------------------------------------------------
// Bump Allocator (Used for UI strings, wiped every frame)
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

impl BumpAllocator {
    pub unsafe fn reset(&self) {
        *self.next.get() = 0;
    }
}

// -----------------------------------------------------------------------------
// Wasm Host ABI & Display ABI
// -----------------------------------------------------------------------------
#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_request_display_mode(backend: u32, cols: u32, rows: u32, px_w: u32, px_h: u32);
    fn os_gui_set_object_bounds(handle: u32, x: u32, y: u32, w: u32, h: u32);
    fn os_gui_set_object_style(handle: u32, style: u32);
    fn os_gui_set_object_label(handle: u32, ptr: *const u8, len: u32);
    fn os_gui_set_object_interaction(handle: u32, interaction: u32);
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
const STYLE_WARNING: u32 = 15; // Mapped to Red/Flashing in Host GUI
const STYLE_SAFE: u32 = 16;    // Mapped to Green in Host GUI

const INTERACTION_HOVERED: u32 = 1; 

// -----------------------------------------------------------------------------
// Zero-Allocation Static Security State
// -----------------------------------------------------------------------------
const MAX_CONNS: usize = 8;
const MAX_APPS: usize = 8;
const MAX_STR: usize = 32;

#[derive(Copy, Clone)]
struct NetworkConnection {
    id: u32,
    ip: [u8; MAX_STR],
    ip_len: usize,
    port: u32,
    is_blocked: bool,
}

#[derive(Copy, Clone)]
struct ProcessPrivilege {
    id: u32,
    name: [u8; MAX_STR],
    name_len: usize,
    has_kernel_access: bool,
}

impl NetworkConnection {
    const fn empty() -> Self { Self { id: 0, ip: [0; MAX_STR], ip_len: 0, port: 0, is_blocked: false } }
}
impl ProcessPrivilege {
    const fn empty() -> Self { Self { id: 0, name: [0; MAX_STR], name_len: 0, has_kernel_access: false } }
}

static mut CONNECTIONS: [NetworkConnection; MAX_CONNS] = [NetworkConnection::empty(); MAX_CONNS];
static mut PROCESSES: [ProcessPrivilege; MAX_APPS] = [ProcessPrivilege::empty(); MAX_APPS];

static mut LOCKDOWN_MODE: bool = false;
static mut NEXT_GUI_ID: u32 = 500;

unsafe fn init_firewall() {
    // Mock Connections
    for (idx, (id, ip, port)) in [(1, "142.250.190.46", 443), (2, "192.168.1.100", 8080), (3, "10.0.0.5", 22)].iter().enumerate() {
        CONNECTIONS[idx].id = *id;
        CONNECTIONS[idx].ip[..ip.len()].copy_from_slice(ip.as_bytes());
        CONNECTIONS[idx].ip_len = ip.len();
        CONNECTIONS[idx].port = *port;
        CONNECTIONS[idx].is_blocked = false;
    }

    // Mock Processes
    for (idx, (id, name, kernel)) in [(1, "Nexus App Store", true), (2, "VFS Explorer", true), (3, "Doom Wasm", false)].iter().enumerate() {
        PROCESSES[idx].id = *id;
        PROCESSES[idx].name[..name.len()].copy_from_slice(name.as_bytes());
        PROCESSES[idx].name_len = name.len();
        PROCESSES[idx].has_kernel_access = *kernel;
    }
}

// -----------------------------------------------------------------------------
// V3.0 Zero-Allocation VDOM Parser
// -----------------------------------------------------------------------------
#[derive(Copy, Clone)]
pub struct Node<'a> {
    pub tag: &'a str,
    pub text: &'a str,
    pub class_attr: &'a str,
    pub child_indices: [usize; 32],
    pub child_count: usize,
}

impl<'a> Node<'a> {
    const fn empty() -> Self {
        Self { tag: "", text: "", class_attr: "", child_indices: [0; 32], child_count: 0 }
    }
}

pub struct DomTree<'a> {
    pub pool: [Node<'a>; 256],
    pub count: usize,
    pub root_idx: usize,
}

impl<'a> DomTree<'a> {
    fn new() -> Self { Self { pool: [Node::empty(); 256], count: 0, root_idx: 0 } }
    fn alloc_node(&mut self, tag: &'a str) -> usize {
        if self.count >= 256 { return 0; }
        let idx = self.count;
        self.pool[idx].tag = tag;
        self.count += 1;
        idx
    }
    fn add_child(&mut self, parent: usize, child: usize) {
        let p = &mut self.pool[parent];
        if p.child_count < 32 {
            p.child_indices[p.child_count] = child;
            p.child_count += 1;
        }
    }
}

struct HtmlParser<'a> { html: &'a str, pos: usize }
impl<'a> HtmlParser<'a> {
    fn new(html: &'a str) -> Self { Self { html, pos: 0 } }
    fn peek(&self) -> Option<u8> { if self.pos < self.html.len() { Some(self.html.as_bytes()[self.pos]) } else { None } }
    fn advance(&mut self) -> Option<u8> { let c = self.peek()?; self.pos += 1; Some(c) }
    
    fn parse_tag(&mut self) -> &'a str {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == b'>' { let tag = &self.html[start..self.pos]; self.advance(); return tag; }
            self.advance();
        }
        &self.html[start..self.pos]
    }
}

fn extract_attr<'a>(tag_content: &'a str, target_key: &str) -> &'a str {
    let mut parts = tag_content.split(target_key);
    parts.next(); 
    if let Some(rest) = parts.next() {
        let trimmed = rest.trim_start();
        if trimmed.starts_with('=') {
            let mut val_parts = trimmed[1..].trim_start().split(|c| c == '\'' || c == '"' || c == ' ' || c == '>');
            val_parts.next(); 
            if let Some(val) = val_parts.next() { return val; }
        }
    }
    ""
}

fn parse_html_to_dom<'a>(html: &'a str) -> DomTree<'a> {
    let mut parser = HtmlParser::new(html);
    let mut tree = DomTree::new();
    let root_idx = tree.alloc_node("document");
    tree.root_idx = root_idx;
    
    let mut stack: [usize; 32] = [0; 32];
    let mut stack_ptr = 0;
    stack[stack_ptr] = root_idx;

    let mut current_text_start = 0;
    let mut capturing_text = false;

    while let Some(c) = parser.advance() {
        if c == b'<' {
            if capturing_text {
                let text = &html[current_text_start..parser.pos - 1];
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let text_node_idx = tree.alloc_node("text");
                    tree.pool[text_node_idx].text = trimmed;
                    tree.add_child(stack[stack_ptr], text_node_idx);
                }
                capturing_text = false;
            }

            let tag_content = parser.parse_tag();
            if tag_content.starts_with("!--") { continue; }
            
            let mut tag_parts = tag_content.split_whitespace();
            let mut name = tag_parts.next().unwrap_or("").trim_start_matches('/');
            if name.ends_with('/') { name = name.trim_end_matches('/'); }
            
            let is_closing = tag_content.starts_with('/');
            let is_void = matches!(name.to_ascii_lowercase().as_str(), "br" | "hr" | "img" | "input" | "link" | "meta" | "!doctype");
            
            if !is_closing {
                let class_attr = extract_attr(tag_content, "class");
                
                let node_idx = tree.alloc_node(name);
                tree.pool[node_idx].class_attr = class_attr;
                
                tree.add_child(stack[stack_ptr], node_idx);
                
                if !is_void && stack_ptr < 31 {
                    stack_ptr += 1;
                    stack[stack_ptr] = node_idx;
                }
            } else {
                if stack_ptr > 0 { stack_ptr -= 1; }
            }
        } else {
            if !capturing_text {
                current_text_start = parser.pos - 1;
                capturing_text = true;
            }
        }
    }
    tree
}

// -----------------------------------------------------------------------------
// UI Layout Engine (Security Dashboard)
// -----------------------------------------------------------------------------
struct LayoutContext {
    current_y: u32,
    margin_x: u32,
    content_width: u32,
    inline_x: u32,
}

unsafe fn layout_dom_node(tree: &DomTree, node_idx: usize, ctx: &mut LayoutContext, win_x: u32, depth: u32) {
    let node = &tree.pool[node_idx];
    
    let mut tag_buf = [0u8; 8];
    let tag_len = core::cmp::min(node.tag.len(), 8);
    tag_buf[..tag_len].copy_from_slice(&node.tag.as_bytes()[..tag_len]);
    for b in tag_buf.iter_mut() { *b = b.to_ascii_lowercase(); }
    let tag_lower = core::str::from_utf8_unchecked(&tag_buf[..tag_len]);

    let mut style = STYLE_PLAIN;
    let mut height = 24;
    let mut indent = depth * 15;
    
    // Panel Layout Overrides
    if node.class_attr == "pane-header" { ctx.margin_x = 20; ctx.content_width = 980; ctx.current_y = 50; ctx.inline_x = 0; }
    if node.class_attr == "pane-left" { ctx.margin_x = 20; ctx.content_width = 470; ctx.current_y = 150; ctx.inline_x = 0; }
    if node.class_attr == "pane-right" { ctx.margin_x = 510; ctx.content_width = 470; ctx.current_y = 150; ctx.inline_x = 0; }

    match tag_lower {
        "h1" => { style = STYLE_ACCENT; height = 40; indent = 0; }
        "h2" | "h3" => { style = STYLE_ACCENT; height = 30; indent = 0; }
        "button" => { style = STYLE_SIDEBAR; height = 30; }
        "span" => { height = 20; indent = 0; }
        _ => {}
    }

    if node.class_attr.contains("os-chrome") { style = STYLE_CHROME; }
    else if node.class_attr.contains("os-sidebar") { style = STYLE_SIDEBAR; }
    else if node.class_attr.contains("os-accent") { style = STYLE_ACCENT; }
    else if node.class_attr.contains("os-warning") { style = STYLE_WARNING; }
    else if node.class_attr.contains("os-safe") { style = STYLE_SAFE; }

    let is_inline = tag_lower == "span" || tag_lower == "text";

    if !node.text.is_empty() {
        let obj_id = NEXT_GUI_ID; NEXT_GUI_ID += 1;
        let mut final_x = win_x + ctx.margin_x + indent;
        if is_inline { final_x += ctx.inline_x; }

        let text_w = core::cmp::max(20, (node.text.len() as u32) * 8);
        
        os_gui_set_object_bounds(obj_id, final_x, ctx.current_y, text_w, height);
        os_gui_set_object_style(obj_id, style);
        os_gui_set_object_label(obj_id, node.text.as_ptr(), node.text.len() as u32);
        
        if is_inline {
            ctx.inline_x += text_w;
        } else {
            ctx.current_y += height + 8;
            ctx.inline_x = 0;
        }
    } else if tag_lower == "br" {
        ctx.current_y += height + 8;
        ctx.inline_x = 0;
    }

    for i in 0..node.child_count {
        layout_dom_node(tree, node.child_indices[i], ctx, win_x, depth + 1);
    }
}

// Generate the HTML based on Security State dynamically
unsafe fn generate_ui_html() -> String {
    let mut html = String::new();
    
    // HEADER
    html.push_str("<div class='pane-header'>");
    if LOCKDOWN_MODE {
        html.push_str("<h1 class='os-warning'>[!] GLOBAL LOCKDOWN ENGAGED [!]</h1>");
        html.push_str("<p>All non-essential processes suspended. Network dropped.</p>");
    } else {
        html.push_str("<h1 class='os-safe'>OpenRhiza Security Dashboard</h1>");
        html.push_str("<p>Threat Level: <span class='os-safe'>Nominal</span></p>");
    }
    html.push_str("</div>");
    
    // LEFT PANE (Network)
    html.push_str("<div class='pane-left'>");
    html.push_str("<h2>Network Sockets</h2><ul>");
    for i in 0..MAX_CONNS {
        if CONNECTIONS[i].id != 0 {
            let ip = core::str::from_utf8_unchecked(&CONNECTIONS[i].ip[..CONNECTIONS[i].ip_len]);
            let (status, css) = if CONNECTIONS[i].is_blocked || LOCKDOWN_MODE { ("BLOCKED", "os-warning") } else { ("ACTIVE", "os-safe") };
            html.push_str(&alloc::format!("<li>[<span class='{}'>{}</span>] {}:{}</li>", css, status, ip, CONNECTIONS[i].port));
        }
    }
    html.push_str("</ul></div>");

    // RIGHT PANE (Privileges)
    html.push_str("<div class='pane-right'>");
    html.push_str("<h2>Sandbox Privileges</h2><ul>");
    for i in 0..MAX_APPS {
        if PROCESSES[i].id != 0 {
            let name = core::str::from_utf8_unchecked(&PROCESSES[i].name[..PROCESSES[i].name_len]);
            let (status, css) = if LOCKDOWN_MODE { ("SUSPENDED", "os-warning") } else if PROCESSES[i].has_kernel_access { ("KERNEL", "os-accent") } else { ("USER", "os-safe") };
            html.push_str(&alloc::format!("<li>[<span class='{}'>{}</span>] {}</li>", css, status, name));
        }
    }
    html.push_str("</ul></div>");

    html
}

unsafe fn render_dashboard() {
    ALLOCATOR.reset();
    NEXT_GUI_ID = 500;
    
    os_gui_set_object_bounds(10, 0, 0, 1024, 768);
    // Background flashes Red in lockdown!
    os_gui_set_object_style(10, if LOCKDOWN_MODE { STYLE_WARNING } else { STYLE_CHROME });
    os_gui_set_object_label(10, b"".as_ptr(), 0);

    let html_str = generate_ui_html();
    let tree = parse_html_to_dom(&html_str);
    
    let mut ctx = LayoutContext {
        current_y: 20,
        margin_x: 20,
        content_width: 984,
        inline_x: 0,
    };
    
    layout_dom_node(&tree, tree.root_idx, &mut ctx, 0, 0);
}

// -----------------------------------------------------------------------------
// Agent Security Automation APIs (The Immune System)
// -----------------------------------------------------------------------------
#[no_mangle]
pub extern "C" fn agent_cmd_block_ip(ip_ptr: *const u8, ip_len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(ip_ptr, ip_len as usize);
        if let Ok(ip_str) = core::str::from_utf8(slice) {
            log(&alloc::format!("[Firewall] Blocking IP: {}", ip_str));
            for i in 0..MAX_CONNS {
                if CONNECTIONS[i].id != 0 {
                    let conn_ip = core::str::from_utf8_unchecked(&CONNECTIONS[i].ip[..CONNECTIONS[i].ip_len]);
                    if conn_ip == ip_str {
                        CONNECTIONS[i].is_blocked = true;
                    }
                }
            }
            render_dashboard();
            return 1;
        }
        0
    }
}

#[no_mangle]
pub extern "C" fn agent_cmd_revoke_privilege(app_id: u32) -> u32 {
    unsafe {
        for i in 0..MAX_APPS {
            if PROCESSES[i].id == app_id {
                PROCESSES[i].has_kernel_access = false;
                log(&alloc::format!("[Firewall] Revoked Kernel Access for Process {}", app_id));
                render_dashboard();
                return 1;
            }
        }
        0
    }
}

#[no_mangle]
pub extern "C" fn agent_cmd_lockdown() -> u32 {
    unsafe {
        log("[Firewall] IMMUNE SYSTEM ALERT: INITIATING GLOBAL LOCKDOWN.");
        LOCKDOWN_MODE = true;
        render_dashboard();
        1
    }
}

#[no_mangle]
pub extern "C" fn agent_eval_js(script_ptr: *const u8, script_len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(script_ptr, script_len as usize);
        if let Ok(script) = core::str::from_utf8(slice) {
            let s = script.trim();
            if s.starts_with("block_ip(") {
                let inner = s[9..].trim_end_matches(|c| c == ')' || c == ';' || c == '\'' || c == '"');
                agent_cmd_block_ip(inner.as_ptr(), inner.len() as u32);
            } else if s.starts_with("revoke(") {
                let inner = s[7..].trim_end_matches(|c| c == ')' || c == ';');
                if let Ok(id) = inner.parse::<u32>() {
                    agent_cmd_revoke_privilege(id);
                }
            } else if s.starts_with("lockdown()") {
                agent_cmd_lockdown();
            }
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log("[Firewall] Zero-Trust Security Module Initialized.");
    unsafe { init_firewall(); }
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log("[Firewall] Booting Security Dashboard GUI...");
    unsafe { 
        os_request_display_mode(2, 0, 0, 1024, 768); 
        render_dashboard();
    }
    1
}
