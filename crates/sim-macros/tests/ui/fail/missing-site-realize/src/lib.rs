use sim::{sim_lib, sim_site};

#[sim_lib(id = "missing-site-realize", version = "0.1.0")]
mod missing_site_realize {
    use super::sim_site;

    #[sim_site(symbol = "model/local")]
    pub fn local_site() {}
}
