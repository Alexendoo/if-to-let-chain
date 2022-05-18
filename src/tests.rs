use insta::{Settings, assert_snapshot};
use std::fs;

macro_rules! snaps {
    ($($file:ident)*) => {
        $(
            #[test]
            fn $file() {
                let path = concat!("src/inputs/", stringify!($file), ".rs");
                let mut settings = Settings::new();
                settings.set_prepend_module_to_snapshot(false);
                settings.set_input_file(path);
                settings.bind(|| {
                    let mut contents = fs::read_to_string(path).unwrap();
                    super::modify(&mut contents, 4, path);
                    assert_snapshot!(contents);
                });
            }
        )*
    };
}

snaps!(
    closure
    comment
    or
    simple
);
