# if-to-let-chain

Converts usages of the `if_chain` macro from the
[if-chain crate](https://docs.rs/if_chain/) to `let-chains`.

Example input:

```Rust
if_chain! {
    if let Ok(num) = u16::from_str(s);
    if num < 4000;
    if let Some(e) = v.get(num);
    then {
        println!("{e}");
    }
}
```

Output:

```Rust
if let Ok(num) = u16::from_str(s)
    && num < 4000
    && let Some(e) = v.get(num)
{
    println!("{e}");
}
```

Usage:

```
if-to-let-chain [Options] PATH...

Options:
    -d, --deindent N    number of chars to deindent by (default 4)
    -v, --verbose       print extra information
    -h, --help          print this help
```

### License

This crate is distributed under the terms of both the MIT license
and the Apache License (Version 2.0), at your option.

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.

#### License of your contributions

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
