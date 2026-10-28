//! Eine Verbindung, von der schon etwas gelesen wurde: Das Gelesene kommt beim Lesen zuerst wieder,
//! damit der WebSocket-Handschlag den ganzen Kopf sieht.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

pub struct MitVorspann {
    vorspann: Vec<u8>,
    gelesen: usize,
    innen: TcpStream,
}

impl MitVorspann {
    pub fn neu(vorspann: Vec<u8>, innen: TcpStream) -> Self {
        Self {
            vorspann,
            gelesen: 0,
            innen,
        }
    }
}

impl AsyncRead for MitVorspann {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.gelesen < self.vorspann.len() {
            let n = buf.remaining().min(self.vorspann.len() - self.gelesen);
            let von = self.gelesen;
            buf.put_slice(&self.vorspann[von..von + n]);
            self.gelesen += n;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.innen).poll_read(cx, buf)
    }
}

impl AsyncWrite for MitVorspann {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.innen).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.innen).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.innen).poll_shutdown(cx)
    }
}
