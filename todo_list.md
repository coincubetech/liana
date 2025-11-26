# TODO List

1. Refactor `EncryptedSigner` paths to live under the configured Liana datadir and per-network directories.
2. Introduce a machine-level master key for Lightning secrets and derive the AES key from `PIN || master_key` using Argon2.
3. Improve encryption file format (version header, zeroized buffers, proper error handling) and integrate the new helpers into the Breez flow.
