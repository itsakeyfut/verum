//! The probes that have to *run*, not merely type-check.
//!
//! Every endpoint here **delegates to the function in `app`'s lib** rather than
//! re-implementing it. That is deliberate and was not true of the first version:
//! `docs/rules/test.md` §9-13 asks that a pass-probe assert the code still
//! exists and still goes through the mechanism, and a test that reimplements the
//! probe asserts neither. Measured on the earlier version: eleven pass-probe
//! bodies could be emptied at once and the suite still reported 16/16.

// `async fn` cannot carry `+ Send` in its return type, and `'req` is the subject.
#![allow(clippy::manual_async_fn, clippy::needless_lifetimes)]

use std::time::Duration;

use app::{NoSendBeforeAwaitEndpoint, SpawnControl, UpdateUser};
use fw::{Domain, Req, Router, Runtime, Server, get};

/// Generates the `Handler` boilerplate for an endpoint that forwards to one of
/// `app`'s probe functions. Six near-identical impls otherwise.
macro_rules! delegating_endpoint {
    ($ep:ident => |$ctx:ident, $req:ident| $body:expr) => {
        struct $ep;

        impl fw::Endpoint for $ep {
            type Reads = ();
            type Mutates = ();
        }

        impl fw::Handler for $ep {
            type Req = Req;
            type Res = String;

            fn decode(body: &[u8]) -> fw::Result<Req> {
                <UpdateUser as fw::Handler>::decode(body)
            }

            fn encode(res: String) -> Vec<u8> {
                res.into_bytes()
            }

            fn handle<'req>(
                &self,
                $req: Req,
                $ctx: fw::Ctx<'req, Self>,
            ) -> impl Future<Output = fw::Result<String>> + Send {
                $body
            }
        }
    };
}

// D1 — the spec's elided `when`, called through `app::d1_when_lends`.
delegating_endpoint!(WhenEndpoint => |ctx, req| async move {
    app::d1_when_lends(ctx, req).await
});

// D1d — the boxed alternative.
delegating_endpoint!(BoxedEndpoint => |ctx, req| async move {
    app::d1d_when_boxed(ctx, req).await
});

// E3 / E4b — #39's two candidates still serve an ordinary handler.
delegating_endpoint!(CandidateEndpoint => |ctx, req| async move {
    let a = app::e3_lifetime_handle_ordinary_use(ctx, &req)?;
    Ok(a)
});

// E1 — the capability handle escapes the request that granted it.
delegating_endpoint!(LeakEndpoint => |ctx, req| async move {
    let user = ctx.users().find(req.id)?;
    app::e1_handle_escapes(ctx, user, "escaped@example.com".to_owned());
    Ok("returned-before-the-task-ran".to_owned())
});

// D5e — the sync-body leak. Note the body is NOT `async move`: the leak runs in
// the handler's synchronous prelude, and only its result crosses into the
// `+ Send` future. That is the whole point.
delegating_endpoint!(SyncBodyLeakEndpoint => |ctx, req| {
    let out = app::d5e_syncbody_leak(ctx, req);
    async move { out }
});

// F2 / F3 — #40's candidate, and the cost of it.
delegating_endpoint!(JobEndpoint => |ctx, req| async move {
    app::f2_owned_jobctx(ctx, req.id);
    Ok("spawned".to_owned())
});

async fn serve_one(ep_router: Router, rt: Runtime, path: &str, body: &str) -> String {
    let server = Server::start(ep_router, rt)
        .await
        .expect("server did not start");
    get(server.addr, path, body).await.expect("request failed")
}

/// B5 — a real multi-thread runtime, a real socket, and dispatch through
/// `Box<dyn ErasedHandler>`.
///
/// `multi_thread` is not decorative, but the reason is narrower than the first
/// version of this file claimed: `tokio::spawn` requires `Send + 'static`
/// whatever the flavour, and the `Send` obligation comes from `serve.rs`'s
/// per-connection spawn. What multi-thread adds is that the future really does
/// move between threads.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b5_multi_thread_server_serves_through_the_erasure_layer() {
    let rt = Runtime::new();
    rt.seed(Domain::new(7, "before@example.com"));

    let router = Router::new()
        .route("/users", UpdateUser)
        .route("/control", SpawnControl);
    let body = serve_one(router, rt.clone(), "/users", "7:after@example.com").await;

    assert_eq!(body, "ok:after@example.com");
    assert_eq!(
        rt.peek(7).expect("user vanished").email(),
        "after@example.com"
    );
}

/// B5b — two distinct concrete handler types behind one `Box<dyn _>` map.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b5_router_dispatches_two_distinct_handler_types() {
    let rt = Runtime::new();
    rt.seed(Domain::new(1, "a@example.com"));

    let router = Router::new()
        .route("/users", UpdateUser)
        .route("/control", SpawnControl);
    let server = Server::start(router, rt)
        .await
        .expect("server did not start");

    assert_eq!(
        get(server.addr, "/users", "1:b@example.com").await.unwrap(),
        "ok:b@example.com"
    );
    assert_eq!(get(server.addr, "/control", "").await.unwrap(), "0");
}

/// D1 — the spec's `when`, as elided, running inside a `+ Send` handler future.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn d1_spec_when_runs_its_body_and_mutates() {
    let rt = Runtime::new();
    rt.seed(Domain::new(3, "old@example.com"));

    let router = Router::new().route("/when", WhenEndpoint);
    let out = serve_one(router, rt.clone(), "/when", "3:new@example.com").await;

    assert_eq!(out, "new@example.com");
    assert_eq!(rt.peek(3).unwrap().email(), "new@example.com");
}

/// D1d — the boxed alternative also runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn d1d_boxed_when_runs_its_body_and_mutates() {
    let rt = Runtime::new();
    rt.seed(Domain::new(11, "old@example.com"));

    let router = Router::new().route("/boxed", BoxedEndpoint);
    let out = serve_one(router, rt.clone(), "/boxed", "11:fresh@example.com").await;

    assert_eq!(out, "fresh@example.com");
    assert_eq!(rt.peek(11).unwrap().email(), "fresh@example.com");
}

/// E3 — #39's candidate 1 still serves an ordinary handler end to end.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e3_lifetime_bound_handle_serves_an_ordinary_request() {
    let rt = Runtime::new();
    rt.seed(Domain::new(17, "before@example.com"));

    let router = Router::new().route("/cand", CandidateEndpoint);
    let out = serve_one(router, rt.clone(), "/cand", "17:after@example.com").await;

    assert_eq!(out, "after@example.com");
    assert_eq!(rt.peek(17).unwrap().email(), "after@example.com");
}

/// F2 — #40's candidate runs, and the owned `JobCtx` mutates after the request.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn f2_owned_jobctx_mutates_from_the_child_task() {
    let rt = Runtime::new();
    rt.seed(Domain::new(23, "before@example.com"));

    let router = Router::new().route("/job", JobEndpoint);
    let out = serve_one(router, rt.clone(), "/job", "23:ignored@example.com").await;
    assert_eq!(out, "spawned");

    assert_eq!(
        rt.peek(23).unwrap().email(),
        "before@example.com",
        "the child task must not have run yet, or this test proves nothing about \
         *when* the owned JobCtx mutates"
    );

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(rt.peek(23).unwrap().email(), "job@example.com");
}

/// D5e — the `when` scope's `Ctx` is used **after the scope returned**, from a
/// handler whose returned future is `+ Send`.
///
/// This is the probe #14 concluded could not exist. The sentinel is written only
/// by the escaped handle: the `when` body captures and writes nothing, so a pass
/// here cannot come from the condition having done the work. RK-017.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn d5e_escaped_ctx_mutates_after_the_when_scope_returned() {
    let rt = Runtime::new();
    rt.seed(Domain::new(31, "before@example.com"));

    let router = Router::new().route("/syncleak", SyncBodyLeakEndpoint);
    let out = serve_one(router, rt.clone(), "/syncleak", "31:ignored@example.com").await;

    assert_eq!(
        out, "leaked-after-scope@example.com",
        "the handler returned a value written through the escaped Ctx"
    );
    assert_eq!(
        rt.peek(31).unwrap().email(),
        "leaked-after-scope@example.com",
        "`+ Send` did not contain the scope: the Ctx was used after `when` returned"
    );
}

/// E5b — a `!Send` handle mutates **before** the await, inside a `+ Send` future.
///
/// `!Send` was considered as an extra restriction on the handle and rejected. This
/// is the half the rejection rests on: `+ Send` on the returned future reaches only
/// what is held **across** an await, so a handle used and dropped before it does
/// its work unimpeded. RK-017's await-scope half.
///
/// Executed rather than merely compiled, because the compile-only form was
/// satisfiable by an ordinary `Send` handle — review deleted the whole body and the
/// row stayed green.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e5b_nosend_handle_mutates_before_the_await() {
    let rt = Runtime::new();
    rt.seed(Domain::new(1, "before@example.com"));

    let router = Router::new().route("/nosend", NoSendBeforeAwaitEndpoint);
    let out = serve_one(router, rt.clone(), "/nosend", "1:ignored@example.com").await;

    assert_eq!(
        out, "nosend@example.com",
        "the handler returned the value it wrote through the `!Send` handle"
    );
    assert_eq!(
        rt.peek(1).unwrap().email(),
        "nosend@example.com",
        "`+ Send` did not stop the `!Send` handle: the mutation happened before the await"
    );
}

/// E1 — the leaked capability handle mutates **after the request returned**.
///
/// Observed from outside through `Runtime::peek`, never from the leaked task's
/// own account of itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e1_leaked_handle_mutates_after_the_request_scope_ended() {
    let rt = Runtime::new();
    rt.seed(Domain::new(9, "before@example.com"));

    let router = Router::new().route("/leak", LeakEndpoint);
    let server = Server::start(router, rt.clone())
        .await
        .expect("server did not start");

    let out = get(server.addr, "/leak", "9:ignored@example.com")
        .await
        .unwrap();
    assert_eq!(out, "returned-before-the-task-ran");
    assert_eq!(
        rt.peek(9).unwrap().email(),
        "before@example.com",
        "the leaked task must not have run yet, or this test proves nothing"
    );

    // The request is over. Nothing here holds a `Ctx`.
    tokio::time::sleep(Duration::from_millis(150)).await;

    assert_eq!(
        rt.peek(9).unwrap().email(),
        "escaped@example.com",
        "the capability handle outlived the request that granted it"
    );
}
