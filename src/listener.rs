use std::{io, net::SocketAddr, time::Duration};

use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use topcoat::router::Listener;
use tracing::{debug, error};

/// A TCP listener tuned for the challenge's traffic and hardened against
/// transient accept failures.
///
/// Topcoat's serve loop stops for good on the first accept error, so a
/// momentary condition like running out of file descriptors under a
/// classroom-sized burst would take the whole server down. This listener
/// absorbs those errors instead and keeps accepting.
#[derive(Debug)]
pub struct ChallengeListener(TcpListener);

impl ChallengeListener {
    /// Binds to `addr`.
    ///
    /// # Errors
    /// If the address cannot be resolved or bound.
    pub async fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        TcpListener::bind(addr).await.map(Self)
    }

    /// The address actually bound, which differs from the requested one when
    /// binding port 0.
    ///
    /// # Errors
    /// If the OS cannot report the socket's address.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }
}

impl Listener for ChallengeListener {
    type Io = TcpStream;

    async fn accept(&mut self) -> io::Result<(TcpStream, Option<SocketAddr>)> {
        // The same back-off axum and hyper's examples use: long enough that a
        // descriptor-exhausted process is not spinning, short enough that
        // players barely notice.
        const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_secs(1);

        loop {
            match self.0.accept().await {
                Ok((stream, addr)) => {
                    // Nagle would sit on the tiny check responses waiting for
                    // data that is never coming; every response here fits in
                    // one write.
                    if let Err(err) = stream.set_nodelay(true) {
                        debug!(%err, "failed to set TCP_NODELAY");
                    }
                    return Ok((stream, Some(addr)));
                }
                // The peer gave up before we got to it; nothing to recover.
                Err(err) if is_connection_error(&err) => {}
                Err(err) => {
                    error!(%err, "accept failed; retrying");
                    tokio::time::sleep(ACCEPT_ERROR_BACKOFF).await;
                }
            }
        }
    }

    fn tcp_addr(&self) -> Option<SocketAddr> {
        self.0.local_addr().ok()
    }
}

fn is_connection_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}
