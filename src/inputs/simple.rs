fn main() {
    if_chain! {
        if x == 1;
        if let Some(y) = f();
        then {
            stuff
        } else {
            other + stuff
        }
    }
}
