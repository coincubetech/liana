const WIDGET_OPTIONS: &str = "{{BASE_URL}}/?apiKey={{API_KEY}}&mode={{MODE}}&partnerContext=CoincubeVault&defaultFiat={{DEFAULT_FIAT}}&wallets=btc:{{ADDRESS}}&onlyCryptoNetworks=bitcoin&sell_defaultFiat={{DEFAULT_FIAT}}&sell_onlyCryptoNetworks=bitcoin&redirectAtCheckout=true&enableCountrySelector=true&themeName=dark";

const fn api_key() -> Option<&'static str> {
    if cfg!(debug_assertions) {
        Some("pk_test_01K2HQVXK7F5C8RDZ36WV2W3F5")
    } else {
        option_env!("ONRAMPER_API_KEY")
    }
}

const fn base_url() -> &'static str {
    if cfg!(debug_assertions) {
        "https://buy.onramper.dev"
    } else {
        "https://buy.onramper.com"
    }
}

pub fn create_widget_url(currency: &str, address: Option<&str>, mode: &str) -> Option<String> {
    let url = WIDGET_OPTIONS
        .replace("{{BASE_URL}}", base_url())
        .replace("{{MODE}}", mode)
        .replace("{{API_KEY}}", api_key()?)
        .replace("{{DEFAULT_FIAT}}", currency);

    // insert address if any
    Some(match address {
        Some(a) => url.replace("{{ADDRESS}}", a),
        None => url,
    })
}
