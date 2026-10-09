fn main() {
    println!("cargo:rerun-if-changed=resources/minuter.rc");
    println!("cargo:rerun-if-changed=resources/minuter.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("resources/minuter.rc");
    }
}
