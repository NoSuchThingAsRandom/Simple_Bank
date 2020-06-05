fn main() {
    println!("Hello");
    prost_build::compile_protos(&["src/proto/message.proto"], &["src/"]).unwrap();
}
