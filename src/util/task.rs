use std::marker::Unpin;

use actix::prelude::*;
use futures::{compat::*, prelude::*};


pub fn spawn<F>(fut: F) where F: Future<Output = ()> + Unpin + 'static {
    Arbiter::spawn(fut.unit_error().compat(TokioDefaultSpawn));
}
