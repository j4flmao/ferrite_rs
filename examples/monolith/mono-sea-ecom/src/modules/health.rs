use ferrite_framework::{controller, impl_controller, inject, Json};

use crate::app_service::{AppInfo, AppService, HealthInfo};

#[controller("")]
pub struct RootController {
    service: AppService,
}

#[impl_controller]
impl RootController {
    #[inject]
    pub fn new(service: AppService) -> Self {
        Self { service }
    }

    #[get("/")]
    pub async fn root(&self) -> Json<AppInfo> {
        Json(self.service.info())
    }

    #[get("/healthz")]
    pub async fn healthz(&self) -> Json<HealthInfo> {
        Json(self.service.health())
    }
}
