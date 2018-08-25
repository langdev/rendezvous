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
