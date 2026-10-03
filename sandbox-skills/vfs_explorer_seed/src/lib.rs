#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::panic::PanicInfo;

// -----------------------------------------------------------------------------
// Bump Allocator (Used only for UI format strings, wiped every frame)
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
const STYLE_CODE_BG: u32 = 12;
const STYLE_CODE_TEXT: u32 = 13;
const STYLE_TERM_BG: u32 = 14;

const INTERACTION_HOVERED: u32 = 1; 

// -----------------------------------------------------------------------------
// Zero-Allocation Static VFS State
// -----------------------------------------------------------------------------
const MAX_DIRS: usize = 8;
const MAX_FILES: usize = 32;
const MAX_NAME: usize = 64;
const MAX_CONTENT: usize = 8192;

#[derive(Copy, Clone)]
struct VfsDir {
    id: u32,
    name: [u8; MAX_NAME],
    name_len: usize,
}

#[derive(Copy, Clone)]
struct VfsFile {
    id: u32,
    dir_id: u32,
    name: [u8; MAX_NAME],
    name_len: usize,
    content: [u8; MAX_CONTENT],
    content_len: usize,
}

impl VfsDir {
    const fn empty() -> Self { Self { id: 0, name: [0; MAX_NAME], name_len: 0 } }
}
impl VfsFile {
    const fn empty() -> Self { Self { id: 0, dir_id: 0, name: [0; MAX_NAME], name_len: 0, content: [0; MAX_CONTENT], content_len: 0 } }
}

static mut VFS_DIRS: [VfsDir; MAX_DIRS] = [VfsDir::empty(); MAX_DIRS];
static mut VFS_FILES: [VfsFile; MAX_FILES] = [VfsFile::empty(); MAX_FILES];

static mut OPEN_DIR_ID: u32 = 1;
static mut OPEN_FILE_ID: u32 = 0;

static mut NEXT_GUI_ID: u32 = 500;

// Terminal Log Buffer
const MAX_TERM_LOGS: usize = 8;
static mut TERM_LOGS: [[u8; 128]; MAX_TERM_LOGS] = [[0; 128]; MAX_TERM_LOGS];
static mut TERM_LOG_LENS: [usize; MAX_TERM_LOGS] = [0; MAX_TERM_LOGS];
static mut TERM_IDX: usize = 0;

unsafe fn push_term_log(msg: &str) {
    let bytes = msg.as_bytes();
    let len = core::cmp::min(bytes.len(), 128);
    
    // Shift logs up
    for i in 0..MAX_TERM_LOGS-1 {
        TERM_LOGS[i] = TERM_LOGS[i+1];
        TERM_LOG_LENS[i] = TERM_LOG_LENS[i+1];
    }
    
    let last = MAX_TERM_LOGS - 1;
    TERM_LOGS[last][..len].copy_from_slice(&bytes[..len]);
    TERM_LOG_LENS[last] = len;
}

unsafe fn init_vfs() {
    let mut d_idx = 0;
    for (id, name) in [(1, "/sys"), (2, "/home"), (3, "/boot"), (4, "/mnt/usb")] {
        VFS_DIRS[d_idx].id = id;
        VFS_DIRS[d_idx].name[..name.len()].copy_from_slice(name.as_bytes());
        VFS_DIRS[d_idx].name_len = name.len();
        d_idx += 1;
    }

    let mut f_idx = 0;
    for (id, d_id, name, cnt) in [
        (1, 1, "config.json", "{\n  \"os_version\": \"v1.2\",\n  \"auto_update\": true\n}"),
        (2, 1, "network.conf", "IFACE=e1000\nDHCP=true"),
        (3, 2, "agent_script.js", "// LLM Agent Automation Script\nfunction hack() { return true; }"),
        (4, 3, "grub.cfg", "menuentry 'OpenRhiza' {\n  linux /vmlinuz\n}"),
    ] {
        VFS_FILES[f_idx].id = id;
        VFS_FILES[f_idx].dir_id = d_id;
        VFS_FILES[f_idx].name[..name.len()].copy_from_slice(name.as_bytes());
        VFS_FILES[f_idx].name_len = name.len();
        VFS_FILES[f_idx].content[..cnt.len()].copy_from_slice(cnt.as_bytes());
        VFS_FILES[f_idx].content_len = cnt.len();
        f_idx += 1;
    }

    push_term_log("[System] VFS Initialized.");
    push_term_log("[Agent Console] Awaiting autonomous commands...");
}

// -----------------------------------------------------------------------------
// V3.0 Zero-Allocation VDOM Parser
// -----------------------------------------------------------------------------
#[derive(Copy, Clone)]
pub struct Node<'a> {
    pub tag: &'a str,
    pub text: &'a str,
    pub class_attr: &'a str,
    pub id_attr: &'a str,
    pub child_indices: [usize; 64],
    pub child_count: usize,
}

impl<'a> Node<'a> {
    const fn empty() -> Self {
        Self { tag: "", text: "", class_attr: "", id_attr: "", child_indices: [0; 64], child_count: 0 }
    }
}

pub struct DomTree<'a> {
    pub pool: [Node<'a>; 1024], // Increased for syntax spans
    pub count: usize,
    pub root_idx: usize,
}

impl<'a> DomTree<'a> {
    fn new() -> Self { Self { pool: [Node::empty(); 1024], count: 0, root_idx: 0 } }
    fn alloc_node(&mut self, tag: &'a str) -> usize {
        if self.count >= 1024 { return 0; }
        let idx = self.count;
        self.pool[idx].tag = tag;
        self.count += 1;
        idx
    }
    fn add_child(&mut self, parent: usize, child: usize) {
        let p = &mut self.pool[parent];
        if p.child_count < 64 {
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
    
    let mut stack: [usize; 64] = [0; 64];
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
                    let parent_idx = stack[stack_ptr];
                    
                    // IF parent is `code`, we should actually not trim if we care about spaces,
                    // but for now trimmed is fine.
                    // Wait, if we want multiple text chunks, we must allocate a text node!
                    // Let's just create a dummy text node.
                    let text_node_idx = tree.alloc_node("text");
                    tree.pool[text_node_idx].text = trimmed;
                    tree.add_child(parent_idx, text_node_idx);
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
                let id_attr = extract_attr(tag_content, "id");
                
                let node_idx = tree.alloc_node(name);
                tree.pool[node_idx].class_attr = class_attr;
                tree.pool[node_idx].id_attr = id_attr;
                
                let parent_idx = stack[stack_ptr];
                tree.add_child(parent_idx, node_idx);
                
                if !is_void && stack_ptr < 63 {
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
// Syntax Highlighting Lexer (Zero Allocation approach inside UI generator)
// -----------------------------------------------------------------------------
fn colorize_word(w: &str) -> String {
    match w {
        "function" | "fn" | "let" | "const" | "return" | "true" | "false" | "if" | "else" | "struct" | "impl" => {
            alloc::format!("<span class='os-accent'>{}</span>", w)
        }
        _ => w.to_string()
    }
}

fn syntax_highlight(line: &str) -> String {
    let mut hl = String::new();
    let mut word = String::new();
    
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            if !word.is_empty() {
                hl.push_str(&colorize_word(&word));
                word.clear();
            }
            if c == ' ' { hl.push_str(" "); }
            else if c == '<' { hl.push_str("&lt;"); }
            else if c == '>' { hl.push_str("&gt;"); }
            else { hl.push(c); }
        }
    }
    if !word.is_empty() { hl.push_str(&colorize_word(&word)); }
    hl
}

// -----------------------------------------------------------------------------
// UI Layout Engine (Three-Pane IDE)
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
    
    // 3-Pane Layout Override Flags
    if node.class_attr == "pane-left" { ctx.margin_x = 20; ctx.content_width = 250; ctx.current_y = 80; ctx.inline_x = 0; }
    if node.class_attr == "pane-right-top" { ctx.margin_x = 290; ctx.content_width = 700; ctx.current_y = 80; ctx.inline_x = 0; }
    if node.class_attr == "pane-right-bot" { ctx.margin_x = 290; ctx.content_width = 700; ctx.current_y = 550; ctx.inline_x = 0; }

    match tag_lower {
        "h1" => { style = STYLE_ACCENT; height = 40; indent = 0; }
        "h2" | "h3" => { style = STYLE_ACCENT; height = 30; indent = 0; }
        "button" => { style = STYLE_SIDEBAR; height = 30; }
        "pre" | "code" => { style = STYLE_CODE_TEXT; height = 20; indent = 0; }
        "span" => { style = STYLE_CODE_TEXT; height = 20; indent = 0; } // Inline
        _ => {}
    }

    if node.class_attr.contains("os-chrome") { style = STYLE_CHROME; }
    else if node.class_attr.contains("os-sidebar") { style = STYLE_SIDEBAR; }
    else if node.class_attr.contains("os-accent") { style = STYLE_ACCENT; }

    // Render Box Backgrounds
    if node.class_attr == "pane-right-top" {
        let bg_id = NEXT_GUI_ID; NEXT_GUI_ID += 1;
        os_gui_set_object_bounds(bg_id, win_x + ctx.margin_x, 80, ctx.content_width, 450);
        os_gui_set_object_style(bg_id, STYLE_CODE_BG);
        os_gui_set_object_label(bg_id, b"".as_ptr(), 0);
    }
    if node.class_attr == "pane-right-bot" {
        let bg_id = NEXT_GUI_ID; NEXT_GUI_ID += 1;
        os_gui_set_object_bounds(bg_id, win_x + ctx.margin_x, 550, ctx.content_width, 180);
        os_gui_set_object_style(bg_id, STYLE_TERM_BG);
        os_gui_set_object_label(bg_id, b"".as_ptr(), 0);
    }

    let is_inline = tag_lower == "span" || tag_lower == "text";

    if !node.text.is_empty() {
        let obj_id = NEXT_GUI_ID; NEXT_GUI_ID += 1;
        let mut final_x = win_x + ctx.margin_x + indent;
        
        if is_inline { final_x += ctx.inline_x; }

        let text_w = core::cmp::max(20, (node.text.len() as u32) * 8);
        
        os_gui_set_object_bounds(obj_id, final_x, ctx.current_y, text_w, height);
        os_gui_set_object_style(obj_id, style);
        os_gui_set_object_label(obj_id, node.text.as_ptr(), node.text.len() as u32);
        
        if tag_lower == "button" || tag_lower == "a" || node.class_attr.contains("clickable") {
            os_gui_set_object_interaction(obj_id, INTERACTION_HOVERED);
        }
        
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

    if tag_lower == "code" {
        ctx.current_y += height + 8;
        ctx.inline_x = 0;
    }
}

// Generate the HTML based on VFS State dynamically
unsafe fn generate_ui_html() -> String {
    let mut html = String::new();
    html.push_str("<h1 class='os-accent'>OpenRhiza AI IDE</h1>");
    
    // 1. LEFT PANE (Directory Tree)
    html.push_str("<div class='pane-left'>");
    html.push_str("<h3>System Root</h3><ul>");
    for i in 0..MAX_DIRS {
        if VFS_DIRS[i].id != 0 {
            let name = core::str::from_utf8_unchecked(&VFS_DIRS[i].name[..VFS_DIRS[i].name_len]);
            let s_class = if OPEN_DIR_ID == VFS_DIRS[i].id { "os-accent" } else { "os-sidebar" };
            html.push_str(&alloc::format!("<li><button class='{}' id='dir_{}'>[+] {}</button></li>", s_class, VFS_DIRS[i].id, name));
        }
    }
    html.push_str("</ul><br/><h3>Directory Contents</h3><ul>");
    for i in 0..MAX_FILES {
        if VFS_FILES[i].id != 0 && VFS_FILES[i].dir_id == OPEN_DIR_ID {
            let name = core::str::from_utf8_unchecked(&VFS_FILES[i].name[..VFS_FILES[i].name_len]);
            let s_class = if OPEN_FILE_ID == VFS_FILES[i].id { "os-accent" } else { "clickable" };
            html.push_str(&alloc::format!("<li><a class='{}' id='file_{}'>📄 {}</a></li>", s_class, VFS_FILES[i].id, name));
        }
    }
    html.push_str("</ul></div>");

    // 2. RIGHT-TOP PANE (Code Editor with Lexer)
    html.push_str("<div class='pane-right-top'>");
    if OPEN_FILE_ID != 0 {
        for i in 0..MAX_FILES {
            if VFS_FILES[i].id == OPEN_FILE_ID {
                let name = core::str::from_utf8_unchecked(&VFS_FILES[i].name[..VFS_FILES[i].name_len]);
                let content = core::str::from_utf8_unchecked(&VFS_FILES[i].content[..VFS_FILES[i].content_len]);
                html.push_str(&alloc::format!("<h3>Editing: {}</h3>", name));
                
                html.push_str("<pre>");
                for (line_num, line) in content.lines().enumerate() {
                    let highlighted = syntax_highlight(line);
                    html.push_str(&alloc::format!("<code><span class='os-sidebar'>{:02} | </span>{}</code>", line_num + 1, highlighted));
                }
                html.push_str("</pre>");
                break;
            }
        }
    } else {
        html.push_str("<h3>IDE Mode</h3><p>Select a file to edit.</p>");
    }
    html.push_str("</div>");

    // 3. RIGHT-BOT PANE (AI Terminal)
    html.push_str("<div class='pane-right-bot'>");
    html.push_str("<h3 class='os-sidebar'>Agent Terminal</h3>");
    for i in 0..MAX_TERM_LOGS {
        if TERM_LOG_LENS[i] > 0 {
            let log_msg = core::str::from_utf8_unchecked(&TERM_LOGS[i][..TERM_LOG_LENS[i]]);
            html.push_str(&alloc::format!("<code>&gt; {}</code>", log_msg));
        }
    }
    html.push_str("</div>");

    html
}

unsafe fn render_dashboard() {
    ALLOCATOR.reset();
    NEXT_GUI_ID = 500;
    
    os_gui_set_object_bounds(10, 0, 0, 1024, 768);
    os_gui_set_object_style(10, STYLE_CHROME);
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
// Agent Automation APIs
// -----------------------------------------------------------------------------
#[no_mangle]
pub extern "C" fn agent_cmd_read_file(file_id: u32) -> u32 {
    unsafe {
        for i in 0..MAX_FILES {
            if VFS_FILES[i].id == file_id {
                let name = core::str::from_utf8_unchecked(&VFS_FILES[i].name[..VFS_FILES[i].name_len]);
                push_term_log(&alloc::format!("[Agent] Reading file: {}", name));
                render_dashboard();
                return VFS_FILES[i].content_len as u32;
            }
        }
        0
    }
}

#[no_mangle]
pub extern "C" fn agent_cmd_write_file(file_id: u32, ptr: *const u8, len: u32) -> u32 {
    unsafe {
        for i in 0..MAX_FILES {
            if VFS_FILES[i].id == file_id {
                let slice = core::slice::from_raw_parts(ptr, len as usize);
                let write_len = core::cmp::min(slice.len(), MAX_CONTENT);
                VFS_FILES[i].content[..write_len].copy_from_slice(&slice[..write_len]);
                VFS_FILES[i].content_len = write_len;
                
                let name = core::str::from_utf8_unchecked(&VFS_FILES[i].name[..VFS_FILES[i].name_len]);
                push_term_log(&alloc::format!("[Agent Auto-Coder] Modified {}", name));
                
                render_dashboard();
                return 1;
            }
        }
        0
    }
}

#[no_mangle]
pub extern "C" fn browser_click_element(id: u32) -> u32 { 1 }

#[no_mangle]
pub extern "C" fn agent_eval_js(script_ptr: *const u8, script_len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(script_ptr, script_len as usize);
        if let Ok(script) = core::str::from_utf8(slice) {
            let s = script.trim();
            if s.starts_with("open_dir(") {
                let inner = s[9..].trim_end_matches(|c| c == ')' || c == ';');
                if let Ok(id) = inner.parse::<u32>() {
                    OPEN_DIR_ID = id;
                    OPEN_FILE_ID = 0;
                    push_term_log(&alloc::format!("[Agent] Navigating to Directory {}", id));
                    render_dashboard();
                }
            } else if s.starts_with("open_file(") {
                let inner = s[10..].trim_end_matches(|c| c == ')' || c == ';');
                if let Ok(id) = inner.parse::<u32>() {
                    OPEN_FILE_ID = id;
                    push_term_log(&alloc::format!("[Agent] Opening File {}", id));
                    render_dashboard();
                }
            } else if s.starts_with("write(") {
                let inner = s[6..].trim_end_matches(|c| c == ')' || c == ';');
                let parts: Vec<&str> = inner.splitn(2, ',').collect();
                if parts.len() == 2 {
                    if let Ok(id) = parts[0].trim().parse::<u32>() {
                        let content = parts[1].trim().trim_matches(|c| c == '\'' || c == '"');
                        agent_cmd_write_file(id, content.as_ptr(), content.len() as u32);
                    }
                }
            }
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log("[VFS Explorer] File System & IDE Initialized.");
    unsafe { init_vfs(); }
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log("[VFS Explorer] Booting IDE GUI...");
    unsafe { 
        os_request_display_mode(2, 0, 0, 1024, 768); 
        render_dashboard();
    }
    1
}
