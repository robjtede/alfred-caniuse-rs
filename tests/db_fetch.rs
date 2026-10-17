use std::{
    io::{self, BufRead as _, BufReader, Write as _},
    net::{Shutdown, TcpListener},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use alfred_caniuse_rs::Db;

#[test]
fn fetch_times_out_while_waiting_for_response_headers() {
    assert_fetch_times_out(b"");
}

#[test]
fn fetch_times_out_while_reading_response_body() {
    assert_fetch_times_out(b"HTTP/1.1 200 OK\r\nContent-Length: 128\r\n\r\n{\"versions\":");
}

fn assert_fetch_times_out(response: &[u8]) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();

    let url = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();

    thread::scope(move |scope| {
        let worker = scope.spawn(move || sender.send(Db::fetch(&url)).unwrap());
        let started = Instant::now();

        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,

                Err(err)
                    if err.kind() == io::ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(5) =>
                {
                    thread::sleep(Duration::from_millis(10));
                }

                Err(err) => panic!("did not receive the database request: {err}"),
            }
        };

        drop(listener);

        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        let mut reader = BufReader::new(&mut stream);
        let mut request = String::new();

        while !request.ends_with("\r\n\r\n") {
            assert_ne!(reader.read_line(&mut request).unwrap(), 0);
        }

        stream.write_all(response).unwrap();

        // Close the connection after the test deadline so the baseline worker can exit.
        let result = receiver.recv_timeout(Duration::from_secs(5));
        let _ = stream.shutdown(Shutdown::Both);
        drop(stream);
        worker.join().unwrap();

        let err = result
            .expect("database fetch did not return within five seconds")
            .unwrap_err();

        assert!(
            matches!(
                err.downcast_ref::<ureq::Error>(),
                Some(ureq::Error::Timeout(ureq::Timeout::Global))
            ),
            "expected a database fetch timeout, got {err}"
        );
    });
}
