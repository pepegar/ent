use anyhow::Result;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String, // Subject (user ID)
    exp: usize,  // Expiration time
    iss: String, // Issuer
}

pub fn generate_test_token(user_id: &str) -> Result<String> {
    let private_key = fs::read_to_string("../test/data/private.pem")?;
    let encoding_key = EncodingKey::from_rsa_pem(private_key.as_bytes())?;

    // Set expiration to 1 hour from now
    let expiration = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as usize + 3600;

    let claims = Claims {
        sub: user_id.to_string(),
        exp: expiration,
        iss: "ent".to_string(),
    };

    let token = encode(
        &Header::new(jsonwebtoken::Algorithm::RS256),
        &claims,
        &encoding_key,
    )?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token() {
        let token = generate_test_token("test-user-123").unwrap();
        assert!(!token.is_empty());
        println!("Generated token: {}", token);
    }
}
