//! User-friendly error message mapping for Breez SDK errors

#[cfg(feature = "breez")]
use super::BreezError;

/// Maps technical SDK errors to user-friendly messages
#[cfg(feature = "breez")]
pub fn friendly_error_message(error: &BreezError) -> String {
    match error {
        BreezError::ConnectionFailed(msg) => {
            format!("Connection lost: {}. Please check your internet connection and try again.", msg)
        }
        BreezError::NotInitialized => {
            "Wallet not ready. Please wait for initialization to complete.".to_string()
        }
        BreezError::SendFailed(msg) => {
            format!("Payment failed: {}. Please check the destination and try again.", msg)
        }
        BreezError::ReceiveFailed(msg) => {
            format!("Unable to generate invoice: {}. Please try again.", msg)
        }
        BreezError::PrepareFailed(msg) => {
            format!("Payment preparation failed: {}. Please try again.", msg)
        }
        BreezError::InvalidDestination => {
            "Invalid payment destination. Please check the address or invoice.".to_string()
        }
        BreezError::AmountOutOfLimits { min, max } => {
            format!("Amount must be between {} and {} sats.", min, max)
        }
        BreezError::InvalidMnemonic(msg) => {
            format!("Invalid mnemonic: {}.", msg)
        }
        BreezError::DerivationFailed(msg) => {
            format!("Key derivation failed: {}.", msg)
        }
        BreezError::InvalidPath(msg) => {
            format!("Invalid derivation path: {}.", msg)
        }
        BreezError::LimitsFetchFailed(msg) => {
            format!("Unable to fetch payment limits: {}. Please try again later.", msg)
        }
        BreezError::BalanceFetchFailed(msg) => {
            format!("Unable to fetch balance: {}. Please try again later.", msg)
        }
        BreezError::EventListenerFailed(msg) => {
            format!("Event listener failed: {}. The app may not receive real-time updates.", msg)
        }
        BreezError::SdkError(msg) => {
            if msg.contains("timeout") || msg.contains("timed out") {
                "Operation timed out. Please try again.".to_string()
            } else if msg.contains("offline") || msg.contains("unreachable") {
                "Service temporarily unavailable. Please try again later.".to_string()
            } else {
                format!("An error occurred: {}. Please try again or contact support.", msg)
            }
        }

        // Catch-all
        _ => {
            "Something went wrong. Please try again or restart the application.".to_string()
        }
    }
}

/// Provides a helpful action suggestion based on the error type
#[cfg(feature = "breez")]
pub fn error_action_hint(error: &BreezError) -> Option<String> {
    match error {
        BreezError::AmountOutOfLimits { .. } => {
            Some("💡 Check the payment limits in the UI".to_string())
        }

        BreezError::InvalidDestination => {
            Some("💡 Make sure you copied the entire invoice/address".to_string())
        }

        BreezError::NotInitialized => {
            Some("💡 Try restarting the application if this persists".to_string())
        }

        _ => None,
    }
}

/// Combines friendly message with action hint
#[cfg(feature = "breez")]
pub fn full_error_message(error: &BreezError) -> String {
    let message = friendly_error_message(error);
    
    if let Some(hint) = error_action_hint(error) {
        format!("{}\n\n{}", message, hint)
    } else {
        message
    }
}
