//! `rbx-observe storefront` — what the experience sells.

use anyhow::Result;
use serde::Serialize;

use crate::api::monetization::{DeveloperProduct, GamePass};
use crate::api::Client;
use crate::render::{asset_hint, date, dim, heading, price, thousands, NAME_WIDTH};

#[derive(Debug, Serialize)]
pub struct Storefront {
    pub universe_id: u64,
    pub game_passes: Vec<GamePass>,
    pub developer_products: Vec<DeveloperProduct>,
    pub summary: Summary,
}

/// The numbers a competitor's price list is actually read for. Computed over
/// **for-sale** items only: an off-sale pass at R$ 1 000 is a leftover, and
/// letting it set the maximum makes the whole row lie.
#[derive(Debug, Default, Serialize)]
pub struct Summary {
    pub passes_for_sale: usize,
    pub products_for_sale: usize,
    pub min_price: Option<u64>,
    pub median_price: Option<u64>,
    pub max_price: Option<u64>,
}

impl Summary {
    fn of(passes: &[GamePass], products: &[DeveloperProduct]) -> Self {
        let mut prices: Vec<u64> = passes
            .iter()
            .filter(|p| p.is_for_sale)
            .filter_map(|p| p.price)
            .chain(
                products
                    .iter()
                    .filter(|p| p.is_for_sale)
                    .filter_map(|p| p.price_in_robux),
            )
            .filter(|price| *price > 0)
            .collect();
        prices.sort_unstable();

        Self {
            passes_for_sale: passes.iter().filter(|p| p.is_for_sale).count(),
            products_for_sale: products.iter().filter(|p| p.is_for_sale).count(),
            min_price: prices.first().copied(),
            median_price: prices.get(prices.len() / 2).copied(),
            max_price: prices.last().copied(),
        }
    }
}

pub async fn collect(client: &Client, universe_id: u64) -> Result<Storefront> {
    let mut game_passes = client.game_passes(universe_id).await?;
    let mut developer_products = client.developer_products(universe_id).await?;

    // Expensive first, off-sale last. The API order is neither, and a price
    // list read out of order is a price list nobody reads: the top of a
    // catalogue is the whale offer, and that is the number being compared.
    game_passes.sort_by(|a, b| {
        sort_key(a.price, a.is_for_sale)
            .cmp(&sort_key(b.price, b.is_for_sale))
            .then_with(|| a.name.cmp(&b.name))
    });
    developer_products.sort_by(|a, b| {
        sort_key(a.price_in_robux, a.is_for_sale)
            .cmp(&sort_key(b.price_in_robux, b.is_for_sale))
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(Storefront {
        universe_id,
        summary: Summary::of(&game_passes, &developer_products),
        game_passes,
        developer_products,
    })
}

/// Sorts descending by price with everything off sale pushed to the end.
/// `Reverse` on the price rather than reversing the whole comparison, so the
/// name tiebreak stays alphabetical.
fn sort_key(price: Option<u64>, for_sale: bool) -> (bool, std::cmp::Reverse<u64>) {
    (!for_sale, std::cmp::Reverse(price.unwrap_or(0)))
}

pub fn render(store: &Storefront) {
    println!("{}", heading("Game passes"));
    if store.game_passes.is_empty() {
        println!("  {}", dim("none"));
    }
    for pass in &store.game_passes {
        println!(
            "  {:>10}  {:<NAME_WIDTH$}  {}",
            price(pass.price, pass.is_for_sale),
            pass.name,
            dim(&format!(
                "pass {} · {} · icon {}",
                pass.id,
                date(pass.created.as_deref()),
                id_or_none(pass.display_icon_image_asset_id)
            ))
        );
    }
    println!();

    println!("{}", heading("Developer products"));
    if store.developer_products.is_empty() {
        println!("  {}", dim("none"));
    }
    for product in &store.developer_products {
        println!(
            "  {:>10}  {:<NAME_WIDTH$}  {}",
            price(product.price_in_robux, product.is_for_sale),
            product.name,
            dim(&format!(
                "product {} · {} · icon {}",
                product.developer_product_id,
                date(product.created.as_deref()),
                id_or_none(product.icon_image_asset_id)
            ))
        );
    }
    println!();

    let s = &store.summary;
    println!("{}", heading("Summary"));
    println!(
        "  {} pass(es) and {} product(s) on sale",
        s.passes_for_sale, s.products_for_sale
    );
    if let (Some(min), Some(median), Some(max)) = (s.min_price, s.median_price, s.max_price) {
        println!(
            "  prices        R$ {} low · R$ {} median · R$ {} high",
            thousands(min),
            thousands(median),
            thousands(max)
        );
    }

    let icons: Vec<u64> = store
        .game_passes
        .iter()
        .filter_map(|pass| pass.display_icon_image_asset_id)
        .chain(
            store
                .developer_products
                .iter()
                .filter_map(|product| product.icon_image_asset_id),
        )
        .collect();
    if let Some(hint) = asset_hint(&icons) {
        println!("  {}", dim(&hint));
    }
}

fn id_or_none(asset_id: Option<u64>) -> String {
    match asset_id {
        Some(id) => id.to_string(),
        None => "none".to_string(),
    }
}

pub async fn run(client: &Client, universe_id: u64, json: bool) -> Result<()> {
    let store = collect(client, universe_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&store)?);
    } else {
        render(&store);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn collect_reads_both_catalogs_and_summarises_for_sale_items_only() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/game-passes/v1/universes/42/game-passes"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"gamePasses":[
                   {"id":1,"name":"VIP","price":100,"isForSale":true,"displayIconImageAssetId":11},
                   {"id":2,"name":"Old","price":9999,"isForSale":false}],
                   "nextPageToken":null}"#,
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(
                "/developer-products/v2/universes/42/developerproducts",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"developerProducts":[
                   {"ProductId":9,"DeveloperProductId":8,"Name":"2X Money","PriceInRobux":49,
                    "IsForSale":true,"IconImageAssetId":22}],
                   "nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        let store = collect(&Client::with_base_url(&server.uri()), 42)
            .await
            .unwrap();

        assert_eq!(store.game_passes.len(), 2);
        // Off sale sorts last even though its price is the highest number in
        // the response — the ordering is about what a buyer can pay today.
        assert_eq!(store.game_passes[0].name, "VIP");
        assert_eq!(store.game_passes[1].name, "Old");
        assert_eq!(store.summary.passes_for_sale, 1);
        assert_eq!(store.summary.products_for_sale, 1);
        // The off-sale R$ 9999 pass is excluded, so the high price is the VIP
        // pass rather than a leftover nobody can buy.
        assert_eq!(store.summary.max_price, Some(100));
        assert_eq!(store.summary.min_price, Some(49));
    }

    #[test]
    fn an_empty_storefront_summarises_to_nothing_rather_than_zero_prices() {
        let summary = Summary::of(&[], &[]);
        assert_eq!(summary.min_price, None);
        assert_eq!(summary.median_price, None);
        assert_eq!(summary.max_price, None);
    }
}
