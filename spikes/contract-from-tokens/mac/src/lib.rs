//! `#[observe]` — the attribute macro whose feasibility is T-M1-07's subject.
//!
//! WHAT IT CLAIMS TO DO
//!   `docs/specs/rust-type-model.md` §What a proc macro can see says effects are
//!   syntactically confined inside a single item, `handle`, so an attribute
//!   macro on the impl block sees all of its body tokens — and that "**that fact
//!   is what makes the approach feasible**". This crate is that macro, written
//!   so the claim can be compiled rather than read.
//!
//! WHAT IT IS NOT
//!   It does not enforce anything. It emits a description beside the impl it was
//!   given, and the impl is passed through untouched. The enforcement in Verum's
//!   design is the type system; the difference between the two is what Q-A
//!   proposes to use as a detector. Saying this here because
//!   `docs/specs/effect-inference.md`'s `observed` is repeatedly read as if it
//!   were a check (#42's first defect).
//!
//! THE RULE TABLE IS THE FINDING
//!   There is no generic "recover the contract" transform. Every contract key
//!   needs its own hand-written rule, and every rule is a naming convention the
//!   macro cannot validate — it has the impl block's tokens and nothing else. No
//!   `Domain` definition, no field list, no trait resolution.

use proc_macro::TokenStream;
use quote::quote;
use syn::visit::Visit;
use syn::{Expr, ExprCall, ExprClosure, ExprMethodCall, ImplItem, ItemImpl};

/// Where an effect was found. `handler-rules.md` Rule 3 and Rule 4 make these
/// semantically distinct, so flattening them would erase the difference between
/// "email may change" and "email changes".
#[derive(Clone, PartialEq)]
enum Scope {
    /// `ctx.when::<C, _>(..)` — the condition's type as written at the call site.
    When(String),
    /// `ctx.after_commit(..)`
    AfterCommit,
}

impl Scope {
    fn part(&self) -> String {
        match self {
            Scope::When(c) => format!("when:{c}"),
            Scope::AfterCommit => "after_commit".to_owned(),
        }
    }
}

/// The whole stack, innermost last. Reading only the innermost scope reported a
/// mutation nested in two `when`s as conditional on the *inner* condition alone
/// — a condition strictly weaker than reality, i.e. an over-claim. Found in
/// review; probe W1 pins it.
fn tag_of(stack: &[Scope]) -> String {
    if stack.is_empty() {
        return "top".to_owned();
    }
    stack.iter().map(Scope::part).collect::<Vec<_>>().join("+")
}

#[derive(Default)]
struct Scan {
    fields: Vec<String>,
    creates: Vec<String>,
    emits: Vec<String>,
    calls: Vec<String>,
    reads: Vec<String>,
    escapes: Vec<String>,
    scope_stack: Vec<Scope>,
}

/// `users` -> `User`, `audit_logs` -> `AuditLog`. A convention, applied to a
/// method name, with nothing to check it against.
fn accessor_to_domain(accessor: &str) -> String {
    let singular = accessor.strip_suffix('s').unwrap_or(accessor);
    singular
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// `set_name` -> `name`. Field identity is the snake-case name; the contract
/// writes `User::name`, so no case transform is needed here.
fn setter_to_field(method: &str) -> Option<&str> {
    method.strip_prefix("set_")
}

/// The *type* a `creates` / `emits` entry names, from a constructor call path.
///
/// It is the segment **before** the function, not the first: `AuditLog::user_updated`
/// and `UserUpdated::from` have it at index 0, but `events::UserUpdated::from` has a
/// module there. Taking the first segment reported the module name as an event type
/// (found in review). A bare local (`emit(evt)`) still yields the variable's name —
/// no path, nothing to resolve, and the macro has no types.
fn arg_type_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Call(ExprCall { func, .. }) => match &**func {
            Expr::Path(p) => segment_before_last(&p.path),
            _ => None,
        },
        Expr::Path(p) => segment_before_last(&p.path),
        Expr::MethodCall(m) => arg_type_name(&m.receiver),
        Expr::Reference(r) => arg_type_name(&r.expr),
        _ => None,
    }
}

/// Is this receiver literally `ctx`? Rule 2's premise is that every effect goes
/// through it, and "goes through it" is checked here by **spelling**.
fn segment_before_last(path: &syn::Path) -> Option<String> {
    let n = path.segments.len();
    let i = if n >= 2 { n - 2 } else { 0 };
    path.segments.iter().nth(i).map(|s| s.ident.to_string())
}

fn is_ctx(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(p) if p.path.is_ident("ctx"))
}

/// `ctx.users()` — returns the accessor name when the receiver is `ctx` itself.
fn ctx_accessor(expr: &Expr) -> Option<String> {
    match expr {
        Expr::MethodCall(m) if is_ctx(&m.receiver) && m.args.is_empty() => {
            Some(m.method.to_string())
        }
        _ => None,
    }
}

impl Scan {
    fn scope_tag(&self) -> String {
        tag_of(&self.scope_stack)
    }

    /// Returns true when it has already walked the node's arguments, so the
    /// generic visit must not walk them again. Without this the closure of a
    /// scope call is visited twice — once inside the scope and once at top —
    /// and every conditional effect is also reported as unconditional.
    fn record(&mut self, node: &ExprMethodCall) -> bool {
        let method = node.method.to_string();
        let scope = self.scope_tag();

        // --- scope-introducing calls on `ctx` itself -----------------------
        if is_ctx(&node.receiver) {
            if method == "when" {
                let cond = node
                    .turbofish
                    .as_ref()
                    .and_then(|t| t.args.first())
                    .map(|a| quote!(#a).to_string())
                    .unwrap_or_else(|| "?".to_owned());
                for a in &node.args {
                    if let Expr::Closure(c) = a {
                        self.scope_stack.push(Scope::When(cond.clone()));
                        self.visit_expr_closure(c);
                        self.scope_stack.pop();
                    } else {
                        // Not the scope body. An effect here runs at the OUTER
                        // scope; skipping every argument dropped it entirely.
                        self.visit_expr(a);
                    }
                }
                return true;
            }
            if method == "after_commit" {
                for a in &node.args {
                    if let Expr::Closure(c) = a {
                        self.scope_stack.push(Scope::AfterCommit);
                        self.visit_expr_closure(c);
                        self.scope_stack.pop();
                    } else {
                        self.visit_expr(a);
                    }
                }
                return true;
            }
        }

        // --- effect calls on a `ctx.<accessor>()` ---------------------------
        let Some(accessor) = ctx_accessor(&node.receiver) else {
            return false;
        };
        let domain = accessor_to_domain(&accessor);

        if let Some(field) = setter_to_field(&method) {
            self.fields.push(format!("{domain}::{field}@{scope}"));
        } else if method == "find" || method == "get" {
            self.reads.push(format!("{domain}@{scope}"));
        } else if method == "create" {
            if let Some(t) = node.args.first().and_then(arg_type_name) {
                self.creates.push(format!("{t}@{scope}"));
            }
        } else if method == "emit" {
            if let Some(t) = node.args.first().and_then(arg_type_name) {
                self.emits.push(format!("{t}@{scope}"));
            }
        } else {
            // Anything else on a ctx accessor is an outbound call. `ctx.email()`
            // -> `Email`; the contract writes `EmailService`, which this cannot
            // know.
            self.calls.push(format!("{domain}@{scope}"));
        }
        false
    }
}

impl<'ast> Visit<'ast> for Scan {
    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let consumed = self.record(node);
        self.visit_expr(&node.receiver);
        if !consumed {
            for a in &node.args {
                self.visit_expr(a);
            }
        }
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Expr::Path(p) = &*node.func {
            let segs: Vec<String> = p
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            if segs.last().map(|s| s == "from_repr").unwrap_or(false) {
                let scope = self.scope_tag();
                let ty = segment_before_last(&p.path).unwrap_or_else(|| "?".to_owned());
                self.escapes.push(format!("{ty}::from_repr@{scope}"));
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_closure(&mut self, node: &'ast ExprClosure) {
        syn::visit::visit_expr_closure(self, node);
    }
}

fn json_array(items: &[String]) -> String {
    let inner = items
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{inner}]")
}

#[proc_macro_attribute]
pub fn observe(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Layer 1. `proc-macro.md` requires a precise span, never `call_site()`, so
    // the error is attached to the item the user actually wrote. R1 pins it.
    // Layer 1, and it must distinguish two failures. Discarding syn's error and
    // always blaming the item kind produced a SECOND, false error whenever the
    // impl block merely had a syntax error — rustc's real message and a
    // contradicting one, side by side (`diagnostics.md` "One error, one cause").
    let input = match syn::parse::<ItemImpl>(item.clone()) {
        Ok(i) => i,
        Err(e) if syn::parse::<syn::Item>(item.clone()).is_ok() => {
            // It parses as *some* item, just not an impl block: the attribute is
            // in the wrong place. `new_spanned` keeps the span on the item, never
            // `call_site()` (`proc-macro.md`). R1 pins this.
            let _ = e;
            let item2 = proc_macro2::TokenStream::from(item);
            return syn::Error::new_spanned(
                item2,
                "#[observe] goes on an `impl` block. A proc macro sees only the tokens of \
                 the item it is attached to, so on anything else it cannot see `handle`'s \
                 body.",
            )
            .to_compile_error()
            .into();
        }
        // Not a well-formed item at all — that is rustc's error to report, not ours.
        Err(e) => return e.to_compile_error().into(),
    };

    let self_ty = &input.self_ty;
    let name = quote!(#self_ty).to_string().replace(' ', "");
    // A path-qualified or generic self type (`endpoints::Scoped`, `Wrapper<u8>`)
    // is the normal case in a real crate, and feeding it straight to
    // `Ident::new` PANICKED the macro. Non-identifier characters collapse to `_`.
    // `__VERUM_` prefixes it per `proc-macro.md` so a user item cannot collide,
    // and two `#[observe]`d impls for one type are still `E0428` by design.
    let sanitised: String = to_screaming(&name)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let const_ident = syn::Ident::new(
        &format!("__VERUM_OBSERVED_{sanitised}"),
        proc_macro2::Span::call_site(),
    );

    let mut scan = Scan::default();
    for it in &input.items {
        if let ImplItem::Fn(f) = it {
            // Every method in the impl block is walked, not only `handle`. P7
            // measures whether that matters.
            scan.visit_block(&f.block);
        }
    }

    let json = format!(
        concat!(
            r#"{{"endpoint":"{}","fields":{},"reads":{},"creates":{},"emits":{},"#,
            r#""calls":{},"escapes":{},"scope":"handle_only","deferred":"unknown"}}"#
        ),
        name,
        json_array(&scan.fields),
        json_array(&scan.reads),
        json_array(&scan.creates),
        json_array(&scan.emits),
        json_array(&scan.calls),
        json_array(&scan.escapes),
    );

    // No `#[automatically_derived]`: it only has an effect on impl blocks. On a
    // const it is a warning on 1.85 (suppressed inside macro expansion, so the
    // spike's own build never showed it) and a hard error on a later toolchain.
    let expanded = quote! {
        #input
        #[doc(hidden)]
        pub const #const_ident: &str = #json;
    };
    expanded.into()
}

fn to_screaming(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i != 0 {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}
