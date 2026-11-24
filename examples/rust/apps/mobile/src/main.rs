#[cfg(target_os = "ios")]
fn main() {
    #[cfg(all(
        feature = "ipad-scenes",
        not(any(
            feature = "candidate-trial",
            feature = "editing-regression",
            feature = "navigation-trial"
        ))
    ))]
    kui_mobile_spike::start_tablet();
    #[cfg(any(
        not(feature = "ipad-scenes"),
        feature = "candidate-trial",
        feature = "editing-regression",
        feature = "navigation-trial"
    ))]
    kui_mobile_spike::start();
}

#[cfg(not(target_os = "ios"))]
fn main() {}
