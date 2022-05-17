use getopts::Options;
use proc_macro2::LineColumn;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::{env, process};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::visit::{visit_file, Visit};
use syn::{Block, Expr, Ident, Macro, Result, Token, Local};

#[derive(Debug)]
struct Statement {
    if_token: Option<Token![if]>,
    expr: Expr,
    semi: Token![;],
}

impl Statement {
    fn start(&self) -> LineColumn {
        let start_span = match self.if_token {
            Some(token) => token.span,
            None => self.expr.span(),
        };

        start_span.start()
    }
}

impl Parse for Statement {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            if_token: input.parse()?,
            expr: input.call(Expr::parse_without_eager_brace)?,
            semi: input.parse()?,
        })
    }
}

#[derive(Debug)]
struct Then {
    then_token: Ident,
    block: Block,
}

impl Parse for Then {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            then_token: input.parse()?,
            block: input.parse()?,
        })
    }
}

#[derive(Debug)]
struct Else {
    _else_token: Token![else],
    block: Block,
}

impl Parse for Else {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            _else_token: input.parse()?,
            block: input.parse()?,
        })
    }
}

#[derive(Debug)]
struct IfChain {
    statements: Vec<Statement>,
    then: Then,
    r#else: Option<Else>,
}

impl IfChain {
    fn end(&self) -> LineColumn {
        match &self.r#else {
            Some(r#else) => r#else.block.span().end(),
            None => self.then.block.span().end(),
        }
    }
}

impl Parse for IfChain {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut statements: Vec<Statement> = Vec::new();
        let mut then: Option<Then> = None;
        let mut r#else: Option<Else> = None;

        while !input.is_empty() {
            if input.peek(Ident) {
                then = Some(input.parse()?);
            } else if input.peek(Token![else]) {
                r#else = Some(input.parse()?);
            } else {
                statements.push(input.parse()?);
            }
        }

        Ok(Self {
            statements,
            then: then.unwrap(),
            r#else,
        })
    }
}

#[derive(Default, Debug)]
struct Visitor<'ast> {
    found: Option<(IfChain, &'ast Macro)>,
}

impl<'ast> Visit<'ast> for Visitor<'ast> {
    fn visit_macro(&mut self, mac: &'ast Macro) {
        if self.found.is_none()
            && mac
                .path
                .segments
                .last()
                .map_or(false, |x| x.ident == "if_chain")
        {
            self.found = Some((mac.parse_body().unwrap(), mac));
        }
    }
}

fn replace_chars(line: &mut String, with: &str, char_start: usize, char_end: usize) {
    let (byte_start, _) = line.char_indices().nth(char_start).unwrap();
    let byte_end = match line.char_indices().nth(char_end) {
        Some((byte_end, _)) => byte_end,
        None => line.len(),
    };

    line.replace_range(byte_start..byte_end, with);
}

fn if_to_let_chain(input: &str, deindent: usize) -> Option<String> {
    let file = syn::parse_file(input).ok()?;

    let mut visitor = Visitor::default();
    visit_file(&mut visitor, &file);

    let (if_chain, mac) = visitor.found?;

    let mut lines: Vec<String> = input.lines().map(String::from).collect();

    for statement in &if_chain.statements {
        let semi = statement.semi.span.start();

        replace_chars(&mut lines[semi.line - 1], "", semi.column, semi.column + 1);
    }

    let (first, rest) = if_chain.statements.split_first().unwrap();

    for statement in rest {
        replace_chars(
            &mut lines[statement.start().line - 1],
            "&& ",
            statement.start().column,
            statement.expr.span().start().column,
        );
    }

    let (start, mut end) = {
        let then_span = if_chain.then.then_token.span();
        let brace_span = if_chain.then.block.brace_token.span;

        let start = then_span.start().line - 1;
        let end = brace_span.end().line;

        replace_chars(
            &mut lines[start],
            "",
            then_span.start().column,
            brace_span.start().column,
        );

        (start, end)
    };

    if let Some(r#else) = &if_chain.r#else {
        end = r#else.block.brace_token.span.end().line;
    }

    for line in start..end {
        let line = &mut lines[line];
        if line.len() > deindent {
            replace_chars(line, "", 0, deindent);
        }
    }

    let delete = {
        let mac_line = mac.span().start().line - 1;
        let mac_col = mac.span().start().column;

        let stmt_line = first.start().line - 1;
        let stmt_col = first.start().column;

        let mut stmt_str = lines[stmt_line].clone();
        replace_chars(
            &mut stmt_str,
            if first.if_token.is_some() { "" } else { "if " },
            0,
            stmt_col,
        );

        replace_chars(&mut lines[mac_line], &stmt_str, mac_col, usize::MAX);
        stmt_line
    };

    {
        let penultimate = if_chain.end().line - 1;
        let penultimate_str = lines[penultimate].clone();

        let last = mac.span().end().line - 1;

        let mut line = &mut lines[last];

        replace_chars(
            &mut line,
            "",
            0,
            mac.span().end().column,
        );

        line.insert_str(0, &penultimate_str);
        lines.remove(penultimate);
    };

    lines.remove(delete);

    Some(lines.join("\n"))
}

fn help(opts: &Options, exit_code: i32) -> ! {
    print!("{}", opts.usage("if-to-let-chain [Options] FILE"));
    process::exit(exit_code);
}

fn main() {
    let mut opts = Options::new();
    opts.optopt("d", "deindent", "number of chars to deindent by (default 4)", "N");
    opts.optflag("h", "help", "print this help");

    let matches = match opts.parse(env::args_os().skip(1)) {
        Ok(m) => m,
        Err(_) => {
            help(&opts, 1);
        }
    };

    if matches.opt_present("help") {
        help(&opts, 0);
    }

    let deindent: usize = matches
        .opt_get_default("deindent", 4)
        .expect("invalid deindent");

    for path in &matches.free {
        println!("{path}");

        let mut file = File::options()
            .read(true)
            .write(true)
            .open(path)
            .unwrap_or_else(|e| panic!("failed to open {path}: {e}"));

        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        let mut modified = false;
        while let Some(next) = if_to_let_chain(&contents, deindent) {
            modified = true;
            contents = next;
        }

        if modified {
            file.rewind().unwrap();
            file.write_all(contents.as_bytes()).unwrap();
            file.set_len(contents.len() as u64).unwrap();
        }
    }
}
