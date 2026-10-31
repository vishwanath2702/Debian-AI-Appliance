//! Bounded loopback HTTP transport for Ollama benchmarks.

use crate::benchmark_ollama::{
    MAX_HTTP_RESPONSE_BYTES, http_response_total_length, parse_http_response,
};
use model::BenchmarkError;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::time::{Duration, Instant};

const OLLAMA_PORT: u16 = 11434;
const MAX_HTTP_REQUEST_BODY_BYTES: usize = 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const IO_TIMEOUT: Duration = Duration::from_secs(2);
const INFERENCE_DEADLINE: Duration = Duration::from_secs(300);

/// Sends a non-streaming generate request to the local Ollama service.
///
/// # Errors
///
/// Returns an execution error on connection, transport, deadline, or HTTP failure.
pub fn post_generate(body: &[u8]) -> Result<Vec<u8>, BenchmarkError> {
    post_generate_to_port(OLLAMA_PORT, body)
}

/// Sends a benchmark request to a selected IPv4 loopback port.
///
/// # Errors
///
/// Returns an execution error for an invalid port or transport failure.
pub fn post_generate_to_port(port: u16, body: &[u8]) -> Result<Vec<u8>, BenchmarkError> {
    if port == 0 {
        return Err(BenchmarkError::InvalidConfiguration("ollama_port"));
    }
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    post_to_address(address, body, INFERENCE_DEADLINE)
}

fn write_with_deadline<W: Write>(
    writer: &mut W,
    mut bytes: &[u8],
    started: Instant,
    deadline: Duration,
    mut set_timeout: impl FnMut(&mut W, Duration) -> std::io::Result<()>,
) -> Result<(), BenchmarkError> {
    while !bytes.is_empty() {
        if started.elapsed() >= deadline {
            return Err(BenchmarkError::Execution(
                "Ollama inference deadline exceeded".into(),
            ));
        }

        let remaining = deadline.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(BenchmarkError::Execution(
                "Ollama inference deadline exceeded".into(),
            ));
        }

        set_timeout(writer, remaining.min(IO_TIMEOUT)).map_err(|error| {
            BenchmarkError::Execution(format!("Ollama write timeout setup failed: {error}"))
        })?;

        match writer.write(bytes) {
            Ok(0) => {
                return Err(BenchmarkError::Execution(
                    "Ollama request write returned zero bytes".into(),
                ));
            }
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error)
                if error.kind() == std::io::ErrorKind::TimedOut
                    || error.kind() == std::io::ErrorKind::WouldBlock =>
            {
                continue;
            }
            Err(error) => {
                return Err(BenchmarkError::Execution(format!(
                    "Ollama request failed: {error}"
                )));
            }
        }
    }

    Ok(())
}

fn post_to_address(
    address: SocketAddrV4,
    body: &[u8],
    deadline: Duration,
) -> Result<Vec<u8>, BenchmarkError> {
    if body.len() > MAX_HTTP_REQUEST_BODY_BYTES {
        return Err(BenchmarkError::Execution(
            "Ollama HTTP request exceeds size limit".into(),
        ));
    }

    let started = Instant::now();

    if deadline.is_zero() {
        return Err(BenchmarkError::Execution(
            "Ollama inference deadline exceeded".into(),
        ));
    }

    let connect_timeout = deadline.min(CONNECT_TIMEOUT);

    let mut stream = TcpStream::connect_timeout(&address.into(), connect_timeout)
        .map_err(|error| BenchmarkError::Execution(format!("Ollama connection failed: {error}")))?;

    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|error| BenchmarkError::Execution(error.to_string()))?;
    let headers = format!(
        "POST /api/generate HTTP/1.1\r\n\
         Host: 127.0.0.1:11434\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );

    write_with_deadline(
        &mut stream,
        headers.as_bytes(),
        started,
        deadline,
        |socket, timeout| socket.set_write_timeout(Some(timeout)),
    )?;
    write_with_deadline(&mut stream, body, started, deadline, |socket, timeout| {
        socket.set_write_timeout(Some(timeout))
    })?;

    let mut response = Vec::new();
    let mut buffer = [0_u8; 8192];

    loop {
        if started.elapsed() >= deadline {
            return Err(BenchmarkError::Execution(
                "Ollama inference deadline exceeded".into(),
            ));
        }

        let remaining = deadline.saturating_sub(started.elapsed());
        let timeout = remaining.min(IO_TIMEOUT);
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|error| BenchmarkError::Execution(error.to_string()))?;

        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                if count > MAX_HTTP_RESPONSE_BYTES.saturating_sub(response.len()) {
                    return Err(BenchmarkError::Execution(
                        "Ollama HTTP response exceeds size limit".into(),
                    ));
                }
                response.extend_from_slice(&buffer[..count]);

                if let Some(expected) = http_response_total_length(&response)? {
                    if response.len() >= expected {
                        return parse_http_response(&response).map(ToOwned::to_owned);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error)
                if error.kind() == std::io::ErrorKind::TimedOut
                    || error.kind() == std::io::ErrorKind::WouldBlock =>
            {
                continue;
            }
            Err(error) => {
                return Err(BenchmarkError::Execution(format!(
                    "Ollama response read failed: {error}"
                )));
            }
        }
    }

    parse_http_response(&response).map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn writes_complete_request_with_partial_writes() {
        struct PartialWriter {
            bytes: Vec<u8>,
        }

        impl Write for PartialWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                let count = bytes.len().min(2);
                self.bytes.extend_from_slice(&bytes[..count]);
                Ok(count)
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut writer = PartialWriter { bytes: Vec::new() };

        write_with_deadline(
            &mut writer,
            b"abcdef",
            Instant::now(),
            Duration::from_secs(1),
            |_, _| Ok(()),
        )
        .unwrap();

        assert_eq!(writer.bytes, b"abcdef");
    }

    #[test]
    fn rejects_write_after_deadline() {
        let mut writer = Vec::new();

        assert_eq!(
            write_with_deadline(
                &mut writer,
                b"abc",
                Instant::now(),
                Duration::ZERO,
                |_, _| Ok(()),
            ),
            Err(BenchmarkError::Execution(
                "Ollama inference deadline exceeded".into()
            ))
        );

        assert!(writer.is_empty());
    }

    #[test]
    fn rejects_zero_byte_write() {
        struct ZeroWriter;

        impl Write for ZeroWriter {
            fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
                Ok(0)
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        assert_eq!(
            write_with_deadline(
                &mut ZeroWriter,
                b"abc",
                Instant::now(),
                Duration::from_secs(1),
                |_, _| Ok(()),
            ),
            Err(BenchmarkError::Execution(
                "Ollama request write returned zero bytes".into()
            ))
        );
    }

    #[test]
    fn bounds_each_write_timeout_by_remaining_deadline() {
        struct PartialWriter {
            calls: usize,
        }

        impl Write for PartialWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.calls += 1;
                Ok(bytes.len().min(1))
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut writer = PartialWriter { calls: 0 };
        let mut timeouts = Vec::new();
        let deadline = Duration::from_secs(1);

        write_with_deadline(
            &mut writer,
            b"abc",
            Instant::now(),
            deadline,
            |_, timeout| {
                timeouts.push(timeout);
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(writer.calls, 3);
        assert_eq!(timeouts.len(), 3);
        assert!(timeouts.iter().all(|timeout| {
            !timeout.is_zero() && *timeout <= deadline && *timeout <= IO_TIMEOUT
        }));
    }

    #[test]
    fn rejects_write_timeout_setup_failure() {
        let mut writer = Vec::new();

        let result = write_with_deadline(
            &mut writer,
            b"abc",
            Instant::now(),
            Duration::from_secs(1),
            |_, _| Err(std::io::Error::other("timeout setup failed")),
        );

        assert_eq!(
            result,
            Err(BenchmarkError::Execution(
                "Ollama write timeout setup failed: timeout setup failed".into()
            ))
        );
        assert!(writer.is_empty());
    }

    #[test]
    fn rejects_zero_deadline_before_connecting() {
        let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);

        assert_eq!(
            post_to_address(address, b"{}", Duration::ZERO),
            Err(BenchmarkError::Execution(
                "Ollama inference deadline exceeded".into()
            ))
        );
    }

    #[test]
    fn rejects_oversized_request_before_connecting() {
        let oversized = vec![b'x'; MAX_HTTP_REQUEST_BODY_BYTES + 1];
        let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);

        assert_eq!(
            post_to_address(address, &oversized, Duration::from_secs(1)),
            Err(BenchmarkError::Execution(
                "Ollama HTTP request exceeds size limit".into()
            ))
        );
    }

    #[test]
    fn sends_generate_request_and_reads_response() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = match listener.local_addr().unwrap() {
            std::net::SocketAddr::V4(address) => address,
            _ => unreachable!(),
        };

        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(IO_TIMEOUT)).unwrap();
            let mut bytes = [0_u8; 2048];
            let count = socket.read(&mut bytes).unwrap();
            let request = String::from_utf8_lossy(&bytes[..count]);

            assert!(request.starts_with("POST /api/generate HTTP/1.1\r\n"));
            assert!(request.contains("Content-Type: application/json\r\n"));
            assert!(request.contains("Content-Length: 2\r\n"));

            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                .unwrap();
        });

        let result = post_to_address(address, b"{}", Duration::from_secs(5));
        server.join().unwrap();
        assert_eq!(result.unwrap(), b"{}");
    }

    #[test]
    fn stalled_response_respects_inference_deadline() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = match listener.local_addr().unwrap() {
            std::net::SocketAddr::V4(address) => address,
            _ => unreachable!(),
        };

        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(IO_TIMEOUT)).unwrap();

            let mut request = [0_u8; 2048];
            let count = socket.read(&mut request).unwrap();
            assert!(count > 0);

            // Deliberately withhold the response beyond the client deadline.
            thread::sleep(Duration::from_millis(250));
        });

        let started = Instant::now();
        let result = post_to_address(address, b"{}", Duration::from_millis(50));
        let elapsed = started.elapsed();

        server.join().unwrap();

        assert_eq!(
            result,
            Err(BenchmarkError::Execution(
                "Ollama inference deadline exceeded".into()
            ))
        );
        assert!(
            elapsed < Duration::from_millis(200),
            "client waited too long for a stalled response: {elapsed:?}"
        );
    }

    #[test]
    fn completes_response_without_waiting_for_connection_close() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = match listener.local_addr().unwrap() {
            std::net::SocketAddr::V4(address) => address,
            _ => unreachable!(),
        };

        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(IO_TIMEOUT)).unwrap();

            let mut request = [0_u8; 2048];
            let count = socket.read(&mut request).unwrap();
            assert!(count > 0);

            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                .unwrap();

            // Keep the connection open longer than the client's deadline.
            thread::sleep(Duration::from_millis(500));
        });

        let result = post_to_address(address, b"{}", Duration::from_millis(200));
        server.join().unwrap();

        assert_eq!(result.unwrap(), b"{}");
    }

    #[test]
    fn rejects_oversized_response_during_read() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = match listener.local_addr().unwrap() {
            std::net::SocketAddr::V4(address) => address,
            _ => unreachable!(),
        };

        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(IO_TIMEOUT)).unwrap();

            let mut request = Vec::new();
            let mut bytes = [0_u8; 2048];

            loop {
                let count = socket.read(&mut bytes).unwrap();
                assert!(count > 0, "request ended before HTTP headers");

                request.extend_from_slice(&bytes[..count]);

                if let Some(boundary) = request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let headers = std::str::from_utf8(&request[..boundary]).unwrap();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("Content-Length: ")
                                .and_then(|value| value.parse::<usize>().ok())
                        })
                        .unwrap();

                    if request.len() >= boundary + 4 + length {
                        break;
                    }
                }

                assert!(request.len() < 8192, "unexpectedly large test request");
            }

            let oversized = vec![b'x'; MAX_HTTP_RESPONSE_BYTES + 8192];
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                oversized.len()
            );

            socket.write_all(headers.as_bytes()).unwrap();
            let _ = socket.write_all(&oversized);
        });

        let result = post_to_address(address, b"{}", Duration::from_secs(5));
        server.join().unwrap();

        assert_eq!(
            result,
            Err(BenchmarkError::Execution(
                "Ollama HTTP response exceeds size limit".into()
            ))
        );
    }

    #[test]
    fn executes_complete_benchmark_over_mock_http_server() {
        use crate::benchmark::BenchmarkExecutor;
        use crate::benchmark_ollama_runner::OllamaBenchmarkRunner;
        use model::{
            BenchmarkConfiguration, BenchmarkRequest, BenchmarkRunKind, InferenceEngineId,
        };
        use serde_json::{Value, json};

        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = match listener.local_addr().unwrap() {
            std::net::SocketAddr::V4(address) => address,
            _ => unreachable!(),
        };

        let server = thread::spawn(move || {
            for index in 0..3_u64 {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(IO_TIMEOUT)).unwrap();

                let mut request = Vec::new();
                let mut buffer = [0_u8; 2048];

                let body_start = loop {
                    let count = socket.read(&mut buffer).unwrap();
                    assert!(count > 0, "incomplete benchmark HTTP request");
                    request.extend_from_slice(&buffer[..count]);

                    assert!(request.len() < 8192, "test request too large");

                    if let Some(boundary) = request.windows(4).position(|part| part == b"\r\n\r\n")
                    {
                        let header = std::str::from_utf8(&request[..boundary]).unwrap();
                        assert!(header.starts_with("POST /api/generate HTTP/1.1\r\n"));

                        let length = header
                            .split("\r\n")
                            .find_map(|line| {
                                line.strip_prefix("Content-Length: ")
                                    .and_then(|value| value.parse::<usize>().ok())
                            })
                            .unwrap();

                        if request.len() >= boundary + 4 + length {
                            assert_eq!(request.len(), boundary + 4 + length);
                            break boundary + 4;
                        }
                    }
                };

                let payload: Value = serde_json::from_slice(&request[body_start..]).unwrap();

                assert_eq!(payload["model"], "example:latest");
                assert_eq!(payload["prompt"], "Explain CPU scheduling");
                assert_eq!(payload["stream"], false);
                assert_eq!(payload["think"], false);
                assert_eq!(payload["options"]["num_ctx"], 8192);
                assert_eq!(payload["options"]["num_thread"], 8);
                assert_eq!(payload["options"]["num_predict"], 96);
                assert_eq!(payload["options"]["temperature"], 0.0);
                assert_eq!(payload["options"]["seed"], 42);

                let response = serde_json::to_vec(&json!({
                    "done": true,
                    "done_reason": "length",
                    "prompt_eval_count": 50,
                    "eval_count": 96,
                    "prompt_eval_duration": 100,
                    "eval_duration": 3_500_000_000_u64 + index,
                    "total_duration": 3_600_000_000_u64 + index,
                    "load_duration": 0
                }))
                .unwrap();

                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                    response.len()
                );

                socket.write_all(header.as_bytes()).unwrap();
                socket.write_all(&response).unwrap();
            }
        });

        let request = BenchmarkRequest {
            model_name: "example:latest".into(),
            engine: InferenceEngineId::ollama(),
            prompt: "Explain CPU scheduling".into(),
            configuration: BenchmarkConfiguration {
                context_length: 8192,
                threads: 8,
                max_output_tokens: 96,
                temperature: 0.0,
                seed: 42,
                measured_repetitions: 2,
            },
        };

        let runner = OllamaBenchmarkRunner::new(|body: &[u8]| {
            post_to_address(address, body, Duration::from_secs(5))
        });

        let mut executor = BenchmarkExecutor::new(runner);
        let result = executor.execute(&request);

        server.join().unwrap();

        let result = result.unwrap();
        assert_eq!(result.request, request);
        assert_eq!(result.runs.len(), 3);
        assert_eq!(result.runs[0].kind, BenchmarkRunKind::Warmup);
        assert_eq!(result.runs[1].kind, BenchmarkRunKind::Measured);
        assert_eq!(result.runs[2].kind, BenchmarkRunKind::Measured);

        for (index, run) in result.runs.iter().enumerate() {
            assert_eq!(run.prompt_eval_count, 50);
            assert_eq!(run.eval_count, 96);
            assert_eq!(run.eval_duration_ns, 3_500_000_000_u64 + index as u64);
            assert_eq!(run.done_reason.as_deref(), Some("length"));
        }
    }
}
