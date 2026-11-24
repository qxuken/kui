#[cfg(target_os = "ios")]
fn main() {
    kui_tasks_mobile::start();
}

#[cfg(not(target_os = "ios"))]
fn main() {}
