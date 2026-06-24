use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::thread;
use crossbeam_channel::{unbounded, Receiver};

pub fn prep_receive_connections(port: u16) -> Receiver<(TcpStream, SocketAddr)> {
    let (send, recv) = unbounded();
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).unwrap();
    thread::spawn(move || {
        let send = send;
        loop {
            if send.send(listener.accept().unwrap()).is_err() {
                return;
            }
        }
    });
    recv
}
pub fn prep_receive_udp_connection(port: u16) -> Receiver<(SocketAddr, Vec<u8>)> {
    let (send, recv) = unbounded();
    thread::spawn(move || {
        let udp = UdpSocket::bind(format!("0.0.0.0:{}", port)).unwrap();
        loop {
            let mut data = vec![0u8; 1024];
            let (len, src) = udp.recv_from(&mut data).unwrap();
            println!("got udp packet from {} len {}!", src, len);
            data.resize(len, 0);
            send.send((src, data)).expect("");
        }
    });
    recv
}