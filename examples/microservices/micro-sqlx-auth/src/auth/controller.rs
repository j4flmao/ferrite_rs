use crate::auth::dto::{AuthResponse, LoginDto, RegisterDto, TokenValidationDto, User};
use crate::auth::handlers::{LoginUserCommand, RegisterUserCommand, ValidateTokenQuery};
use crate::auth::service::AuthService;
use ferrite_auth_jwt::{AuthGuard, CurrentUser};
use ferrite_cqrs::{CommandBus, QueryBus};
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json};
use uuid::Uuid;

#[controller("/auth")]
pub struct AuthController {
    service: AuthService,
    commands: CommandBus,
    queries: QueryBus,
}

#[impl_controller]
impl AuthController {
    #[inject]
    pub fn new(service: AuthService, commands: CommandBus, queries: QueryBus) -> Self {
        Self {
            service,
            commands,
            queries,
        }
    }

    #[post("/register")]
    pub async fn register(
        &self,
        Json(body): Json<RegisterDto>,
    ) -> Result<Json<AuthResponse>, HttpError> {
        let hash = self
            .service
            .hash_password(&body.password)
            .map_err(|_| HttpError::internal("password hashing failed"))?;
        let cmd = RegisterUserCommand {
            id: Uuid::new_v4().simple().to_string(),
            name: body.name.clone(),
            email: body.email.clone(),
            password_hash: hash,
            role: if body.role.trim().is_empty() {
                "user".into()
            } else {
                body.role.clone()
            },
        };
        let cmd_email = cmd.email.clone();
        let cmd_password = body.password.clone();
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        let login_dto = LoginDto {
            email: cmd_email,
            password: cmd_password,
        };
        Ok(Json(self.service.login(login_dto, None).await?))
    }

    #[post("/login")]
    pub async fn login(&self, Json(body): Json<LoginDto>) -> Result<Json<AuthResponse>, HttpError> {
        let ip = None;
        let cmd = LoginUserCommand {
            email: body.email.clone(),
            password: body.password.clone(),
            ip: None,
        };
        let _ = self.commands.dispatch(cmd).await;
        Ok(Json(self.service.login(body, ip).await?))
    }

    #[get("/me")]
    #[use_guards(AuthGuard)]
    pub async fn me(&self, CurrentUser(claims): CurrentUser) -> Json<Option<User>> {
        Json(self.service.me(claims.sub).await)
    }

    #[post("/validate")]
    pub async fn validate(
        &self,
        Json(body): Json<TokenValidationDto>,
    ) -> Result<Json<Option<User>>, HttpError> {
        let q = ValidateTokenQuery {
            token: body.token.clone(),
        };
        let result: Option<User> = self.queries.dispatch(q).await.unwrap_or(None);
        if result.is_none() {
            let svc_result = self.service.validate_token(body).await?;
            Ok(Json(svc_result))
        } else {
            Ok(Json(result))
        }
    }
}
