#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::panic::PanicInfo;

// -----------------------------------------------------------------------------
// Bump Allocator
// -----------------------------------------------------------------------------
struct BumpAllocator {
    heap: UnsafeCell<[u8; 1024 * 1024 * 4]>,
    next: UnsafeCell<usize>,
}

unsafe impl Sync for BumpAllocator {}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator {
    heap: UnsafeCell::new([0; 1024 * 1024 * 4]),
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
        if end > 1024 * 1024 * 4 { return core::ptr::null_mut(); }
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
const STYLE_HEX_TEXT: u32 = 13;
const STYLE_WARNING: u32 = 15;
const STYLE_SAFE: u32 = 16;
const STYLE_CRITICAL: u32 = 17; // For Pointers & Memory

// -----------------------------------------------------------------------------
// Compiler State
// -----------------------------------------------------------------------------
const MAX_SRC_LEN: usize = 2048;
const MAX_WASM_LEN: usize = 4096;

static mut SOURCE_CODE: [u8; MAX_SRC_LEN] = [0; MAX_SRC_LEN];
static mut SOURCE_LEN: usize = 0;

static mut COMPILED_WASM: [u8; MAX_WASM_LEN] = [0; MAX_WASM_LEN];
static mut WASM_LEN: usize = 0;
static mut COMPILATION_SUCCESS: bool = false;
static mut COMPILATION_MSG: [u8; 128] = [0; 128];
static mut COMPILATION_MSG_LEN: usize = 0;

// -----------------------------------------------------------------------------
// 1. Tokenizer (Lexer)
// -----------------------------------------------------------------------------
#[derive(Debug, Clone, PartialEq)]
enum Token<'a> {
    KwFn, KwLet, KwMut, KwWhile, KwReturn, KwI32,
    Ident(&'a str), Number(u32),
    Plus, Minus, Eq, Lt, Semi, LParen, RParen, LBrace, RBrace, Arrow, Asterisk,
    Error,
}

struct Lexer<'a> { src: &'a str, pos: usize }
impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self { Self { src, pos: 0 } }
    fn peek(&self) -> Option<char> { self.src[self.pos..].chars().next() }
    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }
    
    fn next_token(&mut self) -> Option<Token<'a>> {
        while let Some(c) = self.peek() {
            if c.is_whitespace() { self.advance(); continue; }
            match c {
                '+' => { self.advance(); return Some(Token::Plus); }
                '-' => {
                    self.advance();
                    if self.peek() == Some('>') { self.advance(); return Some(Token::Arrow); }
                    return Some(Token::Minus);
                }
                '*' => { self.advance(); return Some(Token::Asterisk); }
                '=' => { self.advance(); return Some(Token::Eq); }
                '<' => { self.advance(); return Some(Token::Lt); }
                ';' => { self.advance(); return Some(Token::Semi); }
                '(' => { self.advance(); return Some(Token::LParen); }
                ')' => { self.advance(); return Some(Token::RParen); }
                '{' => { self.advance(); return Some(Token::LBrace); }
                '}' => { self.advance(); return Some(Token::RBrace); }
                _ if c.is_ascii_digit() => {
                    let start = self.pos;
                    while let Some(d) = self.peek() {
                        if d.is_ascii_digit() { self.advance(); } else { break; }
                    }
                    if let Ok(n) = self.src[start..self.pos].parse::<u32>() { return Some(Token::Number(n)); }
                }
                _ if c.is_alphabetic() || c == '_' => {
                    let start = self.pos;
                    while let Some(a) = self.peek() {
                        if a.is_alphanumeric() || a == '_' { self.advance(); } else { break; }
                    }
                    let word = &self.src[start..self.pos];
                    match word {
                        "fn" => return Some(Token::KwFn),
                        "let" => return Some(Token::KwLet),
                        "mut" => return Some(Token::KwMut),
                        "while" => return Some(Token::KwWhile),
                        "return" => return Some(Token::KwReturn),
                        "i32" => return Some(Token::KwI32),
                        _ => return Some(Token::Ident(word)),
                    }
                }
                _ => { self.advance(); return Some(Token::Error); }
            }
        }
        None
    }
}

// -----------------------------------------------------------------------------
// 2. Abstract Syntax Tree (AST) Arena & Symbol Table
// -----------------------------------------------------------------------------
#[derive(Debug, Clone, Copy)]
enum AstNode {
    Empty,
    ExprNum(u32),
    ExprVar(usize), // symbol index
    ExprPointerRead(usize), // symbol index (addr)
    ExprAdd(usize, usize), // left node, right node
    ExprLt(usize, usize),
    StmtLet(usize, usize), // symbol index, expr node
    StmtAssign(usize, usize), // symbol index, expr node
    StmtPointerWrite(usize, usize), // symbol index (addr), expr node (value)
    StmtWhile(usize, usize), // expr node, block node
    StmtReturn(usize), // expr node
    BlockList(usize, usize), // stmt node, next BlockList node
}

struct AstArena {
    nodes: [AstNode; 256],
    count: usize,
}
impl AstArena {
    fn new() -> Self { Self { nodes: [AstNode::Empty; 256], count: 0 } }
    fn alloc(&mut self, node: AstNode) -> usize {
        if self.count >= 256 { return 0; }
        let idx = self.count;
        self.nodes[idx] = node;
        self.count += 1;
        idx
    }
}

#[derive(Clone, Copy)]
struct Symbol<'a> {
    name: &'a str,
    local_idx: u8,
}
struct SymbolTable<'a> {
    syms: [Symbol<'a>; 32],
    count: usize,
}
impl<'a> SymbolTable<'a> {
    fn new() -> Self { Self { syms: [Symbol { name: "", local_idx: 0 }; 32], count: 0 } }
    fn add(&mut self, name: &'a str) -> usize {
        for i in 0..self.count {
            if self.syms[i].name == name { return i; }
        }
        let idx = self.count;
        self.syms[idx] = Symbol { name, local_idx: idx as u8 };
        self.count += 1;
        idx
    }
    fn find(&self, name: &str) -> Option<usize> {
        for i in 0..self.count {
            if self.syms[i].name == name { return Some(i); }
        }
        None
    }
}

struct Parser<'a> {
    tokens: &'a [Token<'a>],
    pos: usize,
    arena: &'a mut AstArena,
    syms: &'a mut SymbolTable<'a>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token<'a>> { self.tokens.get(self.pos) }
    fn advance(&mut self) -> Option<&Token<'a>> { let t = self.peek()?; self.pos += 1; Some(t) }
    
    fn parse_expr(&mut self) -> Option<usize> {
        let left = match self.peek()? {
            Token::Asterisk => {
                self.advance();
                if let Token::Ident(name) = self.advance()? {
                    let sym_idx = self.syms.find(name)?;
                    self.arena.alloc(AstNode::ExprPointerRead(sym_idx))
                } else {
                    return None;
                }
            }
            Token::Number(_) => {
                if let Token::Number(n) = self.advance()? {
                    self.arena.alloc(AstNode::ExprNum(*n))
                } else { return None; }
            }
            Token::Ident(_) => {
                if let Token::Ident(name) = self.advance()? {
                    let sym_idx = self.syms.find(name)?;
                    self.arena.alloc(AstNode::ExprVar(sym_idx))
                } else { return None; }
            }
            _ => return None,
        };
        
        match self.peek() {
            Some(Token::Plus) => {
                self.advance();
                let right = self.parse_expr()?;
                Some(self.arena.alloc(AstNode::ExprAdd(left, right)))
            }
            Some(Token::Lt) => {
                self.advance();
                let right = self.parse_expr()?;
                Some(self.arena.alloc(AstNode::ExprLt(left, right)))
            }
            _ => Some(left)
        }
    }

    fn parse_stmt(&mut self) -> Option<usize> {
        match self.peek()? {
            Token::Asterisk => { // Pointer Write: *ptr = expr;
                self.advance();
                if let Token::Ident(name) = self.advance()? {
                    if self.advance() == Some(&Token::Eq) {
                        let expr = self.parse_expr()?;
                        if self.advance() == Some(&Token::Semi) {
                            let sym_idx = self.syms.find(name)?;
                            return Some(self.arena.alloc(AstNode::StmtPointerWrite(sym_idx, expr)));
                        }
                    }
                }
            }
            Token::KwLet => {
                self.advance();
                if self.peek() == Some(&Token::KwMut) { self.advance(); }
                if let Some(Token::Ident(name)) = self.advance() {
                    if self.advance() == Some(&Token::Eq) {
                        let expr = self.parse_expr()?;
                        if self.advance() == Some(&Token::Semi) {
                            let sym_idx = self.syms.add(name);
                            return Some(self.arena.alloc(AstNode::StmtLet(sym_idx, expr)));
                        }
                    }
                }
            }
            Token::Ident(name) => {
                // Peek ahead to see if it's an assignment
                if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Eq {
                    self.advance(); // consume ident
                    self.advance(); // consume eq
                    let expr = self.parse_expr()?;
                    if self.advance() == Some(&Token::Semi) {
                        let sym_idx = self.syms.find(name)?;
                        return Some(self.arena.alloc(AstNode::StmtAssign(sym_idx, expr)));
                    }
                }
            }
            Token::KwWhile => {
                self.advance();
                let cond = self.parse_expr()?;
                if self.advance() == Some(&Token::LBrace) {
                    let block = self.parse_block()?;
                    if self.advance() == Some(&Token::RBrace) {
                        return Some(self.arena.alloc(AstNode::StmtWhile(cond, block)));
                    }
                }
            }
            Token::KwReturn => {
                self.advance();
                let expr = self.parse_expr()?;
                if self.advance() == Some(&Token::Semi) {
                    return Some(self.arena.alloc(AstNode::StmtReturn(expr)));
                }
            }
            _ => {}
        }
        None
    }

    fn parse_block(&mut self) -> Option<usize> {
        let mut first = 0;
        let mut last_list_node = 0;
        let mut has_nodes = false;

        while let Some(t) = self.peek() {
            if t == &Token::RBrace { break; }
            if let Some(stmt) = self.parse_stmt() {
                let list_node = self.arena.alloc(AstNode::BlockList(stmt, 0));
                if !has_nodes {
                    first = list_node;
                    has_nodes = true;
                } else {
                    if let AstNode::BlockList(_, ref mut next) = self.arena.nodes[last_list_node] {
                        *next = list_node;
                    }
                }
                last_list_node = list_node;
            } else {
                return None; // Parse error
            }
        }
        Some(first)
    }

    fn parse_program(&mut self) -> Option<usize> {
        while let Some(t) = self.advance() {
            if t == &Token::KwFn {
                self.advance(); // ident
                self.advance(); // (
                self.advance(); // )
                self.advance(); // ->
                self.advance(); // i32
                if self.advance() == Some(&Token::LBrace) {
                    return self.parse_block();
                }
            }
        }
        None
    }
}

// -----------------------------------------------------------------------------
// 3. Wasm Bytecode Emitter (Memory Support!)
// -----------------------------------------------------------------------------
fn set_err(msg: &str) {
    unsafe {
        let bytes = msg.as_bytes();
        let len = core::cmp::min(bytes.len(), 128);
        COMPILATION_MSG[..len].copy_from_slice(&bytes[..len]);
        COMPILATION_MSG_LEN = len;
        COMPILATION_SUCCESS = false;
    }
}

fn emit_leb128(val: u32, buf: &mut Vec<u8>) {
    let mut v = val;
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            buf.push(byte);
            break;
        } else {
            buf.push(byte | 0x80);
        }
    }
}

fn traverse_ast(node_idx: usize, arena: &AstArena, syms: &SymbolTable, code: &mut Vec<u8>) {
    if node_idx == 0 { return; }
    match arena.nodes[node_idx] {
        AstNode::ExprNum(n) => {
            code.push(0x41); // i32.const
            emit_leb128(n, code);
        }
        AstNode::ExprVar(sym_idx) => {
            code.push(0x20); // local.get
            code.push(syms.syms[sym_idx].local_idx);
        }
        AstNode::ExprPointerRead(sym_idx) => {
            code.push(0x20); // local.get (Address)
            code.push(syms.syms[sym_idx].local_idx);
            code.push(0x28); // i32.load
            code.push(0x02); // align=2 (32bit)
            code.push(0x00); // offset=0
        }
        AstNode::ExprAdd(a, b) => {
            traverse_ast(a, arena, syms, code);
            traverse_ast(b, arena, syms, code);
            code.push(0x6A); // i32.add
        }
        AstNode::ExprLt(a, b) => {
            traverse_ast(a, arena, syms, code);
            traverse_ast(b, arena, syms, code);
            code.push(0x48); // i32.lt_s
        }
        AstNode::StmtLet(sym_idx, expr_idx) | AstNode::StmtAssign(sym_idx, expr_idx) => {
            traverse_ast(expr_idx, arena, syms, code);
            code.push(0x21); // local.set
            code.push(syms.syms[sym_idx].local_idx);
        }
        AstNode::StmtPointerWrite(sym_idx, expr_idx) => {
            code.push(0x20); // local.get (Address)
            code.push(syms.syms[sym_idx].local_idx);
            traverse_ast(expr_idx, arena, syms, code); // (Value)
            code.push(0x36); // i32.store
            code.push(0x02); // align=2
            code.push(0x00); // offset=0
        }
        AstNode::StmtWhile(cond_idx, block_idx) => {
            code.push(0x02); code.push(0x40); // block empty
            code.push(0x03); code.push(0x40); // loop empty
            
            traverse_ast(cond_idx, arena, syms, code); // push cond
            code.push(0x45); // i32.eqz (invert condition)
            code.push(0x0D); code.push(0x01); // br_if 1 (break out of block)
            
            traverse_ast(block_idx, arena, syms, code); // exec body
            
            code.push(0x0C); code.push(0x00); // br 0 (continue loop)
            code.push(0x0B); // end loop
            code.push(0x0B); // end block
        }
        AstNode::StmtReturn(expr_idx) => {
            traverse_ast(expr_idx, arena, syms, code);
            code.push(0x0F); // return
        }
        AstNode::BlockList(stmt_idx, next_idx) => {
            traverse_ast(stmt_idx, arena, syms, code);
            traverse_ast(next_idx, arena, syms, code);
        }
        AstNode::Empty => {}
    }
}

static mut SYM_TABLE_LOG: [u8; 1024] = [0; 1024];
static mut SYM_TABLE_LOG_LEN: usize = 0;

unsafe fn compile_micro_rust() -> bool {
    let src = core::str::from_utf8_unchecked(&SOURCE_CODE[..SOURCE_LEN]);
    
    let mut lexer = Lexer::new(src);
    let mut tokens = Vec::new();
    while let Some(t) = lexer.next_token() {
        if t == Token::Error { set_err("Lexer Error: Unrecognized character"); return false; }
        tokens.push(t);
    }

    let mut arena = AstArena::new();
    let mut syms = SymbolTable::new();
    let mut parser = Parser { tokens: &tokens, pos: 0, arena: &mut arena, syms: &mut syms };
    
    let root_block = parser.parse_program();
    if root_block.is_none() {
        set_err("Parser Error: Invalid syntax");
        return false;
    }

    let mut s_log = String::new();
    for i in 0..syms.count { s_log.push_str(&alloc::format!("['{}'->loc {}] ", syms.syms[i].name, syms.syms[i].local_idx)); }
    let s_log_bytes = s_log.as_bytes();
    let cpy_len = core::cmp::min(s_log_bytes.len(), 1024);
    SYM_TABLE_LOG[..cpy_len].copy_from_slice(&s_log_bytes[..cpy_len]);
    SYM_TABLE_LOG_LEN = cpy_len;

    let mut wasm: Vec<u8> = Vec::new();
    wasm.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d]); // Magic
    wasm.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // Version

    // Section 1: Type
    wasm.extend_from_slice(&[0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f]);
    // Section 3: Function
    wasm.extend_from_slice(&[0x03, 0x02, 0x01, 0x00]);
    
    // NEW: Section 5: Memory (1 Page = 64KB)
    wasm.push(0x05); // Section ID
    wasm.push(0x03); // Section Size
    wasm.push(0x01); // 1 memory item
    wasm.push(0x00); // flags: no maximum
    wasm.push(0x01); // 1 page minimum (64KB RAM)

    // Section 7: Export
    let export_name = b"run_skill";
    wasm.push(0x07); 
    wasm.push((4 + export_name.len()) as u8); 
    wasm.push(0x01); 
    wasm.push(export_name.len() as u8);
    wasm.extend_from_slice(export_name);
    wasm.extend_from_slice(&[0x00, 0x00]); 

    // Section 10: Code
    let mut code_body: Vec<u8> = Vec::new();
    if syms.count > 0 {
        code_body.push(0x01); 
        emit_leb128(syms.count as u32, &mut code_body); 
        code_body.push(0x7F); 
    } else {
        code_body.push(0x00);
    }

    traverse_ast(root_block.unwrap(), &arena, &syms, &mut code_body);
    code_body.push(0x0B); 

    wasm.push(0x0A); 
    wasm.push((code_body.len() + 2) as u8); 
    wasm.push(0x01); 
    emit_leb128(code_body.len() as u32, &mut wasm);
    wasm.extend_from_slice(&code_body);

    if wasm.len() <= MAX_WASM_LEN {
        COMPILED_WASM[..wasm.len()].copy_from_slice(&wasm);
        WASM_LEN = wasm.len();
        COMPILATION_SUCCESS = true;
        log(&alloc::format!("[Compiler] V6.0 Pointer & Memory Compilation Success. Emitted {} bytes.", WASM_LEN));
        return true;
    }
    set_err("Compiler Error: Wasm binary too large");
    false
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
// UI Layout Engine
// -----------------------------------------------------------------------------
struct LayoutContext {
    current_y: u32,
    margin_x: u32,
    content_width: u32,
    inline_x: u32,
}

static mut NEXT_GUI_ID: u32 = 500;

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
    
    if node.class_attr == "pane-src" { ctx.margin_x = 20; ctx.content_width = 470; ctx.current_y = 80; ctx.inline_x = 0; }
    if node.class_attr == "pane-ast" { ctx.margin_x = 20; ctx.content_width = 470; ctx.current_y = 450; ctx.inline_x = 0; }
    if node.class_attr == "pane-wasm" { ctx.margin_x = 510; ctx.content_width = 470; ctx.current_y = 80; ctx.inline_x = 0; }

    match tag_lower {
        "h1" => { style = STYLE_ACCENT; height = 40; indent = 0; }
        "h2" | "h3" => { style = STYLE_ACCENT; height = 30; indent = 0; }
        "span" => { height = 20; indent = 0; }
        "code" => { height = 20; indent = 0; }
        _ => {}
    }

    if node.class_attr.contains("os-chrome") { style = STYLE_CHROME; }
    else if node.class_attr.contains("os-sidebar") { style = STYLE_SIDEBAR; }
    else if node.class_attr.contains("os-accent") { style = STYLE_ACCENT; }
    else if node.class_attr.contains("os-safe") { style = STYLE_SAFE; }
    else if node.class_attr.contains("os-warning") { style = STYLE_WARNING; }
    else if node.class_attr.contains("os-critical") { style = STYLE_CRITICAL; } // Pointer red
    else if node.class_attr.contains("hex-text") { style = STYLE_HEX_TEXT; }

    if node.class_attr == "pane-src" || node.class_attr == "pane-ast" || node.class_attr == "pane-wasm" {
        let bg_id = NEXT_GUI_ID; NEXT_GUI_ID += 1;
        let h = if node.class_attr == "pane-wasm" { 620 } else if node.class_attr == "pane-src" { 350 } else { 250 };
        os_gui_set_object_bounds(bg_id, win_x + ctx.margin_x, ctx.current_y, ctx.content_width, h);
        os_gui_set_object_style(bg_id, STYLE_CODE_BG);
        os_gui_set_object_label(bg_id, b"".as_ptr(), 0);
    }

    let is_inline = tag_lower == "span" || tag_lower == "text";

    if !node.text.is_empty() {
        let obj_id = NEXT_GUI_ID; NEXT_GUI_ID += 1;
        let mut final_x = win_x + ctx.margin_x + indent;
        if is_inline { final_x += ctx.inline_x; }

        let text_w = core::cmp::max(16, (node.text.len() as u32) * 9); 
        
        os_gui_set_object_bounds(obj_id, final_x, ctx.current_y, text_w, height);
        os_gui_set_object_style(obj_id, style);
        os_gui_set_object_label(obj_id, node.text.as_ptr(), node.text.len() as u32);
        
        if is_inline { ctx.inline_x += text_w; } else { ctx.current_y += height + 8; ctx.inline_x = 0; }
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

// Generate the HTML based on Compilation State dynamically
unsafe fn generate_ui_html() -> String {
    let mut html = String::new();
    html.push_str("<h1 class='os-accent'>Rhiza V6.0 Low-Level Compiler (Pointers & Memory)</h1>");
    
    // 1. TOP-LEFT: Source Code
    html.push_str("<div class='pane-src'>");
    html.push_str("<h3 class='os-sidebar'>Stage 1: Pointer & Linear Memory Input</h3><br/>");
    let src = core::str::from_utf8_unchecked(&SOURCE_CODE[..SOURCE_LEN]);
    for line in src.lines() { html.push_str(&alloc::format!("<code><span class='os-accent'>{}</span></code>", line)); }
    html.push_str("</div>");

    // 2. BOT-LEFT: AST & Symbol Table
    html.push_str("<div class='pane-ast'>");
    html.push_str("<h3 class='os-sidebar'>Stage 2: Memory Resolution Map</h3><br/>");
    let s_log = core::str::from_utf8_unchecked(&SYM_TABLE_LOG[..SYM_TABLE_LOG_LEN]);
    html.push_str(&alloc::format!("<code><span class='os-safe'>Local Pointers: {}</span></code><br/>", s_log));
    
    if !COMPILATION_SUCCESS {
        let err = core::str::from_utf8_unchecked(&COMPILATION_MSG[..COMPILATION_MSG_LEN]);
        html.push_str(&alloc::format!("<code><span class='os-warning'>{}</span></code>", err));
    } else {
        html.push_str("<code><span class='os-sidebar'>Memory Graph successfully built.</span></code>");
    }
    html.push_str("</div>");

    // 3. RIGHT PANE: Wasm Emitter
    html.push_str("<div class='pane-wasm'>");
    if COMPILATION_SUCCESS {
        html.push_str("<h3 class='os-safe'>Stage 3: Emitted Wasm (Red = Memory Opcodes)</h3><br/>");
        
        for chunk in COMPILED_WASM[..WASM_LEN].chunks(16) {
            html.push_str("<code>");
            for &b in chunk {
                let css = if b == 0x05 { "os-critical" } // Memory Section Highlight
                          else if b == 0x36 || b == 0x28 { "os-critical" } // i32.store, i32.load
                          else if b == 0x00 || b == 0x61 || b == 0x73 || b == 0x6D { "os-warning" } 
                          else if b == 0x20 || b == 0x21 { "os-safe" } 
                          else { "hex-text" };
                html.push_str(&alloc::format!("<span class='{}'>{:02X} </span>", css, b));
            }
            html.push_str("</code>");
        }
        html.push_str(&alloc::format!("<br/><p class='os-sidebar'>Compiled {} bytes. Memory Section (64KB RAM) Included.</p>", WASM_LEN));
    } else {
        html.push_str("<h3 class='os-warning'>Compilation Failed</h3><br/>");
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
pub extern "C" fn agent_cmd_compile_code(ptr: *const u8, len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(ptr, len as usize);
        let cpy_len = core::cmp::min(slice.len(), MAX_SRC_LEN);
        SOURCE_CODE[..cpy_len].copy_from_slice(&slice[..cpy_len]);
        SOURCE_LEN = cpy_len;
        
        log("[Compiler] V6.0 Pointer Compilation Initiated...");
        COMPILATION_SUCCESS = compile_micro_rust();
        render_dashboard();
        
        if COMPILATION_SUCCESS { return 1; }
        0
    }
}

#[no_mangle]
pub extern "C" fn agent_eval_js(script_ptr: *const u8, script_len: u32) -> u32 {
    unsafe {
        let slice = core::slice::from_raw_parts(script_ptr, script_len as usize);
        if let Ok(script) = core::str::from_utf8(slice) {
            let s = script.trim();
            if s.starts_with("compile(") {
                let inner = s[8..].trim_end_matches(|c| c == ')' || c == ';' || c == '\'' || c == '"');
                agent_cmd_compile_code(inner.as_ptr(), inner.len() as u32);
            }
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log("[Rhiza-Compiler] V6.0 Pointer & Memory AOT Compiler Initialized.");
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log("[Rhiza-Compiler] Booting Pointer Compilation Dashboard...");
    unsafe { 
        os_request_display_mode(2, 0, 0, 1024, 768); 
        // V6.0 Extreme Test: Pointer manipulation!
        let mock_src = "fn run_skill() -> i32 {\n  let ptr = 1024;\n  *ptr = 42;\n  let val = *ptr;\n  return val;\n}";
        SOURCE_CODE[..mock_src.len()].copy_from_slice(mock_src.as_bytes());
        SOURCE_LEN = mock_src.len();
        COMPILATION_SUCCESS = compile_micro_rust();
        
        render_dashboard();
    }
    1
}
