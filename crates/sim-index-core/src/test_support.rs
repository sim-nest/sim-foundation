use sim_kernel::{Value, card::Card};

pub(crate) fn card(value: &Value) -> &Card {
    value.object().downcast_ref::<Card>().expect("index card")
}

pub(crate) fn assert_card_entry(
    value: &Value,
    name: &str,
    expected: &str,
    cx: &mut sim_kernel::Cx,
) {
    let (_, value) = card(value)
        .entries()
        .iter()
        .find(|(symbol, _)| symbol.as_qualified_str() == name)
        .unwrap_or_else(|| panic!("missing card entry {name}"));
    assert_eq!(value.object().display(cx).expect("entry display"), expected);
}
