use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::Value;

pub fn decode_payload(token: &str) -> Option<Value> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let _signature = parts.next()?;
    if parts.next().is_some() {
        return None;
    }

    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::decode_payload;

    #[test]
    fn decode_payload_returns_none_for_invalid_token() {
        assert!(decode_payload("invalid-token").is_none());
    }

    #[test]
    fn decode_payload_returns_payload_for_valid_jwt() {
        let token = "eyJhbGciOiJub25lIn0.eyJleHAiOjEyMywiZW1haWwiOiJ0ZXN0QGV4YW1wbGUuY29tIn0.";
        let payload = decode_payload(token).expect("payload should decode");

        assert_eq!(payload["exp"], 123);
        assert_eq!(payload["email"], "test@example.com");
    }
}
