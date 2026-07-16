use sim::{case, sim_fn, sim_lib};

#[sim_lib(id = "broken-shape", version = "0.1.0")]
mod broken_shape {
    use super::{case, sim_fn};

    #[sim_fn(name = "oops")]
    #[case(args = "((capture value Number)", result = "Number")]
    pub fn oops(value: f64) -> f64 {
        value
    }
}
