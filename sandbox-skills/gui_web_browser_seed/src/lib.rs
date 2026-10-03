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

impl BumpAllocator {
    pub unsafe fn reset(&self) {
        *self.next.get() = 0;
    }
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
const INTERACTION_ACTIVE: u32 = 3;

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

unsafe fn find_node_mut<'a>(root: &'a mut Node, target_id: u32) -> Option<&'a mut Node> {
    if root.id == target_id { return Some(root); }
    for child in &mut root.children {
        if let Some(found) = find_node_mut(child, target_id) { return Some(found); }
    }
    None
}

// -----------------------------------------------------------------------------
// Form Serialization Engine
// -----------------------------------------------------------------------------
fn collect_inputs(node: &Node, payload: &mut String) {
    if node.tag.eq_ignore_ascii_case("input") {
        let mut id_attr = alloc::format!("input_{}", node.id);
        for (k, v) in &node.attributes {
            if k.eq_ignore_ascii_case("id") || k.eq_ignore_ascii_case("name") {
                id_attr = v.clone();
            }
        }
        let value = &node.text;
        payload.push_str(&alloc::format!("\"{}\": \"{}\", ", id_attr, value));
    }
    for child in &node.children {
        collect_inputs(child, payload);
    }
}

unsafe fn serialize_form() -> String {
    let mut payload = String::from("{ ");
    if let Some(ref root) = DOM_ROOT {
        collect_inputs(root, &mut payload);
    }
    if payload.ends_with(", ") {
        payload.truncate(payload.len() - 2);
    }
    payload.push_str(" }");
    payload
}

// -----------------------------------------------------------------------------
// Agent Automation & Form APIs
// -----------------------------------------------------------------------------
#[no_mangle]
pub extern "C" fn browser_click_element(id: u32) -> u32 {
    unsafe {
        log(&alloc::format!("[Browser Engine] CLICK event on DOM ID: {}", id));
        os_gui_set_object_interaction(id, INTERACTION_ACTIVE);
        
        let mut navigate_to = String::new();
        
        if let Some(ref mut root) = DOM_ROOT {
            if let Some(node) = find_node_mut(root, id) {
                if node.tag.eq_ignore_ascii_case("button") {
                    
                    let mut is_submit = false;
                    for (k, v) in &node.attributes {
                        if (k.eq_ignore_ascii_case("type") && v.eq_ignore_ascii_case("submit")) || 
                           (k.eq_ignore_ascii_case("class") && v.contains("submit")) {
                            is_submit = true;
                        }
                    }
                    
                    if is_submit {
                        node.text = String::from("Submitting...");
                        let payload = serialize_form();
                        log("==================================================");
                        log(&alloc::format!(">>> [Network Request] POST Form Data: {}", payload));
                        log("==================================================");
                    } else {
                        node.text = String::from("Clicked!");
                    }
                    render_dom_to_screen(); 
                } else if node.tag.eq_ignore_ascii_case("a") {
                    for (k, v) in &node.attributes {
                        if k.eq_ignore_ascii_case("href") { navigate_to = v.clone(); }
                    }
                }
            }
        }
        
        if !navigate_to.is_empty() {
            log(&alloc::format!("[Browser Engine] Navigation via <a> tag: {}", navigate_to));
            browser_navigate(navigate_to.as_ptr(), navigate_to.len() as u32);
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn browser_type_text(id: u32, text_ptr: *const u8, text_len: u32) -> u32 {
    unsafe {
        let text_slice = core::slice::from_raw_parts(text_ptr, text_len as usize);
        if let Ok(text_str) = core::str::from_utf8(text_slice) {
            log(&alloc::format!("[Browser Engine] TYPING into Input ID {}: '{}'", id, text_str));
            
            if let Some(ref mut root) = DOM_ROOT {
                if let Some(node) = find_node_mut(root, id) {
                    if node.tag.eq_ignore_ascii_case("input") {
                        node.text = alloc::string::String::from(text_str);
                        render_dom_to_screen(); 
                        return 1;
                    }
                }
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn browser_dump_dom() -> u32 {
    log("=== [Agent Interface] Real-time DOM State Dump ===");
    unsafe {
        if let Some(ref root) = DOM_ROOT {
            print_dom_tree(root, "");
        } else {
            log("  (DOM is empty)");
        }
    }
    log("==================================================");
    1
}

#[no_mangle]
pub extern "C" fn browser_navigate(url_ptr: *const u8, url_len: u32) -> u32 {
    unsafe {
        let url_slice = core::slice::from_raw_parts(url_ptr, url_len as usize);
        if let Ok(url_str) = core::str::from_utf8(url_slice) {
            log(&alloc::format!("========================================="));
            log(&alloc::format!("[Browser Engine] HTTP GET -> {}", url_str));
            log(&alloc::format!("========================================="));
            
            DOM_ROOT = None; 
            ALLOCATOR.reset();
            NEXT_NODE_ID = 500;
            
            let html = match url_str {
                "/login" => "<html><body><h1 class='os-accent'>Nexus Auth System</h1><p>Agent, please inject payload into the form.</p><input type='text' id='username' placeholder='Username' /><br/><input type='text' id='password' placeholder='Password' /><br/><button class='os-sidebar submit' type='submit'>Secure Login</button><br/><br/><a href='/'>Go Back</a></body></html>",
                "/post/1" => "<html><body><h1>Wasm Drivers are amazing</h1><p>We hot-swapped e1000.</p><a href='/'>Home</a></body></html>",
                _ => "<html><body><h1>OpenRhiza Hacker News</h1><p>Top Stories today.</p><h2>1. Show HN: V7.0 Micro-JS & Form Engine</h2><p>Agents can now execute JS inside Wasm! <a href='/post/1' class='link'>Read Comments</a></p><div><h3>Action Panel</h3><ul><li><a href='/login'>Login to Nexus</a></li></ul></div></body></html>"
            };
            
            parse_html_to_dom(html);
            render_dom_to_screen();
            
            return 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn browser_reset() -> u32 {
    unsafe {
        DOM_ROOT = None; 
        ALLOCATOR.reset();
        NEXT_NODE_ID = 500;
        log("[Browser Engine] Browser memory securely wiped.");
    }
    1
}

// -----------------------------------------------------------------------------
// Micro-JavaScript Evaluator (Agent DOM Command Injector)
// -----------------------------------------------------------------------------
#[no_mangle]
pub extern "C" fn browser_eval_js(script_ptr: *const u8, script_len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(script_ptr, script_len as usize);
        if let Ok(script) = core::str::from_utf8(slice) {
            log(&alloc::format!("[JS Engine] Executing Micro-JS: {}", script));
            
            let s = script.trim();
            if s.starts_with("click(") {
                let id_str = s[6..].trim_end_matches(|c| c == ')' || c == ';');
                if let Ok(id) = id_str.parse::<u32>() {
                    browser_click_element(id);
                }
            } else if s.starts_with("set_text(") {
                let inner = s[9..].trim_end_matches(|c| c == ')' || c == ';');
                if let Some(comma_idx) = inner.find(',') {
                    let id_str = inner[0..comma_idx].trim();
                    let text_str = inner[comma_idx+1..].trim().trim_matches(|c| c == '"' || c == '\'');
                    if let Ok(id) = id_str.parse::<u32>() {
                        browser_type_text(id, text_str.as_ptr(), text_str.len() as u32);
                    }
                }
            } else if s.starts_with("navigate(") {
                let inner = s[9..].trim_end_matches(|c| c == ')' || c == ';').trim_matches(|c| c == '"' || c == '\'');
                browser_navigate(inner.as_ptr(), inner.len() as u32);
            } else if s.starts_with("dump(") {
                browser_dump_dom();
            } else {
                log("[JS Engine] Error: Unknown JS stub command. Supported: click(id), set_text(id, text), navigate(url), dump()");
                return 0;
            }
        }
    }
    1
}

// -----------------------------------------------------------------------------
// HTML to DOM Parser
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
    fn skip_until(&mut self, target: &str) {
        if let Some(idx) = self.html[self.pos..].find(target) {
            self.pos += idx + target.len();
        } else {
            self.pos = self.html.len();
        }
    }
    fn parse_tag(&mut self) -> &'a str {
        let start = self.pos;
        if self.html[self.pos..].starts_with("!--") {
            if let Some(idx) = self.html[self.pos..].find("-->") {
                self.pos += idx + 3;
                return &self.html[start..self.pos-1]; 
            }
        }
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
    
    while let Some(c) = chars.peek() {
        if c.is_whitespace() { break; }
        chars.next();
    }
    
    while let Some(c) = chars.next() {
        if c.is_whitespace() { continue; }
        let mut key = String::new();
        key.push(c);
        while let Some(&k) = chars.peek() {
            if k == '=' || k.is_whitespace() { break; }
            key.push(chars.next().unwrap());
        }
        
        let mut val = String::new();
        while let Some(&n) = chars.peek() {
            if n.is_whitespace() { chars.next(); continue; }
            if n == '=' {
                chars.next(); 
                while let Some(&s) = chars.peek() {
                    if !s.is_whitespace() { break; }
                    chars.next();
                }
                
                if let Some(&quote) = chars.peek() {
                    if quote == '"' || quote == '\'' {
                        chars.next(); 
                        while let Some(v) = chars.next() {
                            if v == quote { break; }
                            val.push(v);
                        }
                    } else {
                        while let Some(v) = chars.next() {
                            if v.is_whitespace() || v == '>' || v == '/' { break; }
                            val.push(v);
                        }
                    }
                }
                break;
            } else {
                break;
            }
        }
        if !key.is_empty() { attrs.push((key, val)); }
    }
    attrs
}

fn decode_entities(text: &str) -> String {
    let t = text.replace("&amp;", "&");
    let t = t.replace("&lt;", "<");
    let t = t.replace("&gt;", ">");
    let t = t.replace("&quot;", "\"");
    t.replace("&#39;", "'")
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
                    if name.eq_ignore_ascii_case("script") {
                        parser.skip_until("</script>");
                        if stack.len() > 1 {
                            let node = stack.pop().unwrap();
                            if let Some(top) = stack.last_mut() { top.children.push(node); }
                        }
                    } else if name.eq_ignore_ascii_case("style") {
                        parser.skip_until("</style>");
                        if stack.len() > 1 {
                            let node = stack.pop().unwrap();
                            if let Some(top) = stack.last_mut() { top.children.push(node); }
                        }
                    }
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
    
    if stack.len() == 1 {
        DOM_ROOT = Some(stack.pop().unwrap());
    }
}

// -----------------------------------------------------------------------------
// Recursive GUI Layout Engine
// -----------------------------------------------------------------------------
struct LayoutContext {
    current_y: u32,
    margin_x: u32,
    content_width: u32,
}

fn print_dom_tree(node: &Node, indent: &str) {
    let mut attrs = String::new();
    for (k, v) in &node.attributes { attrs.push_str(&alloc::format!(" {}='{}'", k, v)); }
    log(&alloc::format!("{}[{}] (ID: {}){}", indent, node.tag, node.id, attrs));
    let text = decode_entities(node.text.trim());
    if !text.is_empty() { log(&alloc::format!("{}  \"{}\"", indent, text)); }
    for child in &node.children { print_dom_tree(child, &alloc::format!("{}  ", indent)); }
}

unsafe fn layout_dom_node(node: &Node, ctx: &mut LayoutContext, depth: u32) {
    let tag = node.tag.to_lowercase();
    let text = decode_entities(node.text.trim());
    
    let mut style = STYLE_PLAIN;
    let mut height = 24;
    let mut indent = depth * 15;

    match tag.as_str() {
        "h1" => { style = STYLE_ACCENT; height = 46; indent = 0; }
        "h2" | "h3" => { style = STYLE_ACCENT; height = 34; indent = 0; }
        "button" => { style = STYLE_PLAIN; height = 30; }
        "input" => { style = STYLE_CHROME; height = 30; }
        "li" => { indent += 10; }
        "a" => { style = STYLE_PLAIN; }
        _ => {}
    }

    for (k, v) in &node.attributes {
        if k.eq_ignore_ascii_case("class") {
            if v.contains("os-chrome") { style = STYLE_CHROME; }
            else if v.contains("os-sidebar") { style = STYLE_SIDEBAR; }
            else if v.contains("os-accent") { style = STYLE_ACCENT; }
        }
    }

    if !text.is_empty() || !node.attributes.is_empty() || tag == "input" {
        let mut display_text = if tag == "li" {
            alloc::format!("* {}", text)
        } else {
            text.to_string()
        };

        if tag == "a" {
            display_text = alloc::format!("[Link: {}]", text);
        } else if tag == "input" {
            let mut placeholder = "...";
            for (k, v) in &node.attributes {
                if k.eq_ignore_ascii_case("placeholder") { placeholder = v; }
            }
            display_text = if text.is_empty() { alloc::format!("[Input: {}]", placeholder) } else { alloc::format!("[Input: {}]", text) };
        }

        if !display_text.is_empty() {
            os_gui_set_object_bounds(node.id, ctx.margin_x + indent, ctx.current_y, ctx.content_width - indent, height);
            os_gui_set_object_style(node.id, style);
            os_gui_set_object_label(node.id, display_text.as_ptr(), display_text.len() as u32);
            
            if node.is_interactive {
                os_gui_set_object_interaction(node.id, INTERACTION_HOVERED);
            }

            ctx.current_y += height + 8;
        }
    }

    for child in &node.children {
        layout_dom_node(child, ctx, depth + 1);
    }
}

unsafe fn render_dom_to_screen() {
    if let Some(ref root) = DOM_ROOT {
        os_gui_set_object_bounds(100, 20, 20, 800, 40);
        os_gui_set_object_style(100, STYLE_CHROME);
        let title = b"OpenRhiza Graphic DOM Browser (v7.0 - Micro JS Engine)";
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
// Skill Entry Points
// -----------------------------------------------------------------------------
#[no_mangle]
pub extern "C" fn init_driver() {
    log("[GUI Browser] Advanced DOM-Aware GUI Wasm Skill Initialized.");
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log("[GUI Browser] Engaging V7.0 Micro-JS Environment...");
    unsafe { os_request_display_mode(2, 0, 0, 1024, 768); }
    
    let url = "/";
    browser_navigate(url.as_ptr(), url.len() as u32);

    1
}
