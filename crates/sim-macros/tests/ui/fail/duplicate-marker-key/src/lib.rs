use sim::{sim_codec, sim_lib};

#[sim_lib(id = "duplicate-marker-key", version = "0.1.0")]
mod duplicate_marker_key {
    use super::sim_codec;

    #[sim_codec(
        symbol = "codec/first",
        symbol = "codec/second",
        decode = "decode_mock",
        encode = "encode_mock"
    )]
    pub fn duplicate_codec() {}

    pub fn decode_mock(text: String) -> sim::kernel::Expr {
        sim::kernel::Expr::String(text)
    }

    pub fn encode_mock(expr: sim::kernel::Expr) -> String {
        format!("{expr:?}")
    }
}
