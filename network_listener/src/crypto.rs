use std::fs;
use std::io::{BufReader, Error, Read};
use std::net::Shutdown;
use std::sync::Arc;

use crate::client_handler::Client;
use crate::messages_request::asd::Request;
use bytes::BytesMut;
use log::{error, info, trace, warn};
use rustls::{NoClientAuth, RootCertStore, ServerConfig, TLSError};

pub struct TlsServer {
    listener: mio::net::TcpListener,
    config: Arc<ServerConfig>,
}

impl TlsServer {
    pub fn new(socket: mio::net::TcpListener) -> TlsServer {
        let client_verifier = NoClientAuth::new();
        //TODO Need to provide client cert authentication
        warn!("Need to do client verification");
        let mut config = ServerConfig::new(client_verifier);
        config
            .set_single_cert(
                load_certs("../certs/cert/my_cert.crt"),
                load_private_key("../certs/cert/priv.key"),
            )
            .unwrap();
        TlsServer {
            listener: socket,
            config: Arc::new(config),
        }
    }
    pub fn accept_connection(&mut self) -> Client {
        let session = rustls::ServerSession::new(&self.config);
        let (stream, addr) = self.listener.accept().unwrap();
        let conn = TlsConnection::new(stream, Box::from(session));
        info!("New client connection from: {}", addr);
        Client::new(addr.to_string(), conn)
    }
}

pub struct TlsConnection {
    pub socket: mio::net::TcpStream,
    tls_session: Box<dyn rustls::Session>,
    closing: bool,
}

impl TlsConnection {
    pub fn new(
        socket: mio::net::TcpStream,
        tls_session: Box<dyn rustls::Session>,
    ) -> TlsConnection {
        TlsConnection {
            socket,
            tls_session,
            closing: false,
        }
    }
    fn read_tls(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let tls_data = self.tls_session.read_tls(&mut self.socket);
        match tls_data {
            Ok(n) => {
                if n == 0 {
                    warn!("No data read from socket");
                    self.closing = true;
                    return Err(Box::from(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "No data has been read",
                    )));
                } else {
                    let processed = self.tls_session.process_new_packets();
                    match processed {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Failed to process TLS packets ({})", e);
                            //TODO Send any unsent messages?
                            self.closing = true;
                            return Err(Box::from(e));
                        }
                    };
                }
            }
            Err(e) => {
                return Err(Box::from(e));
            }
        }
        Ok(())
    }
    pub fn read_plaintext(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        self.read_tls()?;
        Ok(self.tls_session.read(buffer)?)
    }
    pub fn check_write(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let req = self.tls_session.wants_write();
        if req {
            let n = self.tls_session.write_tls(&mut self.socket)?;
        }
        Ok(())
    }

    pub fn write_message(&mut self, msg: &Request) -> Result<(), Box<dyn std::error::Error>> {
        //let mut buffer = None;
        let mut buffer = BytesMut::with_capacity(1024);
        msg.encode_length_delimeted(buffer);
        let data = msg.data.get(0).unwrap().as_bytes();
        let size = data.len() as u16;
        let size_bytes = size.to_be_bytes();

        self.tls_session.write(&size_bytes)?;
        let buffer_bytes = self.tls_session.write(data)?;
        self.tls_session.flush()?;
        let mut written_bytes = self.tls_session.write_tls(&mut self.socket)?;
        trace!("Put {} out of {} in buffer", buffer_bytes, size);
        trace!("Checking if output buffer is empty...");
        while self.tls_session.wants_write() {
            written_bytes += self.tls_session.write_tls(&mut self.socket)?;
        }
        info!(
            "Sent message {} to {}, with {} out of {} bytes sent",
            msg,
            self.socket.peer_addr()?,
            written_bytes,
            size + 2
        );
        Ok(())
    }
    pub fn shutdown(&mut self) -> Result<(), Error> {
        self.socket.shutdown(Shutdown::Both)
    }
}

fn load_private_key(filename: &str) -> rustls::PrivateKey {
    let keyfile = fs::File::open(filename).expect("cannot open private key file");
    let mut reader = BufReader::new(keyfile);
    let key = rustls::internal::pemfile::rsa_private_keys(&mut reader)
        .expect("file contains invalid rsa private key");
    println!("Key size: {}", key.len());
    key[0].clone()
}

fn load_certs(filename: &str) -> Vec<rustls::Certificate> {
    let certfile = fs::File::open(filename).expect("cannot open certificate file");
    let mut reader = BufReader::new(certfile);
    rustls::internal::pemfile::certs(&mut reader).unwrap()
}
