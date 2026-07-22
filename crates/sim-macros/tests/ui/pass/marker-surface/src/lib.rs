use sim::{
    case, shape, sim_class, sim_codec, sim_constructor, sim_macro, sim_number_domain, sim_site,
    sim_lib,
};

#[sim_lib(id = "marker-surface", version = "0.1.0")]
mod marker_surface {
    use sim::kernel::{Expr, Symbol};

    use super::{
        case, shape, sim_class, sim_codec, sim_constructor, sim_macro, sim_number_domain,
        sim_site,
    };

    #[sim_class(name = "Point")]
    #[shape("(fields (:x Number) (:y Number))")]
    #[derive(Clone)]
    pub struct Point {
        x: f64,
        y: f64,
    }

    #[sim_constructor(class = "Point")]
    #[case(args = "((capture x Number) (capture y Number))", result = "Point")]
    pub fn point(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[sim_macro(symbol = "marker/echo", expand = "expand_echo")]
    pub fn echo_marker() {}

    pub fn expand_echo(input: Expr) -> Expr {
        input
    }

    #[sim_codec(symbol = "codec/mock", decode = "decode_mock", encode = "encode_mock")]
    pub fn mock_codec() {}

    pub fn decode_mock(text: String) -> Expr {
        Expr::String(text)
    }

    pub fn encode_mock(expr: Expr) -> String {
        match expr {
            Expr::String(text) => text,
            other => format!("{other:?}"),
        }
    }

    #[sim_number_domain(symbol = "numbers/mock", parse = "parse_mock", encode = "encode_mock_number")]
    pub fn mock_number_domain() {}

    pub fn parse_mock(text: String) -> Option<Expr> {
        Some(Expr::String(text))
    }

    pub fn encode_mock_number(expr: Expr) -> Option<Expr> {
        Some(expr)
    }

    #[sim_site(symbol = "model/local", realize = "realize_local")]
    pub fn local_site() {}

    pub fn realize_local(expr: Expr) -> Expr {
        match expr {
            Expr::Symbol(symbol) => Expr::Symbol(Symbol::qualified("realized", symbol.to_string())),
            other => other,
        }
    }
}
