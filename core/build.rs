// Embeds the git commit into the binary for the System Info page.
fn main() {
    let commit = std::env::var("GITHUB_SHA")
        .ok()
        .map(|s| s.chars().take(9).collect())
        .or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--short=9", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=ARTAVEO_GIT_COMMIT={commit}");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=seeds");
    println!("cargo:rerun-if-changed=migrations");
}
