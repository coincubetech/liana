use iced::Task;
use liana_ui::component::form;

use crate::app::view::{BuySellMessage, MavapayMessage};
use crate::services::{coincube::*, mavapay::*};

#[derive(Debug)]
pub enum MavapayFlowStep {
    Register {
        // TODO: change to normal strings
        first_name: form::Value<String>,
        last_name: form::Value<String>,
        password1: form::Value<String>,
        password2: form::Value<String>,
        email: form::Value<String>,
    },
    VerifyEmail {
        email: String,
        password: String,
        checking: bool,
    },
    Login {
        email: String,
        password: String,
    },
    ActiveBuysell {
        country: Country,
        banks: Option<MavapayBanks>,
        amount: u64,
        beneficiary: Option<Beneficiary>,
        selected_bank: Option<usize>,
        current_quote: Option<GetQuoteResponse>,
        // TODO: Display BTC price on buysell UI
        current_price: Option<GetPriceResponse>,
    },
}

/// State specific to Mavapay flow
pub struct MavapayState {
    pub step: MavapayFlowStep,

    // mavapay session information
    pub current_user: Option<User>,
    pub auth_token: Option<String>,

    // API clients
    pub mavapay_client: MavapayClient,
    pub coincube_client: CoincubeClient,
}

impl MavapayState {
    pub fn new() -> Self {
        Self {
            step: MavapayFlowStep::Login {
                email: String::new(),
                password: String::new(),
            },
            current_user: None,
            auth_token: None,
            mavapay_client: MavapayClient::new(),
            coincube_client: crate::services::coincube::CoincubeClient::new(),
        }
    }
}

impl MavapayState {
    pub fn get_price(&self, country_iso: Option<&str>) -> Task<BuySellMessage> {
        let client = self.mavapay_client.clone();
        let currency = match country_iso {
            Some("KE") => MavapayCurrency::KenyanShilling,
            Some("ZA") => MavapayCurrency::SouthAfricanRand,
            Some("NG") => MavapayCurrency::NigerianNaira,
            c => unreachable!("Country {:?} is not supported by Mavapay", c),
        };

        Task::perform(
            async move { client.get_price(currency).await },
            |result| match result {
                Ok(price) => BuySellMessage::Mavapay(MavapayMessage::PriceReceived(price)),
                Err(e) => BuySellMessage::SessionError(e.to_string()),
            },
        )
    }

    pub fn create_quote(&self, buy_or_sell: &super::panel::BuyOrSell) -> Task<BuySellMessage> {
        let MavapayFlowStep::ActiveBuysell {
            country,
            amount,
            beneficiary,
            ..
        } = &self.step
        else {
            return Task::none();
        };

        let local_currency = match country.code {
            "KE" => MavapayUnitCurrency::KenyanShillingCent,
            "NG" => MavapayUnitCurrency::NigerianNairaKobo,
            "ZA" => MavapayUnitCurrency::SouthAfricanRandCent,
            iso => unreachable!("Country ({}) is unsupported by Mavapay", iso),
        };

        let request = match buy_or_sell {
            super::panel::BuyOrSell::Sell => GetQuoteRequest {
                amount: amount.clone(),
                source_currency: MavapayUnitCurrency::BitcoinSatoshi,
                target_currency: local_currency,
                // TODO: Is direct onchain supported as a payment method? If no, then this is blocked by the breeze-sdk integration task
                payment_method: MavapayPaymentMethod::Lightning,
                payment_currency: MavapayUnitCurrency::BitcoinSatoshi,
                // automatically deposit fiat funds in beneficiary account
                autopayout: true,
                customer_internal_fee: Some(0),
                beneficiary: beneficiary.clone(),
            },
            super::panel::BuyOrSell::Buy { address } => GetQuoteRequest {
                amount: amount.clone(),
                source_currency: local_currency,
                target_currency: MavapayUnitCurrency::BitcoinSatoshi,
                payment_method: MavapayPaymentMethod::BankTransfer,
                payment_currency: MavapayUnitCurrency::BitcoinSatoshi,
                autopayout: true,
                beneficiary: Some(Beneficiary::Onchain {
                    on_chain_address: address.address.to_string(),
                }),
                customer_internal_fee: None,
            },
        };

        // prepare request
        let client = self.mavapay_client.clone();
        let coincube_client = self.coincube_client.clone();

        Task::perform(
            async move {
                // Step 1: Create quote with Mavapay
                let quote = client.create_quote(request).await?;
                tracing::info!("[MAVAPAY] Quote created: {}", quote.id);

                // TODO: Save quote to coincube-api (Step 2)

                // Step 3: Build quote display URL using quote_id
                let url = coincube_client.get_quote_display_url(&quote.id);

                Ok((quote, url))
            },
            |result: Result<(GetQuoteResponse, String), MavapayError>| match result {
                Ok((_, url)) => BuySellMessage::WebviewOpenUrl(url),
                Err(e) => BuySellMessage::SessionError(e.to_string()),
            },
        )
    }
}
