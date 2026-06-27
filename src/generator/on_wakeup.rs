use std::collections::HashMap;
use crate::common::wireguard::{Route, Wireguard};
use crate::generator::config::Config;
use crate::generator::main::ping;

pub fn do_wakeup(x: &mut Config, x0: &mut HashMap<String, Vec<Route>>, x1: &mut Wireguard) {
    for i in &x1.peers {
        for i in &i.allowed_ips {
            if i.prefix_len() == i.max_prefix_len() {
                let _ = ping(i.addr());
            }
        }
    }
}