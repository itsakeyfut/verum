//! The hyper side of criterion (b): a real `Service`, a real multi-thread
//! server, and a client to drive it.
//!
//! The structural point this file exists to demonstrate is in `Svc::call`:
//! `hyper::service::Service::Future` is a plain associated type with no lifetime
//! parameter, so the service's future is effectively `'static`. `'req` therefore
//! cannot flow *in* from hyper — it is created **inside** each request's own
//! future, from a `Runtime` that future owns. Probe B4 is the failing form that
//! grounds this rather than asserting it.

use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, TcpStream};

use crate::{Error, Result, Router, Runtime};

#[derive(Clone)]
struct Svc {
    router: Arc<Router>,
    rt: Runtime,
}

impl Service<Request<Incoming>> for Svc {
    type Response = Response<Full<Bytes>>;
    type Error = Error;
    /// No lifetime. hyper offers nowhere to put one — see the module docs.
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let router = Arc::clone(&self.router);
        // Cloned, not borrowed: the future below must be `'static`, so it has to
        // own the thing the request will borrow.
        let rt = self.rt.clone();
        Box::pin(async move {
            let path = req.uri().path().to_owned();
            let body = req
                .into_body()
                .collect()
                .await
                .map_err(|e| Error(e.to_string()))?
                .to_bytes()
                .to_vec();

            // `&rt` — a borrow that begins and ends inside this future. That
            // borrow *is* `'req`.
            let out = router.lookup(&path)?.call(body, &rt).await?;
            Ok(Response::new(Full::new(Bytes::from(out))))
        })
    }
}

pub struct Server {
    pub addr: SocketAddr,
}

impl Server {
    /// Binds on an ephemeral port and serves until the process ends.
    ///
    /// Multi-thread is not incidental — it is criterion (b). A current-thread
    /// runtime would not require the handler future to be `Send` and the probe
    /// would pass without measuring anything.
    pub async fn start(router: Router, rt: Runtime) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| Error(e.to_string()))?;
        let addr = listener.local_addr().map_err(|e| Error(e.to_string()))?;

        let svc = Svc {
            router: Arc::new(router),
            rt,
        };

        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let svc = svc.clone();
                tokio::spawn(async move {
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), svc)
                        .await;
                });
            }
        });

        Ok(Self { addr })
    }
}

/// A one-shot client request, returning the response body as a `String`.
pub async fn get(addr: SocketAddr, path: &str, body: &str) -> Result<String> {
    let stream = TcpStream::connect(addr)
        .await
        .map_err(|e| Error(e.to_string()))?;
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|e| Error(e.to_string()))?;
    tokio::spawn(conn);

    let req = Request::builder()
        .uri(path)
        .body(Full::new(Bytes::from(body.to_owned())))
        .map_err(|e| Error(e.to_string()))?;

    let res = sender
        .send_request(req)
        .await
        .map_err(|e| Error(e.to_string()))?;
    let bytes = res
        .into_body()
        .collect()
        .await
        .map_err(|e| Error(e.to_string()))?
        .to_bytes();

    String::from_utf8(bytes.to_vec()).map_err(|e| Error(e.to_string()))
}
