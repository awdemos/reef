fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = std::path::PathBuf::from("proto");

    let protos = [
        "proto/cowabunga_sdk/chat/chat.proto",
        "proto/cowabunga_sdk/embeddings/embeddings.proto",
        "proto/cowabunga_sdk/completion/completion.proto",
        "proto/cowabunga_sdk/name/name.proto",
        "proto/cowabunga_sdk/counting/counting.proto",
        "proto/cowabunga_sdk/audio/audio.proto",
    ];

    tonic_build::configure()
        .build_server(false)
        .compile(&protos, &[proto_dir])?;

    Ok(())
}
