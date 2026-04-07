use ni_error::Span;

#[derive(Debug, Clone)]
pub struct Program {
    pub declarations: Vec<Declaration>,
    pub docstring: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub kind: DeclKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DeclKind {
    Var(VarDecl),
    Const(ConstDecl),
    Fun(FunDecl),
    Class(ClassDecl),
    Enum(EnumDecl),
    Import(ImportDecl),
    Spec(SpecDecl),
    Statement(Statement),
}

#[derive(Debug, Clone)]
pub struct SpecDecl {
    pub name: String,
    pub body: Vec<Statement>,       // flat spec (existing)
    pub sections: Vec<SpecSection>, // structured given/when/then
    pub each: Option<EachClause>,   // data-driven
}

#[derive(Debug, Clone)]
pub struct SpecSection {
    pub kind: SpecSectionKind,
    pub label: String,
    pub body: Vec<Statement>,
    pub children: Vec<SpecSection>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpecSectionKind {
    Given,
    When,
    Then,
}

#[derive(Debug, Clone)]
pub struct EachClause {
    pub items: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub type_ann: Option<TypeAnnotation>,
    pub initializer: Expr,
}

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub type_ann: Option<TypeAnnotation>,
    pub initializer: Expr,
}

#[derive(Debug, Clone)]
pub struct FunDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeAnnotation>,
    pub body: Vec<Statement>,
    pub docstring: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub type_ann: Option<TypeAnnotation>,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub name: String,
    pub superclass: Option<String>,
    pub docstring: Option<String>,
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<MethodDecl>,
    pub static_fields: Vec<FieldDecl>,
    pub static_methods: Vec<FunDecl>,
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub name: String,
    pub type_ann: Option<TypeAnnotation>,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct MethodDecl {
    pub fun: FunDecl,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<Expr>,
}

// Imports
#[derive(Debug, Clone)]
pub enum ImportDecl {
    Module {
        path: Vec<String>,
        alias: Option<String>,
    },
    From {
        path: Vec<String>,
        names: Vec<ImportName>,
    },
    FromAll {
        path: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ImportName {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Expr(Expr),
    VarDecl(VarDecl),
    ConstDecl(ConstDecl),
    If(IfStmt),
    While(WhileStmt),
    For(ForStmt),
    Match(MatchStmt),
    Return(Option<Expr>),
    Break,
    Continue,
    Pass,
    Block(Vec<Statement>),
    Try(TryStmt),
    Fail(Expr),
    Assert(Expr, Option<Expr>),
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_body: Vec<Statement>,
    pub elif_branches: Vec<(Expr, Vec<Statement>)>,
    pub else_body: Option<Vec<Statement>>,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub variable: String,
    pub second_var: Option<String>,
    pub iterable: Expr,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct MatchStmt {
    pub subject: Expr,
    pub cases: Vec<MatchCase>,
}

#[derive(Debug, Clone)]
pub struct MatchCase {
    pub patterns: Vec<Pattern>,
    pub guard: Option<Expr>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(Expr),
    Wildcard,
    Binding(String),
    TypeCheck(String, String), // binding, type_name
}

#[derive(Debug, Clone)]
pub enum CatchBody {
    Block(Vec<Statement>),
    Match(Vec<MatchCase>),
}

#[derive(Debug, Clone)]
pub struct TryStmt {
    pub body: Vec<Statement>,
    pub catch_var: Option<String>,
    pub catch_body: CatchBody,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    FStringLiteral(Vec<FStringPart>),
    BoolLiteral(bool),
    NoneLiteral,
    Identifier(String),
    SelfExpr,

    // Unary
    Negate(Box<Expr>),
    Not(Box<Expr>),

    // Binary
    BinaryOp {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },

    // Logical (short-circuit)
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),

    // Comparison
    Compare {
        left: Box<Expr>,
        op: CmpOp,
        right: Box<Expr>,
    },

    // Assignment
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
    },
    CompoundAssign {
        target: Box<Expr>,
        op: BinOp,
        value: Box<Expr>,
    },

    // Access
    GetField(Box<Expr>, String),
    SetField(Box<Expr>, String, Box<Expr>),
    GetIndex(Box<Expr>, Box<Expr>),
    SetIndex(Box<Expr>, Box<Expr>, Box<Expr>),
    SafeNav(Box<Expr>, String),         // ?.
    NoneCoalesce(Box<Expr>, Box<Expr>), // ??

    // Call
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        named_args: Vec<(String, Expr)>,
    },
    MethodCall {
        object: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        named_args: Vec<(String, Expr)>,
    },

    // Super
    SuperCall {
        method: String,
        args: Vec<Expr>,
    },

    // Collections
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),

    // Range
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },

    // Lambda
    Lambda {
        params: Vec<Param>,
        body: Vec<Statement>,
    },

    // Ternary
    IfExpr {
        value: Box<Expr>,
        condition: Box<Expr>,
        else_value: Box<Expr>,
    },

    // Spawn
    Spawn(Box<Expr>),

    // Yield -- suspend fiber, optionally with a value
    Yield(Option<Box<Expr>>),

    // Wait -- suspend fiber for N seconds
    Wait(Box<Expr>),

    // Await
    Await(Box<Expr>),

    // Try expression: try <expr> -- returns none on fail
    TryExpr(Box<Expr>),

    // Fail expression: fail <expr> -- throws value (for use inside try-expr)
    FailExpr(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CmpOp {
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Is,
    In,
}

#[derive(Debug, Clone)]
pub enum FStringPart {
    Literal(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct TypeAnnotation {
    pub name: String,
    pub optional: bool,
    pub type_args: Vec<TypeAnnotation>,
}
