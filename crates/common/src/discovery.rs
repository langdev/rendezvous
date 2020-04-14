use nng::{self, Message, Protocol, Socket};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::ipc::dial;

pub fn address() -> String {
    std::env::var("RENDEZVOUS_DISCOVERY_ADDR")
        .unwrap_or_else(|_| "ipc:///var/tmp/rendezvous-discovery.pipe".to_owned())
}

pub fn register(address: &str, service_info: ServiceInfo) -> anyhow::Result<()> {
    let socket = Socket::new(Protocol::Respondent0)?;
    dial(&socket, address)?;
    register_loop(&socket, service_info)
}

fn register_loop(socket: &Socket, service_info: ServiceInfo) -> anyhow::Result<()> {
    let mut res = Message::new();
    serde_cbor::to_writer(&mut res, &service_info)?;
    loop {
        let _req = socket.recv()?;
        info!("survey received");
        if let Err((_, err)) = socket.send(res.clone()) {
            return Err(err.into());
        }
    }
    Ok(())
}

#[derive(Deserialize, Serialize)]
pub struct ServiceInfo {
    pub name: String,
    pub address: String,
}

#[cfg(test)]
mod test {
    use std::thread;
    use std::time::Duration;

    use nng::options::{protocol::survey::SurveyTime, Options as _, RecvTimeout, SendTimeout};

    use super::*;

    #[test]
    fn register_basic() {
        let address = "inproc://register-basic";
        let service_info = ServiceInfo {
            name: "foo".to_owned(),
            address: "inproc://foo".to_owned(),
        };

        let socket = Socket::new(Protocol::Surveyor0).unwrap();
        let timeout = Some(Duration::from_secs(5));
        socket.set_opt::<SendTimeout>(timeout).unwrap();
        socket.set_opt::<RecvTimeout>(timeout).unwrap();
        socket.set_opt::<SurveyTime>(timeout).unwrap();
        socket.listen(address).unwrap();
        info!("start to listen");

        let respondent_socket = Socket::new(Protocol::Respondent0).unwrap();
        respondent_socket.dial(address).unwrap();

        thread::spawn(move || register_loop(&respondent_socket, service_info).unwrap());
        thread::sleep(Duration::from_secs(1));

        socket
            .send(Message::new())
            .map_err(nng::Error::from)
            .unwrap();
        info!("survey sent");

        let msg = socket.recv().unwrap();
        info!("response received");
        let service_info: ServiceInfo = serde_cbor::from_slice(&msg).unwrap();
        assert_eq!(service_info.name, "foo");
        assert_eq!(service_info.address, "inproc://foo");
    }
}
