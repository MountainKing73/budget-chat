use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection { stream }
    }

    pub async fn write_message(&mut self, s: &str) {
        self.stream
            .write_all(s.as_bytes())
            .await
            .expect("Failed writing response");
    }

    pub async fn read_message(&mut self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(1024);
        let _ = self
            .stream
            .read_buf(&mut buf)
            .await
            .expect("Failed to read from client");

        buf
    }
}
