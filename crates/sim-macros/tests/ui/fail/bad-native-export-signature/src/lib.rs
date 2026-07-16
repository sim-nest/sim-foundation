use sim::{case, sim_class, sim_constructor, sim_fn, sim_lib};

#[sim_lib(id = "bad-native-export", version = "0.1.0", native_export = true)]
mod bad_native_export {
    use super::{case, sim_class, sim_constructor, sim_fn};

    #[sim_class(name = "Point")]
    pub struct Point {
        x: f64,
    }

    #[sim_constructor(class = "Point")]
    #[case(args = "((capture x Number))", result = "Point")]
    pub fn point(x: f64) -> Point {
        Point { x }
    }

    #[sim_fn(name = "touch")]
    #[case(args = "((capture point Point))", result = "Number")]
    pub fn touch(point: &Point) -> f64 {
        point.x
    }
}
