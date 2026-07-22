use sim::sim_lib;

#[path = "module_body.rs"]
#[sim_lib(id = "non-inline-module", version = "0.1.0")]
mod non_inline_module;
