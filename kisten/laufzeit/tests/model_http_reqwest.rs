use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use orchester_laufzeit::harness::credentials::{InMemoryCredentialStore, provider_secret};
use orchester_laufzeit::harness::provider::{
    HttpRequest, HttpTransport, HttpTransportError, ReqwestHttpTransport,
};
use tokio_util::sync::CancellationToken;
use url::Url;

const SECRET_CANARY: &str = "sk-loopback-transport-canary";
const BODY_CANARY: &str = "{\"prompt\":\"loopback body\"}";

fn request(endpoint: Url) -> HttpRequest {
    let store = InMemoryCredentialStore::with("OpenAI", SECRET_CANARY);
    let secret = provider_secret(&store, "OpenAI")
        .expect("credential lookup")
        .expect("credential is present");
    HttpRequest::new(endpoint, BODY_CANARY.as_bytes().to_vec(), Some(secret))
        .expect("bounded request")
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set read timeout");
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
        let header_end = header_end + 4;
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if request.len() >= header_end + content_length {
            break;
        }
    }
    request
}

fn serve_once(response: Vec<u8>) -> (Url, Receiver<Vec<u8>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback server");
    let address = listener.local_addr().expect("loopback address");
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let captured = read_request(&mut stream);
        let _ = sender.send(captured);
        let _ = stream.write_all(&response);
    });
    (
        Url::parse(&format!("http://{address}/v1/responses")).expect("loopback URL"),
        receiver,
        handle,
    )
}

#[tokio::test]
async fn posts_json_with_bearer_auth_and_returns_bounded_response() {
    let (endpoint, captured, server) = serve_once(
        b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nRetry-After: 7\r\nConnection: close\r\n\r\n{\"ok\":true}"
            .to_vec(),
    );
    let response = ReqwestHttpTransport::new()
        .expect("build client")
        .send(request(endpoint), CancellationToken::new())
        .await
        .expect("HTTP response");
    server.join().expect("server thread");

    let wire = String::from_utf8(captured.recv().expect("captured request")).expect("HTTP text");
    let wire_lower = wire.to_ascii_lowercase();
    assert!(wire.starts_with("POST /v1/responses HTTP/1.1\r\n"));
    assert!(wire_lower.contains("content-type: application/json\r\n"));
    assert!(wire_lower.contains(&format!("authorization: bearer {SECRET_CANARY}\r\n")));
    assert!(wire.ends_with(BODY_CANARY));
    assert_eq!(response.status(), 200);
    assert_eq!(response.retry_after(), Some(Duration::from_secs(7)));
    assert_eq!(response.body(), br#"{"ok":true}"#);
}

#[tokio::test]
async fn rejects_declared_and_streamed_responses_over_the_request_limit() {
    let (endpoint, declared_capture, declared_server) = serve_once(
        b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n123456789".to_vec(),
    );
    let declared = request(endpoint)
        .with_response_limit(8)
        .expect("valid response limit");
    let error = ReqwestHttpTransport::new()
        .expect("build client")
        .send(declared, CancellationToken::new())
        .await
        .unwrap_err();
    let _ = declared_capture.recv();
    declared_server.join().expect("server thread");
    assert_eq!(error, HttpTransportError::ResponseTooLarge);

    let (endpoint, streamed_capture, streamed_server) = serve_once(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\n12345\r\n4\r\n6789\r\n0\r\n\r\n"
            .to_vec(),
    );
    let streamed = request(endpoint)
        .with_response_limit(8)
        .expect("valid response limit");
    let error = ReqwestHttpTransport::new()
        .expect("build client")
        .send(streamed, CancellationToken::new())
        .await
        .unwrap_err();
    let _ = streamed_capture.recv();
    streamed_server.join().expect("server thread");
    assert_eq!(error, HttpTransportError::ResponseTooLarge);
}

#[tokio::test]
async fn redirects_are_returned_without_forwarding_authorization() {
    let (endpoint, captured, server) = serve_once(
        b"HTTP/1.1 302 Found\r\nLocation: https://redirect-canary.example/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_vec(),
    );
    let response = ReqwestHttpTransport::new()
        .expect("build client")
        .send(request(endpoint), CancellationToken::new())
        .await
        .expect("redirect response");
    let _ = captured.recv();
    server.join().expect("server thread");
    assert_eq!(response.status(), 302);
}

#[tokio::test]
async fn cancellation_wins_before_network_and_timeout_errors_are_redacted() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let cancelled = ReqwestHttpTransport::new()
        .expect("build client")
        .send(
            request(Url::parse("http://127.0.0.1:9/v1/responses").expect("URL")),
            cancel,
        )
        .await
        .unwrap_err();
    assert_eq!(cancelled, HttpTransportError::Cancelled);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind timeout server");
    let address = listener.local_addr().expect("timeout server address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept timeout request");
        let _ = read_request(&mut stream);
        thread::sleep(Duration::from_millis(100));
    });
    let timed =
        request(Url::parse(&format!("http://{address}/v1/responses")).expect("loopback URL"))
            .with_timeout(Duration::from_millis(20))
            .expect("valid timeout");
    let timeout = ReqwestHttpTransport::new()
        .expect("build client")
        .send(timed, CancellationToken::new())
        .await
        .unwrap_err();
    server.join().expect("server thread");
    assert_eq!(timeout, HttpTransportError::Timeout);

    let rendered = format!("{timeout:?} {timeout}");
    for canary in [SECRET_CANARY, BODY_CANARY, "127.0.0.1"] {
        assert!(!rendered.contains(canary));
    }
}
