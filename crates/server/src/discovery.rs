use std::collections::HashMap;
use std::time::Duration;

use futures::channel::mpsc;

use rendezvous_common::{
    discovery::ServiceInfo,
    nng::{
        self,
        options::{protocol::survey::SurveyTime, Options},
        Message, Protocol, Socket,
    },
    serde_cbor,
    tracing::{info, warn},
};

pub type ServiceMap = HashMap<String, Vec<String>>;

pub fn start(tx: mpsc::UnboundedSender<ServiceMap>) -> nng::Result<()> {
    let address = rendezvous_common::discovery::address();
    let socket = Socket::new(Protocol::Surveyor0)?;
    socket.set_opt::<SurveyTime>(Some(Duration::from_secs(5)))?;
    socket.listen(&address)?;
    start_loop(&socket, tx)
}

fn start_loop(socket: &Socket, tx: mpsc::UnboundedSender<ServiceMap>) -> nng::Result<()> {
    loop {
        let msg = Message::new();
        socket.send(msg)?;
        info!("survey start");
        let mut services = ServiceMap::new();
        loop {
            let msg = match socket.recv() {
                Err(nng::Error::IncorrectState) | Err(nng::Error::TimedOut) => break,
                Err(err) => {
                    return Err(err);
                }
                Ok(msg) => msg,
            };
            info!("response received");
            let service_info: ServiceInfo = match serde_cbor::from_slice(msg.as_slice()) {
                Err(err) => {
                    warn!("{}", err);
                    continue;
                }
                Ok(v) => v,
            };
            services
                .entry(service_info.kind)
                .or_default()
                .push(service_info.address);
        }
        info!("survey done");
        if !services.is_empty() {
            if let Err(_) = tx.unbounded_send(services) {
                break;
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use std::thread;

    use async_std::prelude::*;

    use rendezvous_common::anyhow;

    use super::*;

    #[async_std::test]
    async fn start_basic() -> anyhow::Result<()> {
        let address = "inproc://start_basic";
        let (tx, mut rx) = mpsc::unbounded();

        let socket = Socket::new(Protocol::Surveyor0)?;
        socket.set_opt::<SurveyTime>(Some(Duration::from_secs(1)))?;
        socket.listen(address)?;

        let respondent_socket = Socket::new(Protocol::Respondent0)?;
        info!("try to connect");
        respondent_socket.dial(address)?;
        info!("connected");

        fn respondent_loop(socket: &Socket) -> anyhow::Result<()> {
            let _msg = socket.recv()?;
            info!("survey received");
            let mut msg = Message::new();
            serde_cbor::to_writer(
                &mut msg,
                &ServiceInfo {
                    kind: "echo".to_owned(),
                    address: "inproc://echo".to_owned(),
                },
            )?;
            socket.send(msg).map_err(nng::Error::from)?;
            info!("response sent");
            Ok(())
        }
        thread::spawn(move || respondent_loop(&respondent_socket).unwrap());
        thread::spawn(move || start_loop(&socket, tx));

        let srv_map = rx.next().timeout(Duration::from_secs(5)).await?.unwrap();

        assert_eq!(
            srv_map,
            vec![("echo".to_owned(), vec!["inproc://echo".to_owned()])]
                .into_iter()
                .collect()
        );

        drop(srv_map);
        Ok(())
    }
}
