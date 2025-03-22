use std::net::SocketAddr;

#[derive(Debug)]
pub struct Config {
    pub id: u32,
    pub number_of_nodes: u32,
    pub local_addr: SocketAddr,
}
impl Config {
    pub fn new(id: u32, number_of_nodes: u32, addr: SocketAddr) -> Self {
        Self {
            id,
            number_of_nodes,
            local_addr: addr,
        }
    }
}
