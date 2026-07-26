use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct LoopbackResponses {
    base_url: String,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: Option<Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl LoopbackResponses {
    pub fn start(responses: Vec<serde_json::Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback responses server");
        listener
            .set_nonblocking(true)
            .expect("configure nonblocking listener");
        let address = listener.local_addr().expect("loopback address");
        let responses = responses
            .into_iter()
            .map(|response| serde_json::to_vec(&response).expect("response fixture"))
            .collect::<Vec<_>>();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let (stop, stopped) = mpsc::channel();
        let handle = thread::spawn(move || {
            for response in responses {
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            match stopped.try_recv() {
                                Ok(()) | Err(TryRecvError::Disconnected) => return,
                                Err(TryRecvError::Empty) => {}
                            }
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => return,
                    }
                };
                let request = read_request(&mut stream);
                captured.lock().expect("request capture lock").push(request);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.len()
                );
                if stream.write_all(header.as_bytes()).is_err()
                    || stream.write_all(&response).is_err()
                {
                    return;
                }
            }
        });
        Self {
            base_url: format!("http://{address}/v1"),
            requests,
            stop: Some(stop),
            handle: Some(handle),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn finish(mut self) -> Vec<Vec<u8>> {
        self.stop_server();
        self.requests.lock().expect("request capture lock").clone()
    }

    fn stop_server(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(handle) = self.handle.take() {
            handle.join().expect("loopback responses thread");
        }
    }
}

impl Drop for LoopbackResponses {
    fn drop(&mut self) {
        self.stop_server();
    }
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_nonblocking(false)
        .expect("configure blocking request stream");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set request timeout");
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer).expect("read request");
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let body_start = header_end + 4;
        let headers = String::from_utf8_lossy(&request[..body_start]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if request.len() >= body_start + content_length {
            break;
        }
    }
    request
}
