fn f() {
    // pre
    if_chain! {
        // comment at the start
        if true;
        // comment middle
        if false;
        // comment end
        then {
            1;
        }
        // comment inbetween
        else {
            2;
        }
        // comment after
    }
    // post
}
