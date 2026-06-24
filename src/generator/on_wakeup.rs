use std::collections::HashMap;
use crate::common::wireguard::{Route, Wireguard};
use crate::generator::config::Config;

///ok, so here's the plan:
/// 1. figure out if we're connected.
/// 2. if we're connected, try to ping the endpoint. if good, return, else, continue.
/// 3. return the 10.69.0.1 endpoint to it's wg peer.
/// 4. test every peer. if it's connected, give it the 10.69.0.1 allowed ip.
pub fn do_wakeup(x: &mut Config, x0: &mut HashMap<String, Route>, x1: &mut Wireguard) {
    for i in &x1.peers {
    }
}