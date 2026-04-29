use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub bind_addr: String,
    pub cors_origin: String,
    pub s3: Option<S3Config>,
}

#[derive(Clone, Debug)]
pub struct S3Config {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let s3 = match (
            env::var("S3_ENDPOINT"), env::var("S3_BUCKET"),
            env::var("S3_ACCESS_KEY"), env::var("S3_SECRET_KEY"),
        ) {
            (Ok(endpoint), Ok(bucket), Ok(access_key), Ok(secret_key)) => Some(S3Config {
                endpoint, bucket, access_key, secret_key,
                region: env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".into()),
            }),
            _ => None,
        };
        Ok(Self {
            database_url: env::var("DATABASE_URL")?,
            jwt_secret: env::var("JWT_SECRET")?,
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            cors_origin: env::var("CORS_ORIGIN").unwrap_or_else(|_| "*".into()),
            s3,
        })
    }
}
