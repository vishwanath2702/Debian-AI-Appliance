//! Ollama benchmark protocol encoding and response validation.

use model::{BenchmarkError, BenchmarkRequest, BenchmarkRun, BenchmarkRunKind};
use serde_json::{Value, json};

/// Constructs a non-streaming Ollama generate request.
///
/// The caller must validate the benchmark request before invoking this function.
pub fn encode_generate_request(request: &BenchmarkRequest) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "model": request.model_name,
        "prompt": request.prompt,
        "stream": false,
        "think": false,
        "keep_alive": "10m",
        "options": {
            "num_ctx": request.configuration.context_length,
            "num_thread": request.configuration.threads,
            "num_predict": request.configuration.max_output_tokens,
            "temperature": request.configuration.temperature,
            "seed": request.configuration.seed,
        }
    }))
    .expect("validated benchmark request must be serializable")
}

/// Parses Ollama's final non-streaming inference response.
///
/// # Errors
///
/// Rejects malformed JSON, incomplete responses, and missing timing counters.
pub fn decode_generate_response(
    body: &[u8],
    kind: BenchmarkRunKind,
) -> Result<BenchmarkRun, BenchmarkError> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| BenchmarkError::Execution("invalid Ollama JSON response".into()))?;

    if value.get("done").and_then(Value::as_bool) != Some(true) {
        return Err(BenchmarkError::Execution(
            "Ollama response is not complete".into(),
        ));
    }

    fn counter(value: &Value, name: &str) -> Result<u64, BenchmarkError> {
        value.get(name).and_then(Value::as_u64).ok_or_else(|| {
            BenchmarkError::Execution(format!("missing or invalid Ollama counter: {name}"))
        })
    }

    let done_reason = match value.get("done_reason") {
        None | Some(Value::Null) => None,
        Some(Value::String(reason)) => Some(reason.clone()),
        _ => {
            return Err(BenchmarkError::Execution(
                "invalid Ollama done_reason".into(),
            ));
        }
    };

    Ok(BenchmarkRun {
        kind,
        prompt_eval_count: counter(&value, "prompt_eval_count")?,
        eval_count: counter(&value, "eval_count")?,
        prompt_eval_duration_ns: counter(&value, "prompt_eval_duration")?,
        eval_duration_ns: counter(&value, "eval_duration")?,
        total_duration_ns: counter(&value, "total_duration")?,
        load_duration_ns: counter(&value, "load_duration")?,
        done_reason,
    })
}

/// Maximum accepted HTTP response size, including headers.
pub const MAX_HTTP_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// Returns the expected complete HTTP response size once headers are available.
///
/// `None` means that more header bytes are needed. A returned size may be
/// greater than the bytes received so far, indicating an incomplete body.
///
/// # Errors
///
/// Rejects invalid framing, unsuccessful status, and oversized responses.
pub fn http_response_total_length(response: &[u8]) -> Result<Option<usize>, BenchmarkError> {
    if response.len() > MAX_HTTP_RESPONSE_BYTES {
        return Err(BenchmarkError::Execution(
            "Ollama HTTP response exceeds size limit".into(),
        ));
    }

    let Some(boundary) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Ok(None);
    };

    let header = std::str::from_utf8(&response[..boundary])
        .map_err(|_| BenchmarkError::Execution("invalid HTTP headers".into()))?;

    let mut lines = header.split("\r\n");
    let status = lines
        .next()
        .ok_or_else(|| BenchmarkError::Execution("missing HTTP status".into()))?;

    if status != "HTTP/1.1 200 OK" && status != "HTTP/1.0 200 OK" {
        return Err(BenchmarkError::Execution(format!(
            "Ollama HTTP request failed: {status}"
        )));
    }

    let mut length = None;

    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| BenchmarkError::Execution("malformed HTTP header".into()))?;

        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(BenchmarkError::Execution(
                "unsupported HTTP transfer encoding".into(),
            ));
        }

        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err(BenchmarkError::Execution(
                    "duplicate HTTP content length".into(),
                ));
            }

            length =
                Some(value.trim().parse::<usize>().map_err(|_| {
                    BenchmarkError::Execution("invalid HTTP content length".into())
                })?);
        }
    }

    let length =
        length.ok_or_else(|| BenchmarkError::Execution("missing HTTP content length".into()))?;

    let expected = (boundary + 4).checked_add(length).ok_or_else(|| {
        BenchmarkError::Execution("Ollama HTTP response exceeds size limit".into())
    })?;

    if expected > MAX_HTTP_RESPONSE_BYTES {
        return Err(BenchmarkError::Execution(
            "Ollama HTTP response exceeds size limit".into(),
        ));
    }

    Ok(Some(expected))
}

/// Extracts a complete, length-delimited HTTP response body.
///
/// # Errors
///
/// Rejects incomplete, oversized, malformed, or unsuccessful responses.
pub fn parse_http_response(response: &[u8]) -> Result<&[u8], BenchmarkError> {
    if response.len() > MAX_HTTP_RESPONSE_BYTES {
        return Err(BenchmarkError::Execution(
            "Ollama HTTP response exceeds size limit".into(),
        ));
    }

    let boundary = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| BenchmarkError::Execution("incomplete HTTP headers".into()))?;

    let header = std::str::from_utf8(&response[..boundary])
        .map_err(|_| BenchmarkError::Execution("invalid HTTP headers".into()))?;

    let mut lines = header.split("\r\n");
    let status = lines
        .next()
        .ok_or_else(|| BenchmarkError::Execution("missing HTTP status".into()))?;

    if status != "HTTP/1.1 200 OK" && status != "HTTP/1.0 200 OK" {
        return Err(BenchmarkError::Execution(format!(
            "Ollama HTTP request failed: {status}"
        )));
    }

    let mut length = None;

    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| BenchmarkError::Execution("malformed HTTP header".into()))?;

        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(BenchmarkError::Execution(
                "unsupported HTTP transfer encoding".into(),
            ));
        }

        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err(BenchmarkError::Execution(
                    "duplicate HTTP content length".into(),
                ));
            }

            length =
                Some(value.trim().parse::<usize>().map_err(|_| {
                    BenchmarkError::Execution("invalid HTTP content length".into())
                })?);
        }
    }

    let length =
        length.ok_or_else(|| BenchmarkError::Execution("missing HTTP content length".into()))?;

    let body_start = boundary + 4;
    let body = &response[body_start..];

    if body.len() != length {
        return Err(BenchmarkError::Execution(
            "HTTP response body length mismatch".into(),
        ));
    }

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{BenchmarkConfiguration, InferenceEngineId};

    fn request() -> BenchmarkRequest {
        BenchmarkRequest {
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
        }
    }

    fn response() -> Vec<u8> {
        br#"{
            "done":true,
            "done_reason":"length",
            "prompt_eval_count":50,
            "eval_count":96,
            "prompt_eval_duration":100,
            "eval_duration":3500000000,
            "total_duration":3600000000,
            "load_duration":0
        }"#
        .to_vec()
    }

    #[test]
    fn identifies_complete_and_incomplete_http_frames() {
        let headers = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n";
        let complete = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";

        assert_eq!(http_response_total_length(b"HTTP/1.1"), Ok(None));
        assert_eq!(
            http_response_total_length(headers),
            Ok(Some(complete.len()))
        );
        assert_eq!(
            http_response_total_length(complete),
            Ok(Some(complete.len()))
        );
    }

    #[test]
    fn rejects_invalid_http_frame_headers() {
        for response in [
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".as_slice(),
            b"HTTP/1.1 200 OK\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n",
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\n\r\n",
        ] {
            assert!(http_response_total_length(response).is_err());
        }
    }

    #[test]
    fn parses_length_delimited_http_response() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(parse_http_response(response).unwrap(), b"{}");
    }

    #[test]
    fn rejects_http_error_status() {
        let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\n\r\n{}";
        assert!(parse_http_response(response).is_err());
    }

    #[test]
    fn rejects_missing_content_length() {
        let response = b"HTTP/1.1 200 OK\r\n\r\n{}";
        assert!(parse_http_response(response).is_err());
    }

    #[test]
    fn rejects_chunked_transfer_encoding() {
        let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert!(parse_http_response(response).is_err());
    }

    #[test]
    fn rejects_mismatched_body_length() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n{}";
        assert!(parse_http_response(response).is_err());
    }

    #[test]
    fn rejects_oversized_http_response() {
        let response = vec![b'x'; MAX_HTTP_RESPONSE_BYTES + 1];
        assert!(parse_http_response(&response).is_err());
    }

    #[test]
    fn encodes_explicit_benchmark_settings() {
        let bytes = encode_generate_request(&request());
        let value: Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(value["model"], "example:latest");
        assert_eq!(value["stream"], false);
        assert_eq!(value["think"], false);
        assert_eq!(value["options"]["num_ctx"], 8192);
        assert_eq!(value["options"]["num_thread"], 8);
        assert_eq!(value["options"]["num_predict"], 96);
        assert_eq!(value["options"]["seed"], 42);
    }

    #[test]
    fn preserves_ollama_counters() {
        let run = decode_generate_response(&response(), BenchmarkRunKind::Measured).unwrap();

        assert_eq!(run.eval_count, 96);
        assert_eq!(run.eval_duration_ns, 3_500_000_000);
        assert_eq!(run.done_reason.as_deref(), Some("length"));
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(decode_generate_response(b"{invalid", BenchmarkRunKind::Measured,).is_err());
    }

    #[test]
    fn rejects_missing_timing_counter() {
        let mut value: Value = serde_json::from_slice(&response()).unwrap();
        value.as_object_mut().unwrap().remove("eval_duration");

        let body = serde_json::to_vec(&value).unwrap();
        assert!(decode_generate_response(&body, BenchmarkRunKind::Measured,).is_err());
    }

    #[test]
    fn rejects_incomplete_response() {
        let mut value: Value = serde_json::from_slice(&response()).unwrap();
        value["done"] = json!(false);

        let body = serde_json::to_vec(&value).unwrap();
        assert!(decode_generate_response(&body, BenchmarkRunKind::Warmup,).is_err());
    }
}
