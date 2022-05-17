pub(super) fn check(
    cx: &LateContext<'_>,
    expr: &hir::Expr<'_>,
    caller: &hir::Expr<'_>,
    map_arg: &hir::Expr<'_>,
    name: &str,
    _map_span: Span,
) {
    let caller_ty = cx.typeck_results().expr_ty(caller);

    if_chain! {
        if is_trait_method(cx, expr, sym::Iterator)
            || is_type_diagnostic_item(cx, caller_ty, sym::Result)
            || is_type_diagnostic_item(cx, caller_ty, sym::Option);
        if is_expr_identity_function(cx, map_arg);
        if let Some(sugg_span) = expr.span.trim_start(caller.span);
        then {
            span_lint_and_sugg(
                cx,
                MAP_IDENTITY,
                sugg_span,
                "unnecessary map of the identity function",
                &format!("remove the call to `{}`", name),
                String::new(),
                Applicability::MachineApplicable,
            )
        }
    }
}
