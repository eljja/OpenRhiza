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
const STYLE_CARD_BG: u32 = 12;

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

unsafe fn find_node_mut<'a>(root: &'a mut Node, target_id: u32) -> Option<&'a mut Node> {
    if root.id == target_id { return Some(root); }
    for child in &mut root.children {
        if let Some(found) = find_node_mut(child, target_id) { return Some(found); }
    }
    None
}

// -----------------------------------------------------------------------------
// Human-Friendly App Registry
// -----------------------------------------------------------------------------
struct AppPackage {
    id: u32,
    name: String,
    publisher: String,
    category: String,
    size_mb: u32,
    rating: String,
    state: String, // "AVAILABLE", "INSTALLING", "OPEN"
}

static mut APP_STORE_DB: Option<Vec<AppPackage>> = None;

unsafe fn init_app_store() {
    let mut db = Vec::new();
    db.push(AppPackage { id: 1, name: String::from("Rhiza Studio Code"), publisher: String::from("OpenRhiza Foundation"), category: String::from("Development"), size_mb: 45, rating: String::from("★★★★★"), state: String::from("AVAILABLE") });
    db.push(AppPackage { id: 2, name: String::from("VFS Explorer"), publisher: String::from("OpenRhiza Foundation"), category: String::from("Utilities"), size_mb: 12, rating: String::from("★★★★☆"), state: String::from("AVAILABLE") });
    db.push(AppPackage { id: 3, name: String::from("Doom (Wasm Port)"), publisher: String::from("id Software (Port)"), category: String::from("Games"), size_mb: 8, rating: String::from("★★★★★"), state: String::from("AVAILABLE") });
    db.push(AppPackage { id: 4, name: String::from("Nexus Terminal Pro"), publisher: String::from("Terminal Hackers"), category: String::from("Utilities"), size_mb: 5, rating: String::from("★★★☆☆"), state: String::from("OPEN") });
    db.push(AppPackage { id: 5, name: String::from("Discord Web Client"), publisher: String::from("Discord Inc."), category: String::from("Social"), size_mb: 110, rating: String::from("★★★★☆"), state: String::from("AVAILABLE") });
    db.push(AppPackage { id: 6, name: String::from("Photoshop Express (Wasm)"), publisher: String::from("Adobe Systems"), category: String::from("Creative"), size_mb: 320, rating: String::from("★★★★★"), state: String::from("AVAILABLE") });
    APP_STORE_DB = Some(db);
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
// Recursive GUI Layout Engine (Card Grid System)
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

    if component_id == "app_grid" {
        // Render App Cards in a Flex Grid
        let card_w = 360;
        let card_h = 160;
        let mut cx = ctx.margin_x;
        let start_y = ctx.current_y;
        let mut max_y_in_row = start_y;

        if let Some(ref db) = APP_STORE_DB {
            for app in db {
                if cx + card_w > ctx.content_width {
                    cx = ctx.margin_x;
                    ctx.current_y += card_h + 20; // New row
                }
                
                // Draw Card Background Box
                let bg_id = NEXT_NODE_ID; NEXT_NODE_ID += 1;
                os_gui_set_object_bounds(bg_id, cx, ctx.current_y, card_w, card_h);
                os_gui_set_object_style(bg_id, STYLE_CHROME); // Background style
                os_gui_set_object_label(bg_id, b"".as_ptr(), 0);

                // App Title
                let title_id = NEXT_NODE_ID; NEXT_NODE_ID += 1;
                os_gui_set_object_bounds(title_id, cx + 15, ctx.current_y + 15, card_w - 30, 30);
                os_gui_set_object_style(title_id, STYLE_ACCENT);
                os_gui_set_object_label(title_id, app.name.as_ptr(), app.name.len() as u32);

                // Publisher
                let pub_id = NEXT_NODE_ID; NEXT_NODE_ID += 1;
                os_gui_set_object_bounds(pub_id, cx + 15, ctx.current_y + 50, card_w - 30, 20);
                os_gui_set_object_style(pub_id, STYLE_PLAIN);
                let pub_label = alloc::format!("Developer: {}", app.publisher);
                os_gui_set_object_label(pub_id, pub_label.as_ptr(), pub_label.len() as u32);

                // Metadata (Rating + Size)
                let meta_id = NEXT_NODE_ID; NEXT_NODE_ID += 1;
                os_gui_set_object_bounds(meta_id, cx + 15, ctx.current_y + 75, card_w - 30, 20);
                os_gui_set_object_style(meta_id, STYLE_PLAIN);
                let meta_label = alloc::format!("{} | {} MB | {}", app.rating, app.size_mb, app.category);
                os_gui_set_object_label(meta_id, meta_label.as_ptr(), meta_label.len() as u32);

                // Action Button
                let btn_id = NEXT_NODE_ID; NEXT_NODE_ID += 1;
                os_gui_set_object_bounds(btn_id, cx + 15, ctx.current_y + 110, card_w - 30, 35);
                
                let mut btn_style = STYLE_ACCENT;
                if app.state == "INSTALLING" { btn_style = STYLE_PLAIN; }
                if app.state == "OPEN" { btn_style = STYLE_SIDEBAR; }
                
                os_gui_set_object_style(btn_id, btn_style);
                
                let btn_label = if app.state == "AVAILABLE" { alloc::format!("GET (ID: {})", app.id) }
                                else if app.state == "INSTALLING" { alloc::format!("INSTALLING...") }
                                else { alloc::format!("OPEN") };
                                
                os_gui_set_object_label(btn_id, btn_label.as_ptr(), btn_label.len() as u32);
                os_gui_set_object_interaction(btn_id, INTERACTION_HOVERED);

                cx += card_w + 20; // Gap between columns
                if ctx.current_y + card_h > max_y_in_row { max_y_in_row = ctx.current_y + card_h; }
            }
        }
        ctx.current_y = max_y_in_row + 30; // Move global Y pointer below the grid
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
                <h1 class='os-accent'>OpenRhiza App Store (GUI Edition)</h1>\n\
                <p>Discover human-friendly Wasm Applications.</p>\n\
                <br/>\n\
                <div id='app_grid'></div>\n\
                <br/>\n\
            </body>\n\
        </html>\
    ";
    
    DOM_ROOT = None; 
    NEXT_NODE_ID = 500;
    parse_html_to_dom(html);

    if let Some(ref root) = DOM_ROOT {
        // Draw the top navigation bar/header
        os_gui_set_object_bounds(100, 20, 20, 800, 40);
        os_gui_set_object_style(100, STYLE_CHROME);
        let title = b"App Store VFS Window (Human-Friendly UI)";
        os_gui_set_object_label(100, title.as_ptr(), title.len() as u32);

        let mut ctx = LayoutContext {
            current_y: 70,
            margin_x: 30, // Slight padding for aesthetics
            content_width: 800,
        };

        layout_dom_node(root, &mut ctx, 0);
    }
}

// -----------------------------------------------------------------------------
// UI Event Handlers
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
        
        // Find which button was clicked by parsing "GET (ID: X)"
        if clicked_text.starts_with("GET (ID:") {
            let inner = clicked_text[8..].trim_end_matches(')');
            if let Ok(app_id) = inner.trim().parse::<u32>() {
                if let Some(ref mut db) = APP_STORE_DB {
                    if let Some(app) = db.iter_mut().find(|x| x.id == app_id) {
                        app.state = String::from("OPEN");
                        log(&alloc::format!("[App Store] User Installed App: {}", app.name));
                        render_dashboard();
                    }
                }
            }
        } else if clicked_text == "OPEN" {
            log("[App Store] Launching Application in new Sandbox...");
        }
    }
    1
}

// -----------------------------------------------------------------------------
// Skill Entry Points
// -----------------------------------------------------------------------------
#[no_mangle]
pub extern "C" fn init_driver() {
    log("[AppStore] Human-Friendly GUI App Store Initialized.");
    unsafe { init_app_store(); }
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log("[AppStore] Opening Aesthetic Dashboard...");
    unsafe { 
        os_request_display_mode(2, 0, 0, 1024, 768); 
        render_dashboard();
    }
    1
}
