use ferrite_framework::{controller, impl_controller, inject, Json};

use crate::app_service::{AppService, HealthInfo};

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

    #[get("/healthz")]
    pub async fn healthz(&self) -> Json<HealthInfo> {
        Json(self.service.health())
    }
}
