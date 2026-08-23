use super::*;
use std::{
    collections::VecDeque,
    io::{self, Cursor, Read, Write},
    sync::Mutex,
    time::Duration,
};
struct Script {
    response: Mutex<VecDeque<Vec<u8>>>,
}
impl Connector for Script {
    fn connect(&self, _: &Url, _: &Policy) -> Result<Box<dyn Connection>> {
        Ok(Box::new(Fake {
            read: Cursor::new(self.response.lock().unwrap().pop_front().unwrap()),
            written: Vec::new(),
        }))
    }
}
struct Fake {
    read: Cursor<Vec<u8>>,
    written: Vec<u8>,
}
impl Read for Fake {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.read.read(b)
    }
}
impl Write for Fake {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.written.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Connection for Fake {
    fn set_read_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
    fn set_write_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
}
fn request(url: &str) -> Request<'static> {
    Request {
        method: Method::get(),
        url: Url::parse(url).unwrap(),
        headers: vec![],
        body: RequestBody::Empty,
        deadline: None,
        cancellation: Cancellation::default(),
    }
}
#[test]
fn hostile_contracts_fail_at_boundary() {
    assert_eq!(
        Url::parse("http://u:p@host/").unwrap_err(),
        Error::UserInfoForbidden
    );
    assert!(Header::new("X\r\nBad", "x").is_err());
    let cancel = Cancellation::default();
    cancel.cancel();
    let client = Client::new(
        Script {
            response: Mutex::new(VecDeque::new()),
        },
        Policy::default(),
    );
    let mut r = request("http://host/");
    r.cancellation = cancel;
    assert_eq!(client.execute(r).unwrap_err(), Error::Cancelled)
}
#[test]
fn rejects_ambiguous_and_oversize_responses() {
    for raw in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"
            .to_vec(),
        b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n123456789".to_vec(),
    ] {
        let script = Script {
            response: Mutex::new(VecDeque::from([raw])),
        };
        let policy = Policy {
            max_response_bytes: 4,
            ..Policy::default()
        };
        let client = Client::new(script, policy);
        assert!(client.execute(request("http://host/")).is_err())
    }
}
#[test]
fn sensitive_debug_is_redacted() {
    let h = Header::sensitive("Authorization", "Bearer secret").unwrap();
    let text = format!("{h:?}");
    assert!(!text.contains("secret"));
    assert!(text.contains("REDACTED"))
}

#[test]
fn authority_expansion_and_encoded_bodies_fail_closed() {
    for raw in [
        b"HTTP/1.1 302 Found\r\nLocation: http://other.invalid/\r\nContent-Length: 0\r\n\r\n"
            .to_vec(),
        b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 4\r\n\r\nbomb".to_vec(),
    ] {
        let client = Client::new(
            Script {
                response: Mutex::new(VecDeque::from([raw])),
            },
            Policy::default(),
        );
        assert!(matches!(
            client.execute(request("http://host/")),
            Err(Error::RedirectDenied | Error::DecompressionLimit { .. })
        ));
    }
}

#[test]
fn partial_and_disconnected_bodies_are_bounded_once() {
    let client = Client::new(
        Script {
            response: Mutex::new(VecDeque::from([
                b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\npart".to_vec(),
            ])),
        },
        Policy::default(),
    );
    assert!(
        matches!(client.execute(request("http://host/")), Err(Error::Protocol(message)) if message.contains("truncated"))
    );
}

#[test]
fn streaming_requests_obey_the_request_cap() {
    let client = Client::new(
        Script {
            response: Mutex::new(VecDeque::from([
                b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
            ])),
        },
        Policy {
            max_request_bytes: 3,
            ..Policy::default()
        },
    );
    let mut body = Cursor::new(b"four".to_vec());
    let request = Request {
        method: Method::post(),
        url: Url::parse("http://host/").unwrap(),
        headers: Vec::new(),
        body: RequestBody::Stream(&mut body),
        deadline: None,
        cancellation: Cancellation::default(),
    };
    assert_eq!(
        client.execute(request).unwrap_err(),
        Error::RequestTooLarge { cap: 3 }
    );
}
