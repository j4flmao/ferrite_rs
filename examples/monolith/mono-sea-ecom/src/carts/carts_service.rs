use dashmap::DashMap;
use ferrite_framework::{inject, injectable};

use super::models::{CartItem, CartItemDetail, CartSummary};
use crate::products::ProductsService;

#[injectable]
pub struct CartsService {
    store: DashMap<(i64, i64), CartItem>,
    products: ProductsService,
}

impl Default for CartsService {
    fn default() -> Self {
        Self::new(ProductsService::new().into())
    }
}

impl CartsService {
    #[inject]
    pub fn new(products: ProductsService) -> Self {
        Self {
            store: DashMap::new().into(),
            products,
        }
    }

    pub async fn add_item(
        &self,
        user_id: i64,
        product_id: i64,
        quantity: i64,
    ) -> Result<CartSummary, String> {
        let product = self
            .products
            .find_one(product_id)
            .await
            .ok_or_else(|| "product not found".to_string())?;

        if product.stock < quantity {
            return Err(format!(
                "insufficient stock: available={}, requested={quantity}",
                product.stock
            ));
        }

        let key = (user_id, product_id);
        let mut entry = self.store.entry(key).or_insert_with(|| CartItem {
            user_id,
            product_id,
            quantity: 0,
            price_cents: product.price_cents,
        });
        entry.quantity += quantity;
        entry.price_cents = product.price_cents;

        Ok(self.summary(user_id).await)
    }

    pub async fn update_quantity(
        &self,
        user_id: i64,
        product_id: i64,
        quantity: i64,
    ) -> Result<CartSummary, String> {
        let product = self
            .products
            .find_one(product_id)
            .await
            .ok_or_else(|| "product not found".to_string())?;

        if product.stock < quantity {
            return Err(format!(
                "insufficient stock: available={}, requested={quantity}",
                product.stock
            ));
        }

        let key = (user_id, product_id);
        if !self.store.contains_key(&key) {
            return Err("cart item not found".to_string());
        }

        let mut entry = self.store.get_mut(&key).unwrap();
        entry.quantity = quantity;
        entry.price_cents = product.price_cents;

        Ok(self.summary(user_id).await)
    }

    pub async fn remove_item(&self, user_id: i64, product_id: i64) -> CartSummary {
        self.store.remove(&(user_id, product_id));
        self.summary(user_id).await
    }

    pub async fn clear(&self, user_id: i64) {
        self.store.retain(|k, _| k.0 != user_id);
    }

    pub async fn get_items(&self, user_id: i64) -> Vec<CartItem> {
        self.store
            .iter()
            .filter(|r| r.key().0 == user_id)
            .map(|r| r.value().clone())
            .collect()
    }

    pub async fn summary(&self, user_id: i64) -> CartSummary {
        let items = self.get_items(user_id).await;
        let mut details = Vec::with_capacity(items.len());
        let mut total_cents = 0i64;
        let mut total_items = 0i64;

        for item in items {
            let product = self.products.find_one(item.product_id).await;
            let name = product.as_ref().map(|p| p.name.clone()).unwrap_or_default();
            let image = product
                .as_ref()
                .and_then(|p| p.images.first().cloned())
                .unwrap_or_default();
            let unit = item.price_cents;
            let line = unit * item.quantity;
            total_cents += line;
            total_items += item.quantity;
            details.push(CartItemDetail {
                product_id: item.product_id,
                product_name: name,
                product_image: image,
                quantity: item.quantity,
                unit_price_cents: unit,
                line_total_cents: line,
            });
        }

        CartSummary {
            items: details,
            total_cents,
            total_items,
        }
    }
}
