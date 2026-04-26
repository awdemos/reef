pub mod chat {
    tonic::include_proto!("chat");
}

pub mod embeddings {
    tonic::include_proto!("embeddings");
}

pub mod completion {
    tonic::include_proto!("completion");
}

pub mod name {
    tonic::include_proto!("name");
}

pub mod counting {
    tonic::include_proto!("counting");
}

pub mod audio {
    tonic::include_proto!("audio");
}
