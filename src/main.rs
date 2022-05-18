use getopts::Options;
use proc_macro2::LineColumn;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::{env, process};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::token::Semi;
use syn::visit::{visit_file, Visit};
use syn::{BinOp, Block, Expr, ExprBinary, Ident, Local, Macro, Result, Stmt, Token};

#[derive(Debug)]
struct IfExpr {
    if_token: Token![if],
    expr: Expr,
    semi_token: Token![;],
}

#[derive(Debug)]
enum Statement {
    IfExpr(IfExpr),
    Local(Local),
}

impl Statement {
    fn start(&self) -> LineColumn {
        match self {
            Self::IfExpr(if_expr) => if_expr.if_token.span.start(),
            Self::Local(local) => local.span().start(),
        }
    }

    fn start_after_if(&self) -> LineColumn {
        match self {
            Self::IfExpr(if_expr) => if_expr.expr.span().start(),
            Self::Local(local) => local.span().start(),
        }
    }

    fn semi(&self) -> Semi {
        match self {
            Self::IfExpr(if_expr) => if_expr.semi_token,
            Self::Local(local) => local.semi_token,
        }
    }

    fn expr(&self) -> &Expr {
        match self {
            Self::IfExpr(if_expr) => &if_expr.expr,
            Self::Local(local) => &local.init.as_ref().expect("missing init").1,
        }
    }
}

impl Parse for Statement {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![if]) {
            Ok(Self::IfExpr(IfExpr {
                if_token: input.parse()?,
                expr: input.parse()?,
                semi_token: input.parse()?,
            }))
        } else {
            let stmt: Stmt = input.parse()?;
            match stmt {
                Stmt::Local(local) => Ok(Self::Local(local)),
                _ => Err(input.error("expected local")),
            }
        }
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

fn if_to_let_chain(input: &str, deindent: usize, path: &str) -> Option<String> {
    let file = match syn::parse_file(input) {
        Ok(file) => file,
        Err(e) => {
            println!("failed to parse {path}: {e}");
            return None;
        }
    };

    let mut visitor = Visitor::default();
    visit_file(&mut visitor, &file);

    let (if_chain, mac) = visitor.found?;

    let mut lines: Vec<String> = input.lines().map(String::from).collect();

    for statement in &if_chain.statements {
        let parens = matches!(
            statement.expr(),
            Expr::Binary(ExprBinary {
                op: BinOp::Or(_),
                ..
            }) | Expr::Closure(_)
        );

        let semi = statement.semi().span.start();

        replace_chars(
            &mut lines[semi.line - 1],
            if parens { ")" } else { "" },
            semi.column,
            semi.column + 1,
        );

        if parens {
            let pos = statement.expr().span().start();
            replace_chars(&mut lines[pos.line - 1], "(", pos.column, pos.column)
        }
    }

    let (first, rest) = if_chain.statements.split_first().unwrap();
    if first.start().line - 1 > mac.bang_token.span.start().line {
        println!(
            "{path}:{}: found leading comment or blank line, may require manual fixup",
            first.start().line - 1,
        );
    }

    for statement in rest {
        replace_chars(
            &mut lines[statement.start().line - 1],
            "&& ",
            statement.start().column,
            statement.start_after_if().column,
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
            if matches!(first, Statement::IfExpr(_)) {
                ""
            } else {
                "if "
            },
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

        replace_chars(&mut line, "", 0, mac.span().end().column);

        line.insert_str(0, &penultimate_str);
        lines.remove(penultimate);
    };

    lines.remove(delete);

    Some(lines.join("\n"))
}

fn modify(contents: &mut String, deindent: usize, path: &str) -> bool {
    let mut modified = false;
    while let Some(next) = if_to_let_chain(contents, deindent, path) {
        modified = true;
        *contents = next;
    }
    modified
}

fn help(opts: &Options, exit_code: i32) -> ! {
    print!("{}", opts.usage("if-to-let-chain [Options] FILE"));
    process::exit(exit_code);
}

fn main() {
    let mut opts = Options::new();
    opts.optopt(
        "d",
        "deindent",
        "number of chars to deindent by (default 4)",
        "N",
    );
    opts.optflag("v", "verbose", "print extra information");
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
    let verbose = matches.opt_present("verbose");

    for path in &matches.free {
        let mut file = File::options()
            .read(true)
            .write(true)
            .open(path)
            .unwrap_or_else(|e| panic!("failed to open {path}: {e}"));

        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        if modify(&mut contents, deindent, &path) {
            if verbose {
                println!("modified {path}");
            }
            file.rewind().unwrap();
            file.write_all(contents.as_bytes()).unwrap();
            file.set_len(contents.len() as u64).unwrap();
        } else if verbose {
            println!("unchanged {path}");
        }
    }
}

#[cfg(test)]
mod tests;
