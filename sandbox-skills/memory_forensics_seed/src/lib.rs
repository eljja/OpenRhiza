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
const STYLE_HEX_BG: u32 = 12;
const STYLE_HEX_TEXT: u32 = 13;
const STYLE_WARNING: u32 = 15;

const INTERACTION_HOVERED: u32 = 1; 

// -----------------------------------------------------------------------------
// Mock OS Kernel Memory State (RAM)
// -----------------------------------------------------------------------------
const MEM_SIZE: usize = 1024; // 1KB mock block for display
static mut KERNEL_RAM: [u8; MEM_SIZE] = [0; MEM_SIZE];
static mut LAST_PATCHED_ADDR: i32 = -1;
static mut CURRENT_OFFSET: usize = 0; // The address we are viewing

static mut NEXT_GUI_ID: u32 = 500;

unsafe fn init_memory() {
    // Fill memory with some fake OS data patterns
    for i in 0..MEM_SIZE {
        if i < 64 { KERNEL_RAM[i] = 0xFF; } // Bootloader Header
        else if i % 16 == 0 { KERNEL_RAM[i] = 0xAA; } // Alignments
        else if i >= 512 && i < 560 { 
            let msg = b"OpenRhiza v1.0 Kernel Panic String Block...";
            KERNEL_RAM[i] = *msg.get(i - 512).unwrap_or(&0);
        }
        else {
            KERNEL_RAM[i] = (i % 255) as u8; // Junk data
        }
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
    pub child_indices: [usize; 64],
    pub child_count: usize,
}

impl<'a> Node<'a> {
    const fn empty() -> Self {
        Self { tag: "", text: "", class_attr: "", child_indices: [0; 64], child_count: 0 }
    }
}

pub struct DomTree<'a> {
    pub pool: [Node<'a>; 512],
    pub count: usize,
    pub root_idx: usize,
}

impl<'a> DomTree<'a> {
    fn new() -> Self { Self { pool: [Node::empty(); 512], count: 0, root_idx: 0 } }
    fn alloc_node(&mut self, tag: &'a str) -> usize {
        if self.count >= 512 { return 0; }
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
// UI Layout Engine (Hex Editor)
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
    if node.class_attr == "pane-left" { ctx.margin_x = 20; ctx.content_width = 250; ctx.current_y = 80; ctx.inline_x = 0; }
    if node.class_attr == "pane-right" { ctx.margin_x = 290; ctx.content_width = 700; ctx.current_y = 80; ctx.inline_x = 0; }

    match tag_lower {
        "h1" => { style = STYLE_ACCENT; height = 40; indent = 0; }
        "h2" | "h3" => { style = STYLE_ACCENT; height = 30; indent = 0; }
        "button" => { style = STYLE_SIDEBAR; height = 30; }
        "span" => { height = 20; indent = 0; }
        "code" => { height = 20; indent = 0; }
        _ => {}
    }

    if node.class_attr.contains("os-chrome") { style = STYLE_CHROME; }
    else if node.class_attr.contains("os-sidebar") { style = STYLE_SIDEBAR; }
    else if node.class_attr.contains("os-accent") { style = STYLE_ACCENT; }
    else if node.class_attr.contains("os-warning") { style = STYLE_WARNING; }
    else if node.class_attr.contains("hex-text") { style = STYLE_HEX_TEXT; }

    if node.class_attr == "pane-right" {
        let bg_id = NEXT_GUI_ID; NEXT_GUI_ID += 1;
        os_gui_set_object_bounds(bg_id, win_x + ctx.margin_x, 80, ctx.content_width, 600);
        os_gui_set_object_style(bg_id, STYLE_HEX_BG);
        os_gui_set_object_label(bg_id, b"".as_ptr(), 0);
    }

    let is_inline = tag_lower == "span" || tag_lower == "text";

    if !node.text.is_empty() {
        let obj_id = NEXT_GUI_ID; NEXT_GUI_ID += 1;
        let mut final_x = win_x + ctx.margin_x + indent;
        if is_inline { final_x += ctx.inline_x; }

        // Hacky width calc: monospace hex characters
        let text_w = core::cmp::max(16, (node.text.len() as u32) * 9); 
        
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
    
    if tag_lower == "code" {
        ctx.current_y += height + 6;
        ctx.inline_x = 0;
    }
}

// Helper to format a byte as 2-char hex string
fn byte_to_hex(b: u8) -> String {
    alloc::format!("{:02X}", b)
}

// Generate the HTML based on RAM dynamically
unsafe fn generate_ui_html() -> String {
    let mut html = String::new();
    html.push_str("<h1 class='os-accent'>OpenRhiza Kernel Memory Forensics</h1>");
    
    // LEFT PANE (Memory Map)
    html.push_str("<div class='pane-left'>");
    html.push_str("<h3>Segments Map</h3><ul>");
    html.push_str("<li><button class='os-accent'>[ 0x0000 ] Boot Header</button></li>");
    html.push_str("<li><button class='os-sidebar'>[ 0x0100 ] Device Tree</button></li>");
    html.push_str("<li><button class='os-sidebar'>[ 0x0200 ] Kernel Panic String</button></li>");
    html.push_str("<li><button class='os-sidebar'>[ 0x0300 ] Wasm Heap</button></li>");
    html.push_str("</ul></div>");

    // RIGHT PANE (Hex Dump Grid)
    html.push_str("<div class='pane-right'>");
    html.push_str("<h3 class='os-sidebar'>Hex Dump View (16-byte blocks)</h3>");
    
    // Render 30 lines of Hex
    let start_addr = CURRENT_OFFSET;
    for line in 0..25 {
        let addr = start_addr + (line * 16);
        if addr >= MEM_SIZE { break; }
        
        // Offset Label
        html.push_str(&alloc::format!("<code><span class='os-sidebar'>0x{:04X}: </span>", addr));
        
        // 16 Bytes Hex
        for offset in 0..16 {
            if addr + offset < MEM_SIZE {
                let mem_addr = addr + offset;
                let b = KERNEL_RAM[mem_addr];
                
                let mut css = "hex-text";
                if mem_addr as i32 == LAST_PATCHED_ADDR { css = "os-warning"; }
                else if b == 0x00 { css = "os-sidebar"; } // Fade out zeros
                
                html.push_str(&alloc::format!("<span class='{}'>{} </span>", css, byte_to_hex(b)));
            }
        }
        
        // Divider
        html.push_str("<span class='os-sidebar'>| </span>");
        
        // 16 Bytes ASCII
        for offset in 0..16 {
            if addr + offset < MEM_SIZE {
                let mem_addr = addr + offset;
                let b = KERNEL_RAM[mem_addr];
                let mut ascii = ".";
                if b >= 32 && b <= 126 {
                    ascii = core::str::from_utf8(&[b]).unwrap_or(".");
                }
                
                let mut css = "hex-text";
                if mem_addr as i32 == LAST_PATCHED_ADDR { css = "os-warning"; }
                
                // HTML Escape
                if ascii == "<" { html.push_str(&alloc::format!("<span class='{}'>&lt;</span>", css)); }
                else if ascii == ">" { html.push_str(&alloc::format!("<span class='{}'>&gt;</span>", css)); }
                else { html.push_str(&alloc::format!("<span class='{}'>{}</span>", css, ascii)); }
            }
        }
        html.push_str("</code>");
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
// Agent Memory Manipulation APIs (The Ultimate Hack)
// -----------------------------------------------------------------------------
#[no_mangle]
pub extern "C" fn agent_cmd_read_mem(addr: u32, len: u32) -> u32 {
    unsafe {
        let addr_usize = addr as usize;
        let len_usize = len as usize;
        if addr_usize + len_usize <= MEM_SIZE {
            log(&alloc::format!("[Forensics] Read {} bytes at 0x{:04X}", len, addr));
            // In reality, this would return a pointer to an external buffer
            return 1;
        }
        0
    }
}

#[no_mangle]
pub extern "C" fn agent_cmd_patch_mem(addr: u32, byte_val: u32) -> u32 {
    unsafe {
        let addr_usize = addr as usize;
        if addr_usize < MEM_SIZE {
            let old_val = KERNEL_RAM[addr_usize];
            let new_val = (byte_val & 0xFF) as u8;
            KERNEL_RAM[addr_usize] = new_val;
            LAST_PATCHED_ADDR = addr as i32;
            
            log(&alloc::format!("[Forensics] PATCH SUCCESS: 0x{:04X} [ 0x{:02X} -> 0x{:02X} ]", addr, old_val, new_val));
            render_dashboard();
            return 1;
        }
        0
    }
}

#[no_mangle]
pub extern "C" fn agent_eval_js(script_ptr: *const u8, script_len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(script_ptr, script_len as usize);
        if let Ok(script) = core::str::from_utf8(slice) {
            let s = script.trim();
            if s.starts_with("patch(") {
                // patch(0x0200, 0xFF)
                let inner = s[6..].trim_end_matches(|c| c == ')' || c == ';');
                let parts: Vec<&str> = inner.split(',').collect();
                if parts.len() == 2 {
                    // Primitive Hex String Parser for Script
                    let addr_str = parts[0].trim().trim_start_matches("0x");
                    let val_str = parts[1].trim().trim_start_matches("0x");
                    if let (Ok(addr), Ok(val)) = (u32::from_str_radix(addr_str, 16), u32::from_str_radix(val_str, 16)) {
                        agent_cmd_patch_mem(addr, val);
                    }
                }
            } else if s.starts_with("goto(") {
                let inner = s[5..].trim_end_matches(|c| c == ')' || c == ';');
                let addr_str = inner.trim().trim_start_matches("0x");
                if let Ok(addr) = usize::from_str_radix(addr_str, 16) {
                    CURRENT_OFFSET = addr;
                    render_dashboard();
                }
            }
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log("[Memory Forensics] Hex Editor Sandbox Initialized.");
    unsafe { init_memory(); }
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log("[Memory Forensics] Booting Cybernetic Hex Dump UI...");
    unsafe { 
        os_request_display_mode(2, 0, 0, 1024, 768); 
        render_dashboard();
    }
    1
}
