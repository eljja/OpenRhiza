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
// Bump Allocator (2MB static heap for Wasm)
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
// Wasm Host ABI & Display ABI
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
        let is_interactive = tag.eq_ignore_ascii_case("a") || tag.eq_ignore_ascii_case("button");
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
static mut SYSTEM_LOGS: Vec<String> = Vec::new();

unsafe fn find_node_mut<'a>(root: &'a mut Node, target_id: u32) -> Option<&'a mut Node> {
    if root.id == target_id { return Some(root); }
    for child in &mut root.children {
        if let Some(found) = find_node_mut(child, target_id) { return Some(found); }
    }
    None
}

// -----------------------------------------------------------------------------
// Multi-Hardware Topology & Process State
// -----------------------------------------------------------------------------
struct Core { id: u32, load: u32 }
struct Gpu { id: u32, name: String, mem_used: u32, mem_total: u32 }
struct Nic { id: u32, name: String, tx_mbps: u32, rx_mbps: u32 }

struct Process {
    pid: u32,
    name: String,
    mem_kb: u32,
    affinity_core: Option<u32>,
    affinity_gpu: Option<u32>,
    affinity_nic: Option<u32>,
    alive: bool,
}

static mut CORES: Option<Vec<Core>> = None;
static mut GPUS: Option<Vec<Gpu>> = None;
static mut NICS: Option<Vec<Nic>> = None;
static mut PROCESS_LIST: Option<Vec<Process>> = None;

unsafe fn init_hardware_topology() {
    // Simulate 64 Cores (Server-grade layout)
    let mut cores = Vec::new();
    for i in 0..64 { cores.push(Core { id: i, load: (i * 7 + 13) % 100 }); }
    CORES = Some(cores);

    // Simulate Multi-GPU
    let mut gpus = Vec::new();
    gpus.push(Gpu { id: 0, name: String::from("NVIDIA RTX 4090"), mem_used: 4096, mem_total: 24576 });
    gpus.push(Gpu { id: 1, name: String::from("AMD Instinct MI300"), mem_used: 1024, mem_total: 196608 });
    GPUS = Some(gpus);

    // Simulate Multi-LAN
    let mut nics = Vec::new();
    nics.push(Nic { id: 0, name: String::from("e1000e (WAN)"), tx_mbps: 125, rx_mbps: 450 });
    nics.push(Nic { id: 1, name: String::from("Mellanox ConnectX-6 (LAN)"), tx_mbps: 8400, rx_mbps: 9200 });
    NICS = Some(nics);

    // Assign processes to specific hardware
    let mut procs = Vec::new();
    procs.push(Process { pid: 1, name: String::from("os_kernel_rhiza"), mem_kb: 450, affinity_core: Some(0), affinity_gpu: None, affinity_nic: None, alive: true });
    procs.push(Process { pid: 2, name: String::from("gui_compositor"), mem_kb: 10240, affinity_core: Some(1), affinity_gpu: Some(0), affinity_nic: None, alive: true });
    procs.push(Process { pid: 3, name: String::from("llm_inference_engine"), mem_kb: 80500, affinity_core: Some(42), affinity_gpu: Some(1), affinity_nic: None, alive: true });
    procs.push(Process { pid: 10, name: String::from("web_browser_seed"), mem_kb: 2048, affinity_core: Some(12), affinity_gpu: None, affinity_nic: Some(0), alive: true });
    procs.push(Process { pid: 15, name: String::from("storage_host_nvme"), mem_kb: 500, affinity_core: Some(8), affinity_gpu: None, affinity_nic: None, alive: true });
    procs.push(Process { pid: 99, name: String::from("rogue_ai_agent_module"), mem_kb: 9999, affinity_core: Some(63), affinity_gpu: Some(1), affinity_nic: Some(1), alive: true });
    PROCESS_LIST = Some(procs);
}

// -----------------------------------------------------------------------------
// System Monitor Commands (Agent API)
// -----------------------------------------------------------------------------
unsafe fn append_sys_log(msg: &str) {
    SYSTEM_LOGS.push(String::from(msg));
    log(&alloc::format!("[TaskMgr] {}", msg));
    render_dashboard();
}

#[no_mangle]
pub extern "C" fn agent_cmd_kill_process(pid: u32) -> u32 {
    unsafe {
        append_sys_log(&alloc::format!("Attempting to KILL PID {}...", pid));
        if pid == 1 {
            append_sys_log("Access Denied: Cannot kill OS Kernel Root (Rhiza).");
            return 0;
        }
        
        let mut found = false;
        if let Some(ref mut procs) = PROCESS_LIST {
            for p in procs.iter_mut() {
                if p.pid == pid && p.alive {
                    p.alive = false;
                    p.mem_kb = 0;
                    // Free hardware affinities
                    if let Some(c) = p.affinity_core {
                        if let Some(ref mut cores) = CORES { cores[c as usize].load = 0; }
                    }
                    p.affinity_core = None;
                    p.affinity_gpu = None;
                    p.affinity_nic = None;
                    
                    found = true;
                    append_sys_log(&alloc::format!("SUCCESS: Process '{}' terminated. Hardware freed.", p.name));
                }
            }
        }
        
        if !found {
            append_sys_log(&alloc::format!("Error: PID {} not found or already dead.", pid));
            return 0;
        }
        return 1;
    }
}

#[no_mangle]
pub extern "C" fn agent_cmd_free_memory() -> u32 {
    unsafe {
        append_sys_log(">>> Sweeping orphaned Wasm memory pages...");
        append_sys_log("Recovered 4096 KB of memory across GPUs and RAM.");
    }
    1
}

// -----------------------------------------------------------------------------
// HTML to DOM Parser
// -----------------------------------------------------------------------------
struct HtmlParser<'a> { html: &'a str, pos: usize }
impl<'a> HtmlParser<'a> {
    fn new(html: &'a str) -> Self { Self { html, pos: 0 } }
    fn peek(&self) -> Option<char> { self.html[self.pos..].chars().next() }
    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?; self.pos += c.len_utf8(); Some(c)
    }
    fn parse_tag(&mut self) -> &'a str {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == '>' { let tag = &self.html[start..self.pos]; self.advance(); return tag; }
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
// Recursive GUI Layout Engine (Multi-Hardware Aware)
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

    let mut component_id = "";
    for (k, v) in &node.attributes {
        if k.eq_ignore_ascii_case("id") { component_id = v.as_str(); }
    }

    if component_id == "topology_dashboard" {
        // Render 64 Cores in a Grid (Flex Wrap)
        let core_w = 85;
        let mut cx = ctx.margin_x;
        if let Some(ref cores) = CORES {
            for c in cores {
                if cx + core_w > ctx.content_width - ctx.margin_x {
                    cx = ctx.margin_x;
                    ctx.current_y += 28;
                }
                let cid = NEXT_NODE_ID; NEXT_NODE_ID += 1;
                os_gui_set_object_bounds(cid, cx, ctx.current_y, core_w, 24);
                let mut s = STYLE_PLAIN;
                if c.load > 80 { s = STYLE_ACCENT; }
                os_gui_set_object_style(cid, s);
                let label = alloc::format!("C{:02}: {:02}%", c.id, c.load);
                os_gui_set_object_label(cid, label.as_ptr(), label.len() as u32);
                cx += core_w + 5;
            }
        }
        ctx.current_y += 30;

        // Render Multi-GPU and Multi-NIC
        if let Some(ref gpus) = GPUS {
            for g in gpus {
                let gid = NEXT_NODE_ID; NEXT_NODE_ID += 1;
                os_gui_set_object_bounds(gid, ctx.margin_x, ctx.current_y, ctx.content_width, 24);
                os_gui_set_object_style(gid, STYLE_SIDEBAR);
                let label = alloc::format!("[GPU {}] {} | VRAM: {}/{} MB", g.id, g.name, g.mem_used, g.mem_total);
                os_gui_set_object_label(gid, label.as_ptr(), label.len() as u32);
                ctx.current_y += 28;
            }
        }
        if let Some(ref nics) = NICS {
            for n in nics {
                let nid = NEXT_NODE_ID; NEXT_NODE_ID += 1;
                os_gui_set_object_bounds(nid, ctx.margin_x, ctx.current_y, ctx.content_width, 24);
                os_gui_set_object_style(nid, STYLE_SIDEBAR);
                let label = alloc::format!("[LAN {}] {} | TX: {} Mbps | RX: {} Mbps", n.id, n.name, n.tx_mbps, n.rx_mbps);
                os_gui_set_object_label(nid, label.as_ptr(), label.len() as u32);
                ctx.current_y += 28;
            }
        }
        ctx.current_y += 10;
        return;
    }

    if component_id == "process_table" {
        if let Some(ref procs) = PROCESS_LIST {
            // Header
            let hid = NEXT_NODE_ID; NEXT_NODE_ID += 1;
            os_gui_set_object_bounds(hid, ctx.margin_x, ctx.current_y, ctx.content_width, 24);
            os_gui_set_object_style(hid, STYLE_CHROME);
            let header = alloc::format!("PID | PROCESS NAME              | CPU  | GPU | LAN | MEMORY   | STATUS");
            os_gui_set_object_label(hid, header.as_ptr(), header.len() as u32);
            ctx.current_y += 28;

            for p in procs {
                let status = if p.alive { "RUNNING" } else { "KILLED" };
                let core = if let Some(c) = p.affinity_core { alloc::format!("{:02}", c) } else { String::from("--") };
                let gpu = if let Some(g) = p.affinity_gpu { alloc::format!("{:02}", g) } else { String::from("--") };
                let lan = if let Some(l) = p.affinity_nic { alloc::format!("{:02}", l) } else { String::from("--") };
                
                let line = alloc::format!("{:03} | {:25} | C:{} | G:{} | L:{} | {:7} KB | [{}]", p.pid, p.name, core, gpu, lan, p.mem_kb, status);
                
                let p_id = NEXT_NODE_ID; NEXT_NODE_ID += 1;
                let mut p_style = STYLE_PLAIN;
                if !p.alive { p_style = STYLE_SIDEBAR; } 
                else if p.name.contains("rogue") { p_style = STYLE_ACCENT; } 
                
                os_gui_set_object_bounds(p_id, ctx.margin_x, ctx.current_y, ctx.content_width, 24);
                os_gui_set_object_style(p_id, p_style);
                os_gui_set_object_label(p_id, line.as_ptr(), line.len() as u32);
                ctx.current_y += 28;
            }
        }
        return; 
    }

    if component_id == "log_output" {
        os_gui_set_object_bounds(node.id, ctx.margin_x, ctx.current_y, ctx.content_width, 150);
        os_gui_set_object_style(node.id, STYLE_CHROME);
        let header = alloc::format!("--- LLM AGENT / SYSTEM LOGS ---");
        os_gui_set_object_label(node.id, header.as_ptr(), header.len() as u32);
        ctx.current_y += 30;

        for log_line in &SYSTEM_LOGS {
            let log_id = NEXT_NODE_ID; NEXT_NODE_ID += 1;
            os_gui_set_object_bounds(log_id, ctx.margin_x + 10, ctx.current_y, ctx.content_width - 20, 20);
            os_gui_set_object_style(log_id, STYLE_PLAIN);
            os_gui_set_object_label(log_id, log_line.as_ptr(), log_line.len() as u32);
            ctx.current_y += 24;
        }
        ctx.current_y += 10;
        return; 
    }

    if !text.is_empty() || !node.attributes.is_empty() {
        let display_text = if tag == "li" { alloc::format!("* {}", text) } else { text.to_string() };
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
                <h1 class='os-accent'>OpenRhiza Multi-Node Task Manager</h1>\n\
                <br/>\n\
                <h3>Hardware Topology View</h3>\n\
                <div id='topology_dashboard'></div>\n\
                <br/>\n\
                <h3>Global Process List (Hardware Affinities)</h3>\n\
                <div id='process_table'></div>\n\
                <br/>\n\
                <div>\n\
                    <h3>Agent Quick Actions</h3>\n\
                    <ul>\n\
                        <li><button class='os-sidebar'>Kill Rogue LLM Agent (PID 99)</button></li>\n\
                        <li><button class='os-accent'>Sweep Memory & Reclaim GPUs</button></li>\n\
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
        let title = b"System Monitor (Multi-Core/GPU/LAN) VFS Window";
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
        let mut clicked_text = String::new();
        if let Some(ref mut root) = DOM_ROOT {
            if let Some(node) = find_node_mut(root, id) {
                clicked_text = node.text.clone();
            }
        }
        
        if clicked_text.contains("Kill Rogue") {
            agent_cmd_kill_process(99);
        } else if clicked_text.contains("Sweep Memory") {
            agent_cmd_free_memory();
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn agent_eval_js(script_ptr: *const u8, script_len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(script_ptr, script_len as usize);
        if let Ok(script) = core::str::from_utf8(slice) {
            log(&alloc::format!("[JS Engine] System script: {}", script));
            let s = script.trim();
            if s.starts_with("kill(") {
                let inner = s[5..].trim_end_matches(|c| c == ')' || c == ';');
                if let Ok(pid) = inner.parse::<u32>() {
                    agent_cmd_kill_process(pid);
                }
            } else if s.starts_with("free()") {
                agent_cmd_free_memory();
            } else {
                log("[JS Engine] Unknown system command.");
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
    log("[TaskMgr] Multi-Node System Monitor Initialized.");
    unsafe { init_hardware_topology(); }
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log("[TaskMgr] Scanning Hardware Topologies...");
    unsafe { 
        os_request_display_mode(2, 0, 0, 1024, 768); 
        SYSTEM_LOGS.push(String::from("System Boot. Server Grid: 64 Cores, 2 GPUs, 2 NICs detected."));
        SYSTEM_LOGS.push(String::from("Awaiting LLM Agent resource control..."));
        render_dashboard();
    }
    1
}
