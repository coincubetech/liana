use liana::miniscript::bitcoin;

// TODO: sign onramper url
const WIDGET_OPTIONS: &str = "{{BASE_URL}}/?apiKey={{API_KEY}}&mode={{MODE}}&partnerContext=CoincubeVault&defaultFiat={{DEFAULT_FIAT}}&onlyCryptoNetworks=bitcoin&sell_defaultFiat={{DEFAULT_FIAT}}&sell_onlyCryptoNetworks=bitcoin&redirectAtCheckout=true&enableCountrySelector=true&themeName=dark";

pub fn create_widget_url(
    currency: &str,
    address: Option<&str>,
    mode: &str,
    network: bitcoin::Network,
) -> Result<String, &'static str> {
    let api_key = match network {
        bitcoin::Network::Bitcoin => {
            option_env!("ONRAMPER_API_KEY").ok_or("`ONRAMPER_API_KEY` not configured")?
        }
        _ => "pk_test_01K2HQVXK7F5C8RDZ36WV2W3F5",
    };

    let base_url = match network {
        bitcoin::Network::Bitcoin => "https://buy.onramper.com",
        _ => "https://buy.onramper.dev",
    };

    let url = WIDGET_OPTIONS
        .replace("{{BASE_URL}}", base_url)
        .replace("{{MODE}}", mode)
        .replace("{{API_KEY}}", api_key)
        .replace("{{DEFAULT_FIAT}}", currency);

    // insert address if provided, otherwise remove the wallets parameter entirely
    Ok(match address {
        Some(a) => url.replace("{{ADDRESS}}", a),
        None => {
            // Remove the wallets parameter when no address is provided (e.g., for sell mode)
            url.replace("&wallets=btc:{{ADDRESS}}", "")
        }
    })
}
