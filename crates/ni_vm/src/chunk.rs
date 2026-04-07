use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum OpCode {
    // Stack
    Constant, // u16 index
    None,
    True,
    False,
    Pop,
    Dup,

    // Variables
    GetLocal,     // u8 slot
    SetLocal,     // u8 slot
    GetGlobal,    // u16 name index
    SetGlobal,    // u16 name index
    DefineGlobal, // u16 name index

    // Upvalues
    GetUpvalue, // u8 slot
    SetUpvalue, // u8 slot
    CloseUpvalue,

    // Arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Negate,

    // Comparison
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,

    // Logic
    Not,
    // And/Or handled via jumps

    // Control flow
    Jump,        // u16 offset
    JumpIfFalse, // u16 offset
    JumpIfTrue,  // u16 offset
    Loop,        // u16 offset (jump backward)

    // Functions
    Call, // u8 arg count
    Return,
    Closure, // u16 fn constant, then upvalue descriptors

    // Properties
    GetProperty, // u16 name index
    SetProperty, // u16 name index
    GetIndex,
    SetIndex,

    // Classes
    Class,  // u16 name index
    Method, // u16 name index
    Inherit,
    GetSuper,    // u16 name index
    Invoke,      // u16 name, u8 args
    SuperInvoke, // u16 name, u8 args

    // Collections
    BuildList, // u16 count
    BuildMap,  // u16 count (pairs)

    // Range
    BuildRange, // u8: 0=exclusive, 1=inclusive

    // Iterator
    GetIterator,
    IteratorNext, // u16 jump offset (jumps when done)

    // Fibers
    SpawnFiber,
    Yield,
    Wait,
    Await,

    // String interpolation
    StringConcat, // u16 count

    // Safe navigation
    SafeNav,      // u16 name index
    NoneCoalesce, // u16 jump offset

    // Error handling
    Fail,            // pop message, raise error
    SetCatchPoint,   // u16 handler_offset -- records catch point for try-expr
    ClearCatchPoint, // removes most recent catch point

    // Special
    Print,     // built-in print
    AssertOp,  // assert with optional message
    AssertCmp, // rich assert: pops condition, right, left; u8 cmp_op tag

    // Docstrings
    SetDocstring, // pops string, peeks class on stack, sets class.docstring
}

#[derive(Debug, Clone)]
pub struct ExceptionEntry {
    pub try_start: usize,
    pub try_end: usize,
    pub handler_ip: usize,
    pub stack_depth: usize,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub lines: Vec<usize>, // line number for each byte
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
        }
    }

    pub fn write(&mut self, byte: u8, line: usize) {
        self.code.push(byte);
        self.lines.push(line);
    }

    pub fn write_op(&mut self, op: OpCode, line: usize) {
        self.write(op as u8, line);
    }

    pub fn write_u16(&mut self, value: u16, line: usize) {
        self.write((value >> 8) as u8, line);
        self.write((value & 0xff) as u8, line);
    }

    pub fn read_u16(&self, offset: usize) -> u16 {
        let hi = self.code.get(offset).copied().unwrap_or(0) as u16;
        let lo = self.code.get(offset + 1).copied().unwrap_or(0) as u16;
        (hi << 8) | lo
    }

    pub fn add_constant(&mut self, value: Value) -> u16 {
        self.constants.push(value);
        (self.constants.len() - 1) as u16
    }

    pub fn patch_jump(&mut self, offset: usize) {
        let jump = self.code.len() - offset - 2;
        self.code[offset] = (jump >> 8) as u8;
        self.code[offset + 1] = (jump & 0xff) as u8;
    }

    pub fn current_offset(&self) -> usize {
        self.code.len()
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}
