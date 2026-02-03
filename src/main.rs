use core::task::Poll;
use core::pin::pin;

use std::io;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::io::AsyncWriteExt;

use openssl_lite::{SslCtx, AsyncSsl};

fn help() -> ! {
    eprintln!("Usage: bmcproxy <listenaddr> <BMCaddr>");
    std::process::exit(1);
}

fn main() -> io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main())
}

// Modified connection loop from dhttp
async fn async_main() -> io::Result<()> {
    let mut args = std::env::args();
    args.next();
    let Some(hostaddr) = args.next() else { help() };
    let Some(bmcaddr) = args.next() else { help() };
    if args.next().is_some() { help(); }

    let server = TcpListener::bind(hostaddr).await?;
    let mut err_shown = false;
    loop {
        // This way, shutdown is handled gracefully
        let mut accept = pin!(server.accept());
        let mut ctrlc = pin!(tokio::signal::ctrl_c());

        // Very hell-ish variant of the join macro/Or future
        let result = std::future::poll_fn(|cx| {
            if let Poll::Ready(v) = accept.as_mut().poll(cx) {
                return Poll::Ready(Ok(v));
            }
            if let Poll::Ready(v) = ctrlc.as_mut().poll(cx) {
                return Poll::Ready(Err(v));
            }
            Poll::Pending
        }).await;

        if result.is_err() { break; }

        match result.unwrap() {
            Ok((conn, _addr)) => {
                err_shown = false;
                let bmcaddr2 = bmcaddr.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle(bmcaddr2, conn).await {
                        eprintln!("{err}");
                    }
                });
            }
            Err(e) => {
                // this may fire when fd limit is exhausted
                if !err_shown {
                    println!("BMCproxy critical error: connection not accepted: {e}");
                    err_shown = true;
                }
                let d = Duration::from_millis(100);
                tokio::time::sleep(d).await;
            }
        };
    }

    Ok(())
}

async fn handle(bmcaddr: String, mut server_conn: TcpStream) -> io::Result<()> {
    let mut client_ctx = SslCtx::new()?;
    client_ctx.set_min_version(openssl_lite::version::TLS1_VERSION)?;
    client_ctx.set_cipher_list(c"DEFAULT:@SECLEVEL=0")?;
    client_ctx.set_options(openssl_lite::op::SSL_OP_LEGACY_SERVER_CONNECT | openssl_lite::op::SSL_OP_IGNORE_UNEXPECTED_EOF);
    client_ctx.set_verify(false);

    let client_conn = TcpStream::connect(bmcaddr).await?;
    client_conn.set_nodelay(true)?;
    let mut client_ssl = AsyncSsl::new(&client_ctx, client_conn)?;
    client_ssl.connect().await?;

    tokio::io::copy_bidirectional(&mut client_ssl, &mut server_conn).await?;
    client_ssl.shutdown().await?;
    Ok(())
}
