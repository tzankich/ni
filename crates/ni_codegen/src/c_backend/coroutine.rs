use super::CCodeGen;
use crate::coroutine_transform::CoroutineIR;

/// Emit a C state machine for a coroutine function.
#[allow(dead_code)]
pub(crate) fn emit_coroutine_c(gen: &mut CCodeGen, ir: &CoroutineIR) {
    let name = &ir.name;

    // Emit state enum
    gen.definitions.push_str("typedef enum {\n");
    for (i, _state) in ir.states.iter().enumerate() {
        gen.definitions
            .push_str(&format!("    {}_STATE_{},\n", name.to_uppercase(), i));
    }
    gen.definitions
        .push_str(&format!("    {}_STATE_FINISHED,\n", name.to_uppercase()));
    gen.definitions.push_str(&format!("}} {}_State;\n\n", name));

    // Emit context struct for hoisted variables
    gen.definitions.push_str("typedef struct {\n");
    gen.definitions
        .push_str(&format!("    {}_State state;\n", name));
    // Collect all hoisted variables across states
    let mut all_vars: Vec<String> = Vec::new();
    for state in &ir.states {
        for var in &state.hoisted_vars {
            if !all_vars.contains(var) {
                all_vars.push(var.clone());
            }
        }
    }
    for var in &all_vars {
        gen.definitions.push_str(&format!("    NiValue {};\n", var));
    }
    gen.definitions
        .push_str(&format!("}} {}_Context;\n\n", name));

    // Emit step function using switch
    gen.definitions.push_str(&format!(
        "typedef struct {{ NiValue value; int done; }} {}_FiberResult;\n\n",
        name
    ));

    gen.definitions.push_str(&format!(
        "{}_FiberResult ni_fiber_{}_step(NiVm* vm, {}_Context* ctx) {{\n",
        name, name, name
    ));
    gen.definitions.push_str("    switch (ctx->state) {\n");

    for (i, state) in ir.states.iter().enumerate() {
        gen.definitions
            .push_str(&format!("    case {}_STATE_{}:\n", name.to_uppercase(), i));

        for line in &state.code_lines {
            gen.definitions.push_str(&format!("        {}\n", line));
        }

        if i + 1 < ir.states.len() {
            gen.definitions.push_str(&format!(
                "        ctx->state = {}_STATE_{};\n",
                name.to_uppercase(),
                i + 1
            ));
            gen.definitions.push_str(&format!(
                "        return ({}_FiberResult){{ NI_NONE, 0 }};\n",
                name
            ));
        } else {
            gen.definitions.push_str(&format!(
                "        ctx->state = {}_STATE_FINISHED;\n",
                name.to_uppercase()
            ));
            gen.definitions.push_str(&format!(
                "        return ({}_FiberResult){{ NI_NONE, 1 }};\n",
                name
            ));
        }
    }

    gen.definitions.push_str(&format!(
        "    case {}_STATE_FINISHED:\n",
        name.to_uppercase()
    ));
    gen.definitions.push_str("    default:\n");
    gen.definitions.push_str(&format!(
        "        return ({}_FiberResult){{ NI_NONE, 1 }};\n",
        name
    ));
    gen.definitions.push_str("    }\n");
    gen.definitions.push_str("}\n\n");
}
