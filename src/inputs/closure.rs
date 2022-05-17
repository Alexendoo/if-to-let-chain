/// lint use of `filter().map()` or `find().map()` for `Iterators`
#[allow(clippy::too_many_arguments)]
pub(super) fn check<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &hir::Expr<'_>,
    filter_recv: &hir::Expr<'_>,
    filter_arg: &hir::Expr<'_>,
    filter_span: Span,
    map_recv: &hir::Expr<'_>,
    map_arg: &hir::Expr<'_>,
    map_span: Span,
    is_find: bool,
) {
    lint_filter_some_map_unwrap(
        cx,
        expr,
        filter_recv,
        filter_arg,
        map_arg,
        map_span,
        filter_span.with_hi(expr.span.hi()),
    );
    if_chain! {
            if is_trait_method(cx, map_recv, sym::Iterator);

            // filter(|x| ...is_some())...
            if let ExprKind::Closure(_, _, filter_body_id, ..) = filter_arg.kind;
            let filter_body = cx.tcx.hir().body(filter_body_id);
            if let [filter_param] = filter_body.params;
            // optional ref pattern: `filter(|&x| ..)`
            let (filter_pat, is_filter_param_ref) = if let PatKind::Ref(ref_pat, _) = filter_param.pat.kind {
                (ref_pat, true)
            } else {
                (filter_param.pat, false)
            };
            // closure ends with is_some() or is_ok()
            if let PatKind::Binding(_, filter_param_id, _, None) = filter_pat.kind;
            if let ExprKind::MethodCall(path, [filter_arg], _) = filter_body.value.kind;
            if let Some(opt_ty) = cx.typeck_results().expr_ty(filter_arg).ty_adt_def();
            if let Some(is_result) = if cx.tcx.is_diagnostic_item(sym::Option, opt_ty.did()) {
                Some(false)
            } else if cx.tcx.is_diagnostic_item(sym::Result, opt_ty.did()) {
                Some(true)
            } else {
                None
            };
            if path.ident.name.as_str() == if is_result { "is_ok" } else { "is_some" };

            // ...map(|x| ...unwrap())
            if let ExprKind::Closure(_, _, map_body_id, ..) = map_arg.kind;
            let map_body = cx.tcx.hir().body(map_body_id);
            if let [map_param] = map_body.params;
            if let PatKind::Binding(_, map_param_id, map_param_ident, None) = map_param.pat.kind;
            // closure ends with expect() or unwrap()
            if let ExprKind::MethodCall(seg, [map_arg, ..], _) = map_body.value.kind;
            if matches!(seg.ident.name, sym::expect | sym::unwrap | sym::unwrap_or);

            let eq_fallback = |a: &Expr<'_>, b: &Expr<'_>| {
                // in `filter(|x| ..)`, replace `*x` with `x`
                let a_path = if_chain! {
                    if !is_filter_param_ref;
                    if let ExprKind::Unary(UnOp::Deref, expr_path) = a.kind;
                    then { expr_path } else { a }
                };
                // let the filter closure arg and the map closure arg be equal
                if_chain! {
                    if path_to_local_id(a_path, filter_param_id);
                    if path_to_local_id(b, map_param_id);
                    if cx.typeck_results().expr_ty_adjusted(a) == cx.typeck_results().expr_ty_adjusted(b);
                    then {
                        return true;
                    }
                }
                false
            };
            if SpanlessEq::new(cx).expr_fallback(eq_fallback).eq_expr(filter_arg, map_arg);
            then {
                let span = filter_span.with_hi(expr.span.hi());
                let (filter_name, lint) = if is_find {
                    ("find", MANUAL_FIND_MAP)
                } else {
                    ("filter", MANUAL_FILTER_MAP)
                };
                let msg = format!("`{}(..).map(..)` can be simplified as `{0}_map(..)`", filter_name);
                let to_opt = if is_result { ".ok()" } else { "" };
                let sugg = format!("{}_map(|{}| {}{})", filter_name, map_param_ident,
                    snippet(cx, map_arg.span, ".."), to_opt);
                span_lint_and_sugg(cx, lint, span, &msg, "try", sugg, Applicability::MachineApplicable);
            }
    }
}
