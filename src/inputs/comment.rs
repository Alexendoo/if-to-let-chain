fn f() {
    if_chain! {
        // comment at the start TODO: fix or warn?
        if true;
        // comment middle
        if false;
        // comment end
        then {
            1;
        }
        // comment inbetween? :o
        else {
            2;
        }
        // comment after
    }
}
