use std::{io::Cursor, rc::Rc};

use soroban_env_host::{
    xdr::{ReadXdr, ScSpecEntry, ScSpecFunctionV0},
    Vm,
};

pub fn function_spec(vm: &Rc<Vm>, name: &str) -> Option<ScSpecFunctionV0> {
    let spec = vm.custom_section("contractspecv0")?;
    let mut cursor = Cursor::new(spec);
    for spec_entry in ScSpecEntry::read_xdr_iter(&mut cursor).flatten() {
        if let ScSpecEntry::FunctionV0(f) = spec_entry {
            if let Ok(n) = f.name.to_string() {
                if n == name {
                    return Some(f);
                }
            }
        }
    }
    None
}
