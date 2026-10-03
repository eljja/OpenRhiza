#![no_std]
#![no_main]

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

const INTERACTION_HOVERED: u32 = 1; 

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
static mut NEXUS_LOGS: Vec<String> = Vec::new();

unsafe fn find_node_mut<'a>(root: &'a mut Node, target_id: u32) -> Option<&'a mut Node> {
    if root.id == target_id { return Some(root); }
    for child in &mut root.children {
        if let Some(found) = find_node_mut(child, target_id) { return Some(found); }
    }
    None
}

// -----------------------------------------------------------------------------
// Driver Cloud Registry Data (Simulated)
// -----------------------------------------------------------------------------
struct DriverPackage {
    id: u32,
    name: String,
    hw_id: String,
    size_kb: u32,
    rating: String,
    installed: bool,
    dependencies: Vec<u32>,
}

static mut CLOUD_REGISTRY: Option<Vec<DriverPackage>> = None;
static mut CURRENT_SEARCH: String = String::new();

unsafe fn init_registry() {
    let mut db = Vec::new();
    db.push(DriverPackage { id: 1, name: String::from("Intel e1000 Gigabit"), hw_id: String::from("PCI 8086:100E"), size_kb: 120, rating: String::from("★★★★★ (Verified)"), installed: false, dependencies: alloc::vec![7] });
    db.push(DriverPackage { id: 2, name: String::from("Realtek RTL8139"), hw_id: String::from("PCI 10EC:8139"), size_kb: 85, rating: String::from("★★★★☆"), installed: false, dependencies: alloc::vec![7] });
    db.push(DriverPackage { id: 3, name: String::from("NVIDIA RTX 4090 Open"), hw_id: String::from("PCI 10DE:2684"), size_kb: 4500, rating: String::from("★★★★★ (Verified)"), installed: false, dependencies: alloc::vec![6] });
    db.push(DriverPackage { id: 4, name: String::from("Generic USB HID Mouse"), hw_id: String::from("USB 03F0:0141"), size_kb: 45, rating: String::from("★★★☆☆"), installed: true, dependencies: Vec::new() });
    db.push(DriverPackage { id: 5, name: String::from("NVMe Storage Host"), hw_id: String::from("PCI 144D:A80C"), size_kb: 310, rating: String::from("★★★★★"), installed: true, dependencies: Vec::new() });
    db.push(DriverPackage { id: 6, name: String::from("Vulkan API Subsystem"), hw_id: String::from("VIRTUAL_CORE_VULKAN"), size_kb: 15400, rating: String::from("★★★★★ (Core)"), installed: false, dependencies: Vec::new() });
    db.push(DriverPackage { id: 7, name: String::from("TCP/IP Network Stack"), hw_id: String::from("VIRTUAL_CORE_NET"), size_kb: 850, rating: String::from("★★★★★ (Core)"), installed: true, dependencies: Vec::new() });
    CLOUD_REGISTRY = Some(db);
}

// -----------------------------------------------------------------------------
// Agent Commands
// -----------------------------------------------------------------------------
unsafe fn append_log(msg: &str) {
    NEXUS_LOGS.push(String::from(msg));
    log(&alloc::format!("[Nexus] {}", msg));
    render_dashboard();
}

#[no_mangle]
pub extern "C" fn agent_cmd_search_driver(hw_id_ptr: *const u8, len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(hw_id_ptr, len as usize);
        if let Ok(query) = core::str::from_utf8(slice) {
            append_log(&alloc::format!("Searching Cloud Registry for: '{}'", query));
            CURRENT_SEARCH = String::from(query);
            render_dashboard();
            return 1;
        }
    }
    0
}

unsafe fn recursive_install(driver_id: u32) -> bool {
    let mut deps_to_install = Vec::new();
    let mut driver_name = String::new();
    let mut size = 0;
    
    if let Some(ref db) = CLOUD_REGISTRY {
        if let Some(d) = db.iter().find(|x| x.id == driver_id) {
            if d.installed { return true; }
            deps_to_install = d.dependencies.clone();
            driver_name = d.name.clone();
            size = d.size_kb;
        } else { return false; }
    }

    for dep_id in deps_to_install {
        if !recursive_install(dep_id) { return false; }
    }

    if let Some(ref mut db) = CLOUD_REGISTRY {
        if let Some(d) = db.iter_mut().find(|x| x.id == driver_id) {
            append_log(&alloc::format!("Downloading Wasm Package: '{}' ({} KB)...", driver_name, size));
            append_log("Verifying Cryptographic Signature...");
            append_log("Allocating Wasm Sandbox Memory...");
            append_log(&alloc::format!("Mapping {} to OS Kernel Memory @ 0x8F00A...", driver_name));
            d.installed = true;
            append_log(&alloc::format!("SUCCESS: '{}' Hot-Swapped Successfully.", driver_name));
            return true;
        }
    }
    false
}

#[no_mangle]
pub extern "C" fn agent_cmd_install_driver(driver_id: u32) -> u32 {
    unsafe {
        append_log(&alloc::format!(">>> Resolving Dependencies for Driver ID {}...", driver_id));
        if recursive_install(driver_id) {
            render_dashboard();
            return 1;
        }
        append_log(&alloc::format!("Error: Dependency resolution failed for ID {}.", driver_id));
        0
    }
}

#[no_mangle]
pub extern "C" fn agent_cmd_uninstall_driver(driver_id: u32) -> u32 {
    unsafe {
        if let Some(ref mut db) = CLOUD_REGISTRY {
            if let Some(d) = db.iter_mut().find(|x| x.id == driver_id) {
                if !d.installed {
                    append_log(&alloc::format!("Driver '{}' is not installed.", d.name));
                    return 0;
                }
                append_log(&alloc::format!(">>> Unloading Driver '{}'...", d.name));
                append_log("De-allocating Wasm Sandbox Memory...");
                append_log("Detaching from OS Kernel Bus...");
                d.installed = false;
                append_log("UNINSTALL SUCCESSFUL.");
                render_dashboard();
                return 1;
            }
        }
        append_log(&alloc::format!("Error: Driver ID {} not found.", driver_id));
        0
    }
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

    let mut component_id = "";
    for (k, v) in &node.attributes {
        if k.eq_ignore_ascii_case("id") { component_id = v.as_str(); }
    }

    if component_id == "registry_list" {
        if let Some(ref db) = CLOUD_REGISTRY {
            // Header
            let hid = NEXT_NODE_ID; NEXT_NODE_ID += 1;
            os_gui_set_object_bounds(hid, ctx.margin_x, ctx.current_y, ctx.content_width, 24);
            os_gui_set_object_style(hid, STYLE_CHROME);
            let header = alloc::format!("ID | HARDWARE ID   | DRIVER NAME                 | SIZE     | RATING      | STATUS");
            os_gui_set_object_label(hid, header.as_ptr(), header.len() as u32);
            ctx.current_y += 28;

            let query = CURRENT_SEARCH.to_lowercase();
            
            for d in db {
                if !query.is_empty() && !d.hw_id.to_lowercase().contains(&query) && !d.name.to_lowercase().contains(&query) {
                    continue; 
                }

                let status = if d.installed { "[ INSTALLED ]" } else { "[ AVAILABLE ]" };
                let line = alloc::format!("{:02} | {:13} | {:27} | {:5} KB | {:11} | {}", d.id, d.hw_id, d.name, d.size_kb, d.rating, status);
                
                let p_id = NEXT_NODE_ID; NEXT_NODE_ID += 1;
                let mut p_style = STYLE_PLAIN;
                if d.installed { p_style = STYLE_SIDEBAR; } 
                else { p_style = STYLE_ACCENT; } 
                
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
        let header = alloc::format!("--- NEXUS INSTALLATION LOGS ---");
        os_gui_set_object_label(node.id, header.as_ptr(), header.len() as u32);
        ctx.current_y += 30;

        for log_line in &NEXUS_LOGS {
            let log_id = NEXT_NODE_ID; NEXT_NODE_ID += 1;
            os_gui_set_object_bounds(log_id, ctx.margin_x + 10, ctx.current_y, ctx.content_width - 20, 20);
            os_gui_set_object_style(log_id, STYLE_PLAIN);
            os_gui_set_object_label(log_id, log_line.as_ptr(), log_line.len() as u32);
            ctx.current_y += 24;
        }
        ctx.current_y += 10;
        return; 
    }

    if !text.is_empty() || !node.attributes.is_empty() || tag == "input" {
        let mut display_text = if tag == "li" { alloc::format!("* {}", text) } else { text.to_string() };
        if tag == "input" {
            display_text = alloc::format!("[Search: {}]", CURRENT_SEARCH);
            if CURRENT_SEARCH.is_empty() { display_text = String::from("[Search HW ID...]"); }
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
                <h1 class='os-accent'>OpenRhiza Nexus App Store</h1>\n\
                <p>Cloud Driver Registry - Connects to openrhiza.com</p>\n\
                <br/>\n\
                <input type='text' id='search_bar' placeholder='Search HW ID...' />\n\
                <br/>\n\
                <h3>Available Wasm Drivers</h3>\n\
                <div id='registry_list'></div>\n\
                <br/>\n\
                <div>\n\
                    <h3>Agent Quick Actions</h3>\n\
                    <ul>\n\
                        <li><button class='os-sidebar'>Search NVIDIA (10DE:2684)</button></li>\n\
                        <li><button class='os-accent'>Install Driver ID 3 (RTX 4090)</button></li>\n\
                        <li><button class='os-sidebar'>Uninstall Driver ID 3</button></li>\n\
                        <li><button class='os-sidebar'>Clear Search</button></li>\n\
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
        let title = b"Nexus Driver Registry VFS Window";
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
        
        if clicked_text.contains("Search NVIDIA") {
            let q = "10DE:2684";
            agent_cmd_search_driver(q.as_ptr(), q.len() as u32);
        } else if clicked_text.contains("Install Driver ID 3") {
            agent_cmd_install_driver(3);
        } else if clicked_text.contains("Uninstall Driver ID 3") {
            agent_cmd_uninstall_driver(3);
        } else if clicked_text.contains("Clear Search") {
            let q = "";
            agent_cmd_search_driver(q.as_ptr(), q.len() as u32);
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn agent_eval_js(script_ptr: *const u8, script_len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(script_ptr, script_len as usize);
        if let Ok(script) = core::str::from_utf8(slice) {
            log(&alloc::format!("[JS Engine] Nexus script: {}", script));
            let s = script.trim();
            if s.starts_with("search(") {
                let inner = s[7..].trim_end_matches(|c| c == ')' || c == ';').trim_matches(|c| c == '"' || c == '\'');
                agent_cmd_search_driver(inner.as_ptr(), inner.len() as u32);
            } else if s.starts_with("install(") {
                let inner = s[8..].trim_end_matches(|c| c == ')' || c == ';');
                if let Ok(id) = inner.parse::<u32>() {
                    agent_cmd_install_driver(id);
                }
            } else if s.starts_with("uninstall(") {
                let inner = s[10..].trim_end_matches(|c| c == ')' || c == ';');
                if let Ok(id) = inner.parse::<u32>() {
                    agent_cmd_uninstall_driver(id);
                }
            } else {
                log("[JS Engine] Unknown nexus command.");
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
    log("[Nexus] Driver Nexus Client Sandbox Initialized.");
    unsafe { init_registry(); }
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log("[Nexus] Connecting to openrhiza.com cloud registry...");
    unsafe { 
        os_request_display_mode(2, 0, 0, 1024, 768); 
        NEXUS_LOGS.push(String::from("Connected to Cloud Node US-East-1."));
        NEXUS_LOGS.push(String::from("Awaiting LLM Agent Hardware Signatures..."));
        render_dashboard();
    }
    1
}
