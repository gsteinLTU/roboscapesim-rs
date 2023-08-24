use tokio::sync::{oneshot, mpsc};

struct SocketActor {
    receiver: mpsc::Receiver<ActorMessage>,
    next_id: u32,
}
enum SocketActorMessage {
    GetUniqueId {
        respond_to: oneshot::Sender<u32>,
    },
}