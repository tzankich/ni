#[allow(dead_code)]
use ni_error::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),

    // String interpolation
    FStringStart(String),
    FStringMiddle(String),
    FStringEnd(String),

    // Identifier & keywords
    Identifier(String),

    // Keywords - Declarations
    Var,
    Const,
    Fun,
    Class,
    Extends,
    Enum,
    Import,
    From,
    As,
    Return,
    Static,
    Super,
    Spec,
    Given, // spec sub-block
    When,  // spec sub-block
    Then,  // spec sub-block
    Each,  // data-driven iteration in spec

    // Keywords - Control flow
    If,
    Elif,
    Else,
    For,
    In,
    While,
    Break,
    Continue,
    Match,
    Case,
    Pass,

    // Keywords - Error handling
    Try,
    Catch,
    Fail,
    Assert,

    // Keywords - Coroutines
    Yield,
    Wait,
    Spawn,
    Fiber,

    // Keywords - Reserved for future use
    Trait,
    Abstract,
    Private,
    Defer,
    Async,
    Await,
    Type,

    // Keywords - Operators & values
    And,
    Or,
    Not,
    Is,
    True,
    False,
    None,
    SelfKw,

    // Single-character tokens
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Colon,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Underscore,
    At,

    // Multi-character tokens
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    Equal,
    EqualEqual,
    Bang,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    ColonEqual,       // :=
    Arrow,            // ->
    DotDot,           // ..
    DotDotEqual,      // ..=
    QuestionDot,      // ?.
    QuestionQuestion, // ??

    // Structural
    Newline,
    Indent,
    Dedent,

    // End of file
    Eof,
}

impl TokenKind {
    pub fn is_newline(&self) -> bool {
        matches!(self, TokenKind::Newline)
    }
}
