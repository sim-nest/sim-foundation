use sim::sim_lib;

#[sim_lib(id = "one", id = "two", version = "0.1.0")]
mod duplicate_lib_id {}
