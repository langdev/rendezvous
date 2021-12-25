#[cfg(feature = "legacy")]
mod legacy;

use std::collections::HashMap;
use std::sync::Arc;

use tokio_stream::wrappers::TcpListenerStream;

use rendezvous_common::{
    anyhow,
    futures::{
        channel::mpsc::{self, channel},
        prelude::*,
    },
    proto::{
        bouncer_service_server::{BouncerService, BouncerServiceServer},
        ClientType, Event, Header, PostResult,
    },
    tokio::{self, net::TcpListener, sync::Mutex},
    tonic::{self, transport::Server, Request, Response, Status},
    tracing::{self, debug, info, instrument, warn},
};

type BouncerMap = Arc<Mutex<HashMap<ClientType, Option<mpsc::Sender<Result<Event, Status>>>>>>;

fn main() -> anyhow::Result<()> {
    tracing::init()?;

    serve_legacy()?;

    let addr = "[::1]:49252";

    let service_impl = BouncerServiceImpl {
        bouncers: BouncerMap::default(),
    };

    let svc = BouncerServiceServer::new(service_impl);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;

    rt.block_on(async move {
        let listener = TcpListener::bind(addr).await?;
        let incoming = TcpListenerStream::new(listener);

        Server::builder()
            .add_service(svc)
            .serve_with_incoming(incoming)
            .await?;
        Ok::<_, anyhow::Error>(())
    })?;

    Ok(())
}

#[cfg(feature = "legacy")]
fn serve_legacy() -> anyhow::Result<()> {
    std::thread::spawn(|| crate::legacy::serve());
    Ok(())
}

#[cfg(not(feature = "legacy"))]
fn serve_legacy() -> anyhow::Result<()> {
    Ok(())
}

#[derive(Debug, Default)]
pub struct BouncerServiceImpl {
    bouncers: BouncerMap,
}

#[tonic::async_trait]
impl BouncerService for BouncerServiceImpl {
    type SubscribeStream = mpsc::Receiver<Result<Event, Status>>;

    #[instrument]
    async fn post(&self, request: Request<Event>) -> Result<Response<PostResult>, Status> {
        println!("{:?}", request);
        debug!("{:?}", request);
        let event = request.into_inner();
        let header = event.header()?;
        let mut bouncers = self.bouncers.lock().await;
        for (b, sender) in &mut *bouncers {
            if b == &header.client_type() {
                continue;
            }
            if let Some(s) = sender {
                if let Err(e) = s.send(Ok(event.clone())).await {
                    if e.is_disconnected() {
                        *sender = None;
                        info!("Disconnected from {:?}", b);
                    } else {
                        warn!("{:?}: {}", b, e);
                    }
                }
            }
        }
        bouncers.retain(|_, sender| sender.is_some());
        Ok(Response::new(PostResult::new()))
    }

    #[instrument]
    async fn subscribe(
        &self,
        request: Request<Header>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        debug!("{:?}", request);
        let (sender, receiver) = channel(16);
        let mut bouncers = self.bouncers.lock().await;
        bouncers.insert(request.get_ref().client_type(), Some(sender));
        Ok(Response::new(receiver))
    }
}
