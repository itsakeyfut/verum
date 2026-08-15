//! The `Handler` trait and the erasure layer RK-012 says the router needs.
//!
//! RK-012's lesson is that `async fn` in trait leaves the future non-`Send` and
//! the trait dyn-incompatible, so `Handler` is RPITIT and dyn compatibility is
//! bought back with a boxing layer. What RK-012 does *not* say — and what B1/B2
//! measure — is that the erased signature cannot mention `Ctx<'req, E>` at all.

use std::collections::HashMap;
use std::pin::Pin;

use crate::{Ctx, Endpoint, Error, Result, Runtime};

/// What the user writes. RPITIT, `+ Send`, `Ctx` taken by value.
///
/// `decode` / `encode` stand in for request extraction and view generation,
/// both of which are on `docs/rules/README.md`'s undecided list. They are here
/// only because the erasure layer needs *some* `E`-free byte boundary to cross.
pub trait Handler: Endpoint {
    type Req: Send + 'static;
    type Res: Send + 'static;

    fn decode(body: &[u8]) -> Result<Self::Req>;
    fn encode(res: Self::Res) -> Vec<u8>;

    fn handle<'req>(
        &self,
        req: Self::Req,
        ctx: Ctx<'req, Self>,
    ) -> impl Future<Output = Result<Self::Res>> + Send;
}

/// The dyn-compatible layer the router stores.
///
/// Note what is **not** in this signature: the `Ctx`. It cannot be — `E` differs
/// per handler and `dyn ErasedHandler` has no way to name it. The `Runtime` is
/// passed instead and the `Ctx` is built on the other side of the boxing, where
/// `H` is still known. Probe B2 is the failing form that grounds this.
pub trait ErasedHandler: Send + Sync {
    fn call<'req>(
        &'req self,
        body: Vec<u8>,
        rt: &'req Runtime,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'req>>;
}

impl<H: Handler> ErasedHandler for H {
    fn call<'req>(
        &'req self,
        body: Vec<u8>,
        rt: &'req Runtime,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'req>> {
        Box::pin(async move {
            let req = H::decode(&body)?;
            // Here, and only here. `Ctx::new` is `pub(crate)`, and this is the
            // one place in the design that both knows `H` and holds the
            // `Runtime` borrow the `'req` lifetime comes from.
            let ctx = Ctx::new(rt);
            let res = self.handle(req, ctx).await?;
            Ok(H::encode(res))
        })
    }
}

/// Holds erased handlers, which is what forces the layer above to exist.
#[derive(Default)]
pub struct Router {
    routes: HashMap<String, Box<dyn ErasedHandler>>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route<H: Handler>(mut self, path: &str, h: H) -> Self {
        self.routes.insert(path.to_owned(), Box::new(h));
        self
    }

    pub(crate) fn lookup(&self, path: &str) -> Result<&dyn ErasedHandler> {
        self.routes
            .get(path)
            .map(|b| &**b)
            .ok_or_else(|| Error(format!("no route: {path}")))
    }
}
