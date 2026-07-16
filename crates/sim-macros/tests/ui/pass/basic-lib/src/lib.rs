use sim::{case, sim_fn, sim_lib};

#[sim_lib(id = "basic-lib", version = "0.1.0")]
mod basic_lib {
    use super::{case, sim_fn};

    #[sim_fn(name = "negate")]
    #[case(args = "((capture value Number))", result = "Number")]
    pub fn negate(value: f64) -> f64 {
        -value
    }
}
