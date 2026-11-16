//! Windows resource stamp: the icon Explorer shows and the version string in
//! the file's properties. Only the MSVC environment has rc.exe, so other
//! toolchains simply build the exe without a resource section.

fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");

    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        embed_resource::compile("assets/app.rc", embed_resource::NONE);
    }
}
