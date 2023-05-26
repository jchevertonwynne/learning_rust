use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use waitgroup::{WaitGroup, Worker};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:25565").await?;

    let state = Arc::new(State::default());

    let token = CancellationToken::new();

    let wg = WaitGroup::new();

    loop {
        let (stream, socket): (TcpStream, SocketAddr) = select! {
            conn = listener.accept() => {
                conn?
            },
            _ = tokio::signal::ctrl_c() => {
                token.cancel();
                break;
            }
        };

        let clients = state.active_clients.fetch_add(1, SeqCst) + 1;
        println!("{clients} clients are connected");

        tokio::spawn(handle_client(
            stream,
            socket,
            state.clone(),
            token.clone(),
            wg.worker(),
        ));
    }

    wg.wait().await;

    Ok(())
}

#[derive(Default)]
struct State {
    active_clients: AtomicUsize,
    line_counter: AtomicUsize,
    all_input: Mutex<String>,
}

async fn handle_client(
    stream: TcpStream,
    socket: SocketAddr,
    state: Arc<State>,
    token: CancellationToken,
    worker: Worker,
) {
    println!("new client connected: {socket}");

    let (read, mut write) = tokio::io::split(stream);
    let mut read = BufReader::new(read);
    loop {
        let mut buffer = String::new();
        let read_line = select! {
            _ = token.cancelled() => {
                break;
            },
            read_line = read.read_line(&mut buffer) => {
                read_line
            }
        };

        let read = match read_line {
            Ok(n) => n,
            Err(err) => {
                println!("Error reading from socket: {:?}", err);
                break;
            }
        };

        if read == 0 {
            println!("Client disconnected: {socket}");
            break;
        }

        let count = state.line_counter.fetch_add(1, SeqCst) + 1;
        println!(
            "Received line {count} from client {}: `{}`",
            socket,
            buffer.trim()
        );
        write.write_all(buffer.as_bytes()).await.unwrap();

        let mut lock = state.all_input.lock().await;
        lock.push_str(&buffer);
        println!("====\nall input so far:\n{}\n======", lock.as_str());
    }

    state.active_clients.fetch_sub(1, SeqCst);
    drop(worker)
}
