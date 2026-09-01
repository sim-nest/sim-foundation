#![allow(deprecated)]

use std::sync::Arc;

use sim::{
    case,
    kernel::{DefaultFactory, EagerPolicy, HandleSeed, Lib, Symbol},
    sim_fn, sim_lib,
};

#[sim_lib(id = "consumer-smoke", version = "0.1.0")]
mod consumer_smoke {
    use super::{case, sim_fn};

    #[sim_fn(name = "negate")]
    #[case(args = "((capture value Number))", result = "Number")]
    pub fn negate(value: f64) -> f64 {
        -value
    }
}

#[test]
fn generated_manifest_and_load_paths_work() {
    let lib = consumer_smoke::ConsumerSmokeLib;
    let manifest = Lib::manifest(&lib);
    assert_eq!(manifest.id, Symbol::new("consumer-smoke"));
    assert!(
        manifest
            .exports
            .iter()
            .any(|export| export.symbol() == &Symbol::new("negate"))
    );
    assert!(consumer_smoke::__SIM_LIB_EXPANSION.contains("ConsumerSmokeLib"));

    let mut cx = sim::kernel::Cx::new(
        Arc::new(EagerPolicy),
        Arc::new(DefaultFactory),
        HandleSeed::new(0),
    );
    cx.load_lib(&lib).unwrap();
    assert!(cx.resolve_function(&Symbol::new("negate")).is_ok());
}
