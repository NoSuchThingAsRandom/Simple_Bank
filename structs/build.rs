extern crate protobuf_codegen_pure;

fn main() {
    println!("Hello");

    let res = protobuf_codegen_pure::Codegen::new()
        .out_dir("src/protos")
        .inputs(&["src/protos/message.proto"])
        .include("src/protos")
        .run();
    if let Err(e) = res {
        panic!("protos compile failed {}", e);
    }
}
