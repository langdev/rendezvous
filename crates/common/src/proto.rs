tonic::include_proto!("org.langdev.rendezvous");

mod impls {
    use tonic::{Code, Status};

    use super::*;

    impl PostResult {
        pub fn new() -> Self {
            Self {}
        }
    }

    impl Event {
        pub fn header(&self) -> Result<&Header, Status> {
            match &self.header {
                Some(header) => Ok(header),
                None => Err(Status::new(Code::InvalidArgument, "missing header")),
            }
        }
    }
}
