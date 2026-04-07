use ni_parser::*;

/// Intermediate representation for a coroutine after transformation.
#[derive(Debug, Clone)]
pub struct CoroutineIR {
    pub name: String,
    pub states: Vec<CoroutineState>,
    pub params: Vec<String>,
}

/// A single state in the state machine.
#[derive(Debug, Clone)]
pub struct CoroutineState {
    /// Variables hoisted to the state struct (live across yield points)
    pub hoisted_vars: Vec<String>,
    /// Code lines for this state (pre-generated as strings by the backend)
    pub code_lines: Vec<String>,
}

/// Identifies yield points in a function body.
pub fn find_yield_points(stmts: &[Statement]) -> Vec<usize> {
    let mut points = Vec::new();
    for (i, stmt) in stmts.iter().enumerate() {
        if contains_yield(stmt) {
            points.push(i);
        }
    }
    points
}

/// Check if a statement contains a yield, wait, or wait_until call.
fn contains_yield(stmt: &Statement) -> bool {
    match &stmt.kind {
        StmtKind::Expr(expr) => expr_contains_yield(expr),
        StmtKind::VarDecl(decl) => expr_contains_yield(&decl.initializer),
        StmtKind::ConstDecl(decl) => expr_contains_yield(&decl.initializer),
        StmtKind::Return(Some(expr)) => expr_contains_yield(expr),
        StmtKind::If(if_stmt) => {
            expr_contains_yield(&if_stmt.condition)
                || stmts_contain_yield(&if_stmt.then_body)
                || if_stmt
                    .elif_branches
                    .iter()
                    .any(|(c, b)| expr_contains_yield(c) || stmts_contain_yield(b))
                || if_stmt
                    .else_body
                    .as_ref()
                    .is_some_and(|b| stmts_contain_yield(b))
        }
        StmtKind::While(w) => expr_contains_yield(&w.condition) || stmts_contain_yield(&w.body),
        StmtKind::For(f) => expr_contains_yield(&f.iterable) || stmts_contain_yield(&f.body),
        StmtKind::Block(stmts) => stmts_contain_yield(stmts),
        StmtKind::Try(t) => {
            stmts_contain_yield(&t.body)
                || match &t.catch_body {
                    CatchBody::Block(stmts) => stmts_contain_yield(stmts),
                    CatchBody::Match(cases) => cases.iter().any(|c| stmts_contain_yield(&c.body)),
                }
        }
        _ => false,
    }
}

fn stmts_contain_yield(stmts: &[Statement]) -> bool {
    stmts.iter().any(contains_yield)
}

fn expr_contains_yield(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Call { callee, args, .. } => {
            if let ExprKind::Identifier(name) = &callee.kind {
                if name == "yield" || name == "wait" || name == "wait_until" {
                    return true;
                }
            }
            expr_contains_yield(callee) || args.iter().any(expr_contains_yield)
        }
        ExprKind::MethodCall { object, args, .. } => {
            expr_contains_yield(object) || args.iter().any(expr_contains_yield)
        }
        ExprKind::Yield(_) => true,
        ExprKind::Wait(_) => true,
        ExprKind::Spawn(inner) | ExprKind::Await(inner) => expr_contains_yield(inner),
        ExprKind::BinaryOp { left, right, .. } => {
            expr_contains_yield(left) || expr_contains_yield(right)
        }
        ExprKind::Compare { left, right, .. } => {
            expr_contains_yield(left) || expr_contains_yield(right)
        }
        ExprKind::And(l, r) | ExprKind::Or(l, r) => {
            expr_contains_yield(l) || expr_contains_yield(r)
        }
        ExprKind::Negate(inner) | ExprKind::Not(inner) => expr_contains_yield(inner),
        ExprKind::Assign { target, value } => {
            expr_contains_yield(target) || expr_contains_yield(value)
        }
        ExprKind::GetField(obj, _) => expr_contains_yield(obj),
        ExprKind::GetIndex(obj, idx) => expr_contains_yield(obj) || expr_contains_yield(idx),
        ExprKind::List(items) => items.iter().any(expr_contains_yield),
        ExprKind::Map(pairs) => pairs
            .iter()
            .any(|(k, v)| expr_contains_yield(k) || expr_contains_yield(v)),
        ExprKind::IfExpr {
            value,
            condition,
            else_value,
        } => {
            expr_contains_yield(value)
                || expr_contains_yield(condition)
                || expr_contains_yield(else_value)
        }
        _ => false,
    }
}

/// Collect all variable declarations in statements that appear before yield points.
/// These need to be hoisted into the state machine context.
pub fn find_hoisted_vars(stmts: &[Statement]) -> Vec<String> {
    let mut vars = Vec::new();
    let yield_points = find_yield_points(stmts);
    if yield_points.is_empty() {
        return vars;
    }

    for stmt in stmts {
        match &stmt.kind {
            StmtKind::VarDecl(decl) => {
                if !vars.contains(&decl.name) {
                    vars.push(decl.name.clone());
                }
            }
            StmtKind::ConstDecl(decl) => {
                if !vars.contains(&decl.name) {
                    vars.push(decl.name.clone());
                }
            }
            _ => {}
        }
    }
    vars
}

/// Build the coroutine IR by splitting a function body at yield points.
pub fn build_coroutine_ir(
    name: &str,
    params: &[Param],
    stmts: &[Statement],
) -> Option<CoroutineIR> {
    let yield_points = find_yield_points(stmts);
    if yield_points.is_empty() {
        return None;
    }

    let hoisted = find_hoisted_vars(stmts);
    let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();

    // Split statements at yield points
    let mut states = Vec::new();
    let mut current_start = 0;

    for &yield_idx in &yield_points {
        // Create state for statements before this yield
        states.push(CoroutineState {
            hoisted_vars: hoisted.clone(),
            code_lines: Vec::new(), // Will be filled by the backend
        });
        current_start = yield_idx + 1;
    }

    // Final state after last yield
    if current_start <= stmts.len() {
        states.push(CoroutineState {
            hoisted_vars: hoisted.clone(),
            code_lines: Vec::new(),
        });
    }

    Some(CoroutineIR {
        name: name.to_string(),
        states,
        params: param_names,
    })
}
