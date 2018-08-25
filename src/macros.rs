#[macro_export]
macro_rules! impl_get_bus_id {
    ( $target:ident ) => {
        impl Handler<crate::util::GetBusId> for $target {
            type Result = crate::bus::BusId;
            fn handle(&mut self, _: crate::util::GetBusId, _: &mut Self::Context) -> Self::Result {
                self.bus_id
            }
        }
    }
}

#[macro_export]
macro_rules! sleep_millis {
    ($millis:expr) => ({
        let d = ::std::time::Duration::from_millis($millis);
        let f = ::tokio::timer::Delay::new(Instant::now() + d).compat();
        await!(f)
    })
}


#[cfg(test)]
#[macro_use]
mod test {
    #[macro_export]
    macro_rules! actix_test_cases {
        { $(async fn $name:ident() $body:block)+ } => {
            $(
                #[test]
                fn $name() {
                    async fn test() {
                        $body
                    }

                    assert_eq!(::actix::System::run(||{
                        use futures::{compat::*, prelude::*};
                        use tokio::prelude::{
                            Future as Future01,
                            FutureExt as Future01Ext,
                        };

                        let f = test().unit_error().boxed().compat(TokioDefaultSpawn);
                        let f = f.deadline(Instant::now() + Duration::from_secs(5));

                        Arbiter::spawn(f.then(|r| {
                            r.unwrap();
                            System::current().stop();
                            Ok(())
                        }));
                    }), 0);
                }
            )*
        }
    }
}
