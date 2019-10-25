use wasmer_runtime_core:: {
    wasmparser::Operator
};

pub fn get_costs_table(table_name: &str) -> fn(&Operator) -> u64 {
    match table_name {
        "uniform_one" => { uniform_one }
        "expensive_loop_else_one" => { expensive_loop_else_one }
        "expensive_branching_else_one" => { expensive_branching_else_one }
        _ => { uniform_zero }
    }
}

fn expensive_branching_else_one(op: &Operator) -> u64 {
    match *op {
        Operator::Loop { .. } 
        | Operator::Br { .. }
        | Operator::BrTable { .. }
        | Operator::BrIf { .. }
        | Operator::Call { .. }
        | Operator::CallIndirect { .. }
        | Operator::Return => { 12 }
        _ => { 1 }
    }
}

fn expensive_loop_else_one(op: &Operator) -> u64 {
    match *op {
        Operator::Loop { .. } => { 12 }
        _ => { 1 }
    }
}

fn uniform_one(_op: &Operator) -> u64 {
    1
}
 
fn uniform_zero(_op: &Operator) -> u64 {
    0
}
