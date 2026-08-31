//! Tests for the metering program, run against LiteSVM: an in-process SVM,
//! so `cargo test` needs no validator. `anchor build` is still a prerequisite,
//! since the harness loads `target/deploy/pay_on_chain.so`.

#[cfg(test)]
mod harness;

#[cfg(test)]
mod test_client_parity;

#[cfg(test)]
mod test_metering;

#[cfg(test)]
mod test_preflight;
