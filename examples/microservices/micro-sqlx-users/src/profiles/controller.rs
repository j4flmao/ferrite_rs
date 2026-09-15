use crate::profiles::dto::{ProfileDto, ProfileListDto};
use crate::users::dto::{GetUserQuery, ListUsersQuery};
use crate::users::repo::UsersRepo;
use ferrite_cqrs::QueryBus;
use ferrite_framework::{controller, impl_controller, inject, Json, Path, Query};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[controller("/profiles")]
pub struct ProfilesController {
    queries: QueryBus,
    repo: UsersRepo,
}

#[impl_controller]
impl ProfilesController {
    #[inject]
    pub fn new(queries: QueryBus, repo: UsersRepo) -> Self {
        Self { queries, repo }
    }

    #[get("/{user_id}")]
    pub async fn get_profile(&self, Path(user_id): Path<i64>) -> Json<Option<ProfileDto>> {
        let user = self
            .queries
            .dispatch(GetUserQuery { user_id })
            .await
            .unwrap_or(None);
        Json(user.map(|u| ProfileDto {
            id: u.id,
            name: u.name,
            avatar_url: u.avatar_url,
            bio: u.bio,
            created_at: u.created_at,
        }))
    }

    #[get("/")]
    pub async fn list_profiles(&self, Query(q): Query<ListParams>) -> Json<ProfileListDto> {
        let limit = q.limit.unwrap_or(20).clamp(1, 100);
        let offset = q.offset.unwrap_or(0);
        let query = ListUsersQuery { limit, offset };
        let list = self
            .queries
            .dispatch(query)
            .await
            .unwrap_or(crate::users::dto::UserList {
                total: 0,
                limit: 20,
                offset: 0,
                items: vec![],
            });
        let items: Vec<ProfileDto> = list
            .items
            .into_iter()
            .map(|u| ProfileDto {
                id: u.id,
                name: u.name,
                avatar_url: u.avatar_url,
                bio: u.bio,
                created_at: u.created_at,
            })
            .collect();
        Json(ProfileListDto {
            total: list.total,
            limit: list.limit,
            offset: list.offset,
            items,
        })
    }
}
