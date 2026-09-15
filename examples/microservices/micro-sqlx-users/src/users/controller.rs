use crate::users::dto::{
    ChangePasswordDto, CreateUserDto, GetUserQuery, ListUsersQuery, UpdateUserDto, User, UserList,
};
use crate::users::service::UsersService;
use ferrite_auth_jwt::{AuthGuard, CurrentUser};
use ferrite_cqrs::QueryBus;
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json, Path, Query};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[controller("/users")]
pub struct UsersController {
    service: UsersService,
    queries: QueryBus,
}

#[impl_controller]
impl UsersController {
    #[inject]
    pub fn new(service: UsersService, queries: QueryBus) -> Self {
        Self { service, queries }
    }

    #[post("/commands/create")]
    #[use_guards(AuthGuard)]
    pub async fn create_user(
        &self,
        CurrentUser(claims): CurrentUser,
        Json(body): Json<CreateUserDto>,
    ) -> Result<Json<User>, HttpError> {
        let current = self
            .queries
            .dispatch(GetUserQuery {
                user_id: claims.sub,
            })
            .await
            .unwrap_or(None);
        if current.as_ref().map(|u| u.role.as_str()) != Some("admin") {
            return Err(HttpError::forbidden("admin role required"));
        }
        Ok(Json(self.service.create_user(body).await?))
    }

    #[put("/commands/{id}/update")]
    #[use_guards(AuthGuard)]
    pub async fn update_user(
        &self,
        CurrentUser(claims): CurrentUser,
        Path(id): Path<i64>,
        Json(body): Json<UpdateUserDto>,
    ) -> Result<Json<User>, HttpError> {
        let current = self
            .queries
            .dispatch(GetUserQuery {
                user_id: claims.sub,
            })
            .await
            .unwrap_or(None);
        let is_admin = current.as_ref().map(|u| u.role.as_str()) == Some("admin");
        let is_own = claims.sub == id;
        if !is_admin && !is_own {
            return Err(HttpError::forbidden(
                "can only update own profile or be admin",
            ));
        }
        Ok(Json(self.service.update_user(id, body).await?))
    }

    #[delete("/commands/{id}")]
    #[use_guards(AuthGuard)]
    pub async fn delete_user(
        &self,
        Path(id): Path<i64>,
        CurrentUser(claims): CurrentUser,
    ) -> Result<Json<()>, HttpError> {
        let current = self
            .queries
            .dispatch(GetUserQuery {
                user_id: claims.sub,
            })
            .await
            .unwrap_or(None);
        if current.as_ref().map(|u| u.role.as_str()) != Some("admin") {
            return Err(HttpError::forbidden("admin role required"));
        }
        self.service.delete_user(id).await?;
        Ok(Json(()))
    }

    #[get("/queries/{id}")]
    pub async fn get_user(&self, Path(id): Path<i64>) -> Json<Option<User>> {
        Json(
            self.queries
                .dispatch(GetUserQuery { user_id: id })
                .await
                .unwrap_or(None),
        )
    }

    #[get("/queries")]
    #[use_guards(AuthGuard)]
    pub async fn list_users(
        &self,
        Query(q): Query<ListParams>,
        CurrentUser(claims): CurrentUser,
    ) -> Result<Json<UserList>, HttpError> {
        let current = self
            .queries
            .dispatch(GetUserQuery {
                user_id: claims.sub,
            })
            .await
            .unwrap_or(None);
        if current.as_ref().map(|u| u.role.as_str()) != Some("admin") {
            return Err(HttpError::forbidden("admin role required"));
        }
        let query = ListUsersQuery {
            limit: q.limit.unwrap_or(20).clamp(1, 100),
            offset: q.offset.unwrap_or(0),
        };
        Ok(Json(self.queries.dispatch(query).await.unwrap_or(
            UserList {
                total: 0,
                limit: 20,
                offset: 0,
                items: vec![],
            },
        )))
    }

    #[post("/commands/{id}/password")]
    #[use_guards(AuthGuard)]
    pub async fn change_password(
        &self,
        CurrentUser(claims): CurrentUser,
        Path(id): Path<i64>,
        Json(body): Json<ChangePasswordDto>,
    ) -> Result<Json<()>, HttpError> {
        if claims.sub != id {
            return Err(HttpError::forbidden("can only change own password"));
        }
        self.service.change_password(id, body).await?;
        Ok(Json(()))
    }
}
