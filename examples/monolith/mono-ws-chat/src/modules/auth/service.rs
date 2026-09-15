use dashmap::DashMap;
use ferrite_auth_jwt::{JwtError, JwtService};
use ferrite_framework::{inject, injectable, HttpError};

use super::dto::{AuthResponse, LoginDto, MeResponse, RegisterDto};

#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub password: String,
}

#[injectable]
pub struct UsersRepo {
    inner: DashMap<String, User>,
}

impl UsersRepo {
    #[inject]
    pub fn new() -> Self {
        Self {
            inner: DashMap::new().into(),
        }
    }

    pub fn next_id(&self) -> i64 {
        (self.inner.len() as i64) + 1
    }

    pub fn find_by_email(&self, email: &str) -> Option<User> {
        self.inner.get(email).map(|r| r.value().clone())
    }

    pub fn find_by_id(&self, id: i64) -> Option<User> {
        self.inner
            .iter()
            .find(|r| r.value().id == id)
            .map(|r| r.value().clone())
    }

    pub fn insert(&self, user: User) {
        self.inner.insert(user.email.clone(), user);
    }
}

#[injectable]
pub struct AuthService {
    users: UsersRepo,
    jwt: JwtService,
}

impl AuthService {
    #[inject]
    pub fn new(users: UsersRepo, jwt: JwtService) -> Self {
        Self { users, jwt }
    }

    pub fn register(&self, dto: RegisterDto) -> Result<AuthResponse, HttpError> {
        if self.users.find_by_email(&dto.email).is_some() {
            return Err(HttpError::bad_request("email already registered"));
        }
        let id = self.users.next_id();
        let user = User {
            id,
            email: dto.email.clone(),
            name: dto.name.clone(),
            password: dto.password,
        };
        self.users.insert(user.clone());
        let token = self.jwt.sign(user.id, &user.email).map_err(jwt_to_http)?;
        Ok(AuthResponse {
            token,
            token_type: "Bearer",
            expires_in: self.jwt.default_ttl_secs(),
            user_id: user.id,
            email: user.email,
            name: user.name,
        })
    }

    pub fn login(&self, dto: LoginDto) -> Result<AuthResponse, HttpError> {
        let user = self
            .users
            .find_by_email(&dto.email)
            .ok_or_else(|| HttpError::unauthorized("invalid email or password"))?;
        if user.password != dto.password {
            return Err(HttpError::unauthorized("invalid email or password"));
        }
        let token = self.jwt.sign(user.id, &user.email).map_err(jwt_to_http)?;
        Ok(AuthResponse {
            token,
            token_type: "Bearer",
            expires_in: self.jwt.default_ttl_secs(),
            user_id: user.id,
            email: user.email,
            name: user.name,
        })
    }

    pub fn me(&self, sub: i64, email: &str) -> Result<MeResponse, HttpError> {
        let user = self
            .users
            .find_by_id(sub)
            .or_else(|| self.users.find_by_email(email))
            .ok_or_else(|| HttpError::not_found("user not found"))?;
        Ok(MeResponse {
            id: user.id,
            email: user.email,
            name: user.name,
        })
    }

    pub fn resolve(&self, sub: i64, email: &str) -> Option<User> {
        self.users
            .find_by_id(sub)
            .or_else(|| self.users.find_by_email(email))
    }
}

fn jwt_to_http(e: JwtError) -> HttpError {
    match e {
        JwtError::MissingSecret => HttpError::internal("JWT_SECRET not configured"),
        JwtError::InvalidToken(_) => HttpError::unauthorized("invalid credentials"),
        other => HttpError::internal(format!("jwt: {other}")),
    }
}
