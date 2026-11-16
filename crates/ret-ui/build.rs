fn main() {
    println!("cargo:rerun-if-changed=ret.rc");
    println!("cargo:rerun-if-changed=../../assets/ret.ico");
    embed_resource::compile("ret.rc", embed_resource::NONE)
        .manifest_required()
        .expect("failed to embed the Windows application icon");
}
