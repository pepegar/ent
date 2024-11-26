use anyhow::Result;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tonic::{Request, Status};

static JWT_VALIDATOR: OnceCell<JwtValidator> = OnceCell::new();

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iss: String,
}

#[derive(Clone)]
pub struct JwtValidator {
    decoding_key: DecodingKey,
    issuer: String,
}

impl JwtValidator {
    pub fn new(public_key_pem: &str, issuer: String) -> Result<Self> {
        let decoding_key = DecodingKey::from_rsa_pem(public_key_pem.as_bytes())?;
        Ok(Self {
            decoding_key,
            issuer,
        })
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims> {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);

        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)?;
        Ok(token_data.claims)
    }

    // Initialize the global JWT validator
    pub fn init(public_key_pem: &str, issuer: String) -> Result<()> {
        let validator = JwtValidator::new(public_key_pem, issuer)?;
        JWT_VALIDATOR
            .set(validator)
            .map_err(|_| anyhow::anyhow!("JWT Validator has already been initialized"))
    }

    // Get the global JWT validator instance
    pub fn get() -> Option<&'static JwtValidator> {
        JWT_VALIDATOR.get()
    }
}

pub trait AuthenticatedRequest {
    fn user_id(&self) -> Result<String, Status>;
}

impl<T> AuthenticatedRequest for Request<T> {
    fn user_id(&self) -> Result<String, Status> {
        let token = self
            .metadata()
            .get("authorization")
            .ok_or_else(|| Status::unauthenticated("Missing authorization token"))?
            .to_str()
            .map_err(|_| Status::unauthenticated("Invalid authorization token"))?;

        let token = token.strip_prefix("Bearer ").unwrap_or(token);

        let validator =
            JwtValidator::get().ok_or_else(|| Status::internal("JWT validator not configured"))?;

        let claims = validator
            .validate_token(token)
            .map_err(|_| Status::unauthenticated("Invalid token"))?;

        Ok(claims.sub)
    }
}
