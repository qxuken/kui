//! `#[derive(Message)]` reaching kui through `kui_value` alone.
#![cfg(feature = "derive")]

use kui_value::{Message, Value};

#[derive(Message, Clone, Debug, PartialEq)]
#[message(crate = "kui_value")]
enum Msg {
    Save,
    Rename { to: String },
}

#[test]
fn a_message_round_trips_with_no_core() {
    let v: Value = Msg::Rename { to: "b".into() }.into();
    assert_eq!(v.get_str("kind"), Some("rename"));
    assert_eq!(Msg::try_from(&v), Ok(Msg::Rename { to: "b".into() }));
    assert_eq!(Msg::try_from(&Value::from(Msg::Save)), Ok(Msg::Save));
}
