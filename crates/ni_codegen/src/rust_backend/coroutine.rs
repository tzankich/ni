use super::RustCodeGen;
use crate::coroutine_transform::CoroutineIR;

/// Emit a Rust state machine for a coroutine function.
#[allow(dead_code)]
pub(crate) fn emit_coroutine_rust(gen: &mut RustCodeGen, ir: &CoroutineIR) {
    let name = &ir.name;

    // Emit the state enum
    gen.declarations.push_str("#[derive(Debug, Clone)]\n");
    gen.declarations
        .push_str(&format!("pub enum {}_State {{\n", name));
    for (i, state) in ir.states.iter().enumerate() {
        gen.declarations.push_str(&format!("    State{}", i));
        if !state.hoisted_vars.is_empty() {
            gen.declarations.push_str(" { ");
            let fields: Vec<String> = state
                .hoisted_vars
                .iter()
                .map(|v| format!("{}: NiValue", v))
                .collect();
            gen.declarations.push_str(&fields.join(", "));
            gen.declarations.push_str(" }");
        }
        gen.declarations.push_str(",\n");
    }
    gen.declarations.push_str("    Finished,\n");
    gen.declarations.push_str("}\n\n");

    // Emit the FiberResult type alias (if not already defined)
    gen.declarations
        .push_str(&format!("pub enum {}_FiberResult {{\n", name));
    gen.declarations
        .push_str(&format!("    Yielded(NiValue, {}_State),\n", name));
    gen.declarations.push_str("    Done(NiValue),\n");
    gen.declarations.push_str("}\n\n");

    // Emit the step function
    gen.declarations.push_str(&format!(
        "pub fn ni_fiber_{}_step(vm: &mut dyn NiVm, state: {}_State) -> NiResult<{}_FiberResult> {{\n",
        name, name, name
    ));
    gen.declarations.push_str("    match state {\n");

    for (i, state) in ir.states.iter().enumerate() {
        if state.hoisted_vars.is_empty() {
            gen.declarations
                .push_str(&format!("        {}_State::State{} => {{\n", name, i));
        } else {
            let vars: Vec<String> = state.hoisted_vars.iter().map(|v| v.to_string()).collect();
            gen.declarations.push_str(&format!(
                "        {}_State::State{} {{ {} }} => {{\n",
                name,
                i,
                vars.join(", ")
            ));
        }

        // Emit the code for this state
        for line in &state.code_lines {
            gen.declarations
                .push_str(&format!("            {}\n", line));
        }

        // Transition to next state
        if i + 1 < ir.states.len() {
            gen.declarations.push_str(&format!(
                "            Ok({n}_FiberResult::Yielded(NiValue::None, {n}_State::State{next}))\n",
                n = name,
                next = i + 1
            ));
        } else {
            gen.declarations.push_str(&format!(
                "            Ok({}_FiberResult::Done(NiValue::None))\n",
                name
            ));
        }

        gen.declarations.push_str("        }\n");
    }

    gen.declarations.push_str(&format!(
        "        {}_State::Finished => Ok({}_FiberResult::Done(NiValue::None)),\n",
        name, name
    ));
    gen.declarations.push_str("    }\n");
    gen.declarations.push_str("}\n\n");
}
